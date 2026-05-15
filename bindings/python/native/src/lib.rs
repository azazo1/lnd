use futures::StreamExt;
use lnd::{
    AddressSelection, AnnounceHandle as RustAnnounceHandle, ClientConfig, LndClient,
    parse_announce_json, parse_filter_json, resolve_announce_addrs_with_defaults,
};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use serde::Deserialize;
use tokio::runtime::Runtime;
use tokio::sync::oneshot;

#[derive(Debug, Clone, Deserialize)]
struct AddressSelectionConfig {
    include_loopback: bool,
    include_ipv6: bool,
    include_private_ipv4: bool,
    include_link_local_ipv4: bool,
    #[serde(default)]
    interface_allowlist: Vec<String>,
    #[serde(default)]
    interface_denylist: Vec<String>,
}

impl From<AddressSelectionConfig> for AddressSelection {
    fn from(value: AddressSelectionConfig) -> Self {
        Self {
            include_loopback: value.include_loopback,
            include_ipv6: value.include_ipv6,
            include_private_ipv4: value.include_private_ipv4,
            include_link_local_ipv4: value.include_link_local_ipv4,
            interface_allowlist: value.interface_allowlist,
            interface_denylist: value.interface_denylist,
        }
    }
}

fn runtime() -> PyResult<&'static Runtime> {
    static RUNTIME: std::sync::OnceLock<Runtime> = std::sync::OnceLock::new();
    if let Some(runtime) = RUNTIME.get() {
        return Ok(runtime);
    }
    let created = Runtime::new()
        .map_err(|error| PyRuntimeError::new_err(format!("failed to create runtime: {error}")))?;
    let _ = RUNTIME.set(created);
    RUNTIME
        .get()
        .ok_or_else(|| PyRuntimeError::new_err("failed to initialize runtime"))
}

fn parse_defaults(address_defaults_json: &str) -> PyResult<AddressSelection> {
    let selection: AddressSelectionConfig = serde_json::from_str(address_defaults_json)
        .map_err(|error| PyRuntimeError::new_err(format!("invalid address defaults: {error}")))?;
    Ok(selection.into())
}

fn build_client(
    server_url: String,
    bearer_token: String,
    timeout_ms: u64,
    reconnect_backoff_ms: (u64, u64),
    address_defaults_json: &str,
) -> PyResult<LndClient> {
    let config = ClientConfig {
        server_url,
        bearer_token,
        timeout: std::time::Duration::from_millis(timeout_ms),
        reconnect_backoff_min: std::time::Duration::from_millis(reconnect_backoff_ms.0),
        reconnect_backoff_max: std::time::Duration::from_millis(reconnect_backoff_ms.1),
        default_address_selection: parse_defaults(address_defaults_json)?,
    };
    LndClient::new(config).map_err(|error| PyRuntimeError::new_err(error.to_string()))
}

fn json_to_py(py: Python<'_>, value: serde_json::Value) -> PyResult<PyObject> {
    let module = py.import("json")?;
    let text = serde_json::to_string(&value)
        .map_err(|error| PyRuntimeError::new_err(format!("failed to encode json: {error}")))?;
    Ok(module.call_method1("loads", (text,))?.into())
}

#[pyclass]
struct NativeAnnounceHandle {
    handle: Option<RustAnnounceHandle>,
}

#[pymethods]
impl NativeAnnounceHandle {
    fn close(&mut self) -> PyResult<()> {
        if let Some(handle) = self.handle.take() {
            runtime()?
                .block_on(handle.stop())
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        }
        Ok(())
    }
}

#[pyclass]
struct NativeWatchHandle {
    stop_tx: Option<oneshot::Sender<()>>,
    join_handle: Option<std::thread::JoinHandle<()>>,
}

#[pymethods]
impl NativeWatchHandle {
    fn close(&mut self) {
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        if let Some(join_handle) = self.join_handle.take() {
            let _ = join_handle.join();
        }
    }
}

#[pyfunction]
fn discover_json(
    py: Python<'_>,
    server_url: String,
    bearer_token: String,
    filter_json: String,
    timeout_ms: u64,
    reconnect_backoff_ms: (u64, u64),
    address_defaults_json: String,
) -> PyResult<PyObject> {
    let client = build_client(
        server_url,
        bearer_token,
        timeout_ms,
        reconnect_backoff_ms,
        &address_defaults_json,
    )?;
    let filter: lnd::DiscoveryFilter = parse_filter_json(&filter_json)
        .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
    let nodes = runtime()?
        .block_on(client.list(filter))
        .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
    json_to_py(
        py,
        serde_json::to_value(nodes)
            .map_err(|error| PyRuntimeError::new_err(format!("failed to encode json: {error}")))?,
    )
}

#[pyfunction]
fn resolve_announce_addrs_json(
    py: Python<'_>,
    _server_url: String,
    _bearer_token: String,
    spec_json: String,
    _timeout_ms: u64,
    _reconnect_backoff_ms: (u64, u64),
    address_defaults_json: String,
) -> PyResult<PyObject> {
    let spec: lnd::AnnounceSpec = parse_announce_json(&spec_json)
        .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
    let addrs =
        resolve_announce_addrs_with_defaults(&spec, &parse_defaults(&address_defaults_json)?)
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
    let text = addrs.iter().map(ToString::to_string).collect::<Vec<_>>();
    json_to_py(
        py,
        serde_json::to_value(text)
            .map_err(|error| PyRuntimeError::new_err(format!("failed to encode json: {error}")))?,
    )
}

#[pyfunction]
fn announce_once_json(
    py: Python<'_>,
    server_url: String,
    bearer_token: String,
    spec_json: String,
    timeout_ms: u64,
    reconnect_backoff_ms: (u64, u64),
    address_defaults_json: String,
) -> PyResult<PyObject> {
    let client = build_client(
        server_url,
        bearer_token,
        timeout_ms,
        reconnect_backoff_ms,
        &address_defaults_json,
    )?;
    let spec: lnd::AnnounceSpec = parse_announce_json(&spec_json)
        .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
    let addrs = client
        .resolve_announce_addrs(&spec)
        .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
    let node = runtime()?
        .block_on(client.announce_once(spec.into_announcement(addrs)))
        .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
    json_to_py(
        py,
        serde_json::to_value(node)
            .map_err(|error| PyRuntimeError::new_err(format!("failed to encode json: {error}")))?,
    )
}

#[pyfunction]
fn announce_start(
    server_url: String,
    bearer_token: String,
    spec_json: String,
    timeout_ms: u64,
    reconnect_backoff_ms: (u64, u64),
    address_defaults_json: String,
) -> PyResult<NativeAnnounceHandle> {
    let client = build_client(
        server_url,
        bearer_token,
        timeout_ms,
        reconnect_backoff_ms,
        &address_defaults_json,
    )?;
    let spec: lnd::AnnounceSpec = parse_announce_json(&spec_json)
        .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
    let handle = client
        .announce_loop(spec)
        .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
    Ok(NativeAnnounceHandle {
        handle: Some(handle),
    })
}

#[pyfunction]
#[allow(clippy::too_many_arguments)]
fn watch_start(
    py: Python<'_>,
    server_url: String,
    bearer_token: String,
    filter_json: String,
    callback: PyObject,
    timeout_ms: u64,
    reconnect_backoff_ms: (u64, u64),
    address_defaults_json: String,
) -> PyResult<NativeWatchHandle> {
    let client = build_client(
        server_url,
        bearer_token,
        timeout_ms,
        reconnect_backoff_ms,
        &address_defaults_json,
    )?;
    let filter: lnd::DiscoveryFilter = parse_filter_json(&filter_json)
        .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
    let callback = callback.bind(py).clone().unbind();
    let (stop_tx, mut stop_rx) = oneshot::channel();
    let join_handle = std::thread::spawn(move || {
        let runtime = match runtime() {
            Ok(runtime) => runtime,
            Err(_) => return,
        };
        let mut stream = client.watch(filter);
        runtime.block_on(async move {
            loop {
                tokio::select! {
                    _ = &mut stop_rx => break,
                    event = stream.next() => {
                        match event {
                            Some(Ok(event)) => {
                                let value = match serde_json::to_value(event) {
                                    Ok(value) => value,
                                    Err(_) => break,
                                };
                                Python::with_gil(|py| {
                                    let _ = json_to_py(py, value)
                                        .and_then(|payload| callback.call1(py, (payload,)).map(|_| ()));
                                });
                            }
                            Some(Err(_)) | None => break,
                        }
                    }
                }
            }
        });
    });
    Ok(NativeWatchHandle {
        stop_tx: Some(stop_tx),
        join_handle: Some(join_handle),
    })
}

#[pymodule]
fn _native(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(discover_json, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_announce_addrs_json, module)?)?;
    module.add_function(wrap_pyfunction!(announce_once_json, module)?)?;
    module.add_function(wrap_pyfunction!(announce_start, module)?)?;
    module.add_function(wrap_pyfunction!(watch_start, module)?)?;
    module.add_class::<NativeAnnounceHandle>()?;
    module.add_class::<NativeWatchHandle>()?;
    Ok(())
}
