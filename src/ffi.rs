use std::ffi::{CStr, CString, c_char, c_void};
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::{Arc, Mutex, OnceLock};

use futures::StreamExt;
use tokio::runtime::Runtime;
use tokio::sync::oneshot;

use crate::client::{
    AnnounceHandle, ClientConfig, ClientError, LndClient, discover_nodes_to_json, parse_announce_json,
    parse_filter_json, watch_event_to_json,
};

type SharedRuntime = Arc<Runtime>;

static RUNTIME: OnceLock<SharedRuntime> = OnceLock::new();
static LAST_ERROR: Mutex<Option<CString>> = Mutex::new(None);

#[repr(C)]
pub struct LndClientHandle {
    client: LndClient,
}

#[repr(C)]
pub struct LndAnnounceHandle {
    handle: Option<AnnounceHandle>,
}

pub type LndWatchCallback = extern "C" fn(*const c_char, *mut c_void);

#[repr(C)]
pub struct LndWatchHandle {
    stop_tx: Option<oneshot::Sender<()>>,
    join_handle: Option<std::thread::JoinHandle<()>>,
}

fn runtime() -> Result<&'static SharedRuntime, ClientError> {
    if let Some(runtime) = RUNTIME.get() {
        return Ok(runtime);
    }
    let created = Arc::new(
        Runtime::new()
            .map_err(|error| ClientError::Api(format!("failed to create runtime: {error}")))?,
    );
    let _ = RUNTIME.set(created);
    RUNTIME
        .get()
        .ok_or_else(|| ClientError::Api("failed to initialize runtime".to_string()))
}

fn set_last_error(error: impl ToString) {
    let text = CString::new(error.to_string()).unwrap_or_else(|_| CString::new("unknown error").unwrap());
    if let Ok(mut slot) = LAST_ERROR.lock() {
        *slot = Some(text);
    }
}

fn clear_last_error() {
    if let Ok(mut slot) = LAST_ERROR.lock() {
        *slot = None;
    }
}

fn read_cstr(ptr: *const c_char, field: &str) -> Result<String, ClientError> {
    if ptr.is_null() {
        return Err(ClientError::InvalidConfig(format!("{field} is null")));
    }
    let value = unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .map_err(|_| ClientError::InvalidConfig(format!("{field} is not utf-8")))?;
    Ok(value.to_string())
}

fn into_c_string(text: String) -> *mut c_char {
    CString::new(text).unwrap().into_raw()
}

fn catch_ffi<T>(f: impl FnOnce() -> Result<T, ClientError>) -> Result<T, ClientError> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(result) => result,
        Err(_) => Err(ClientError::Api("panic in ffi entrypoint".to_string())),
    }
}

fn block_on_ffi<F, T>(future: F) -> Result<T, ClientError>
where
    F: Future<Output = Result<T, ClientError>> + Send + 'static,
    T: Send + 'static,
{
    let runtime = runtime()?.clone();
    if tokio::runtime::Handle::try_current().is_ok() {
        std::thread::spawn(move || runtime.block_on(future))
            .join()
            .map_err(|_| ClientError::Api("ffi task panicked".to_string()))?
    } else {
        runtime.block_on(future)
    }
}

#[unsafe(no_mangle)]
/// Create a client handle for later discovery, announce and watch calls.
///
/// # Safety
/// `server_url` must be a valid, null-terminated UTF-8 string.
/// `bearer_token` may be null, otherwise it must be a valid, null-terminated UTF-8 string.
pub unsafe extern "C" fn lnd_client_new(
    server_url: *const c_char,
    bearer_token: *const c_char,
) -> *mut LndClientHandle {
    clear_last_error();
    let result: Result<*mut LndClientHandle, ClientError> = catch_ffi(|| {
        let server_url = read_cstr(server_url, "server_url")?;
        let bearer_token = if bearer_token.is_null() {
            String::new()
        } else {
            read_cstr(bearer_token, "bearer_token")?
        };
        let client = LndClient::new(ClientConfig {
            server_url,
            bearer_token,
            ..ClientConfig::default()
        })?;
        Ok(Box::into_raw(Box::new(LndClientHandle { client })))
    });
    match result {
        Ok(handle) => handle,
        Err(error) => {
            set_last_error(error);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
/// Free a client handle previously returned by `lnd_client_new`.
///
/// # Safety
/// `handle` must be null or a pointer returned by `lnd_client_new` that has not been freed yet.
pub unsafe extern "C" fn lnd_client_free(handle: *mut LndClientHandle) {
    if handle.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(handle)) };
}

#[unsafe(no_mangle)]
/// Run a one-shot discovery request and return a newly allocated JSON string.
///
/// # Safety
/// `handle` must be a live pointer returned by `lnd_client_new`.
/// `filter_json` must be a valid, null-terminated UTF-8 string.
/// The returned pointer must be released with `lnd_string_free`.
pub unsafe extern "C" fn lnd_discover_json(
    handle: *mut LndClientHandle,
    filter_json: *const c_char,
) -> *mut c_char {
    clear_last_error();
    let result: Result<*mut c_char, ClientError> = catch_ffi(|| {
        let handle = unsafe { handle.as_ref() }
            .ok_or_else(|| ClientError::InvalidConfig("client handle is null".to_string()))?;
        let client = handle.client.clone();
        let filter_json = read_cstr(filter_json, "filter_json")?;
        let filter = parse_filter_json(&filter_json)?;
        let nodes = block_on_ffi(async move { client.list(filter).await })?;
        let json = discover_nodes_to_json(&nodes)?;
        Ok(into_c_string(json))
    });
    match result {
        Ok(ptr) => ptr,
        Err(error) => {
            set_last_error(error);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
/// Start a background announce loop from a JSON spec.
///
/// # Safety
/// `handle` must be a live pointer returned by `lnd_client_new`.
/// `announce_json` must be a valid, null-terminated UTF-8 string.
/// The returned pointer must be released with `lnd_announce_stop`.
pub unsafe extern "C" fn lnd_announce_start(
    handle: *mut LndClientHandle,
    announce_json: *const c_char,
) -> *mut LndAnnounceHandle {
    clear_last_error();
    let result: Result<*mut LndAnnounceHandle, ClientError> = catch_ffi(|| {
        let handle = unsafe { handle.as_ref() }
            .ok_or_else(|| ClientError::InvalidConfig("client handle is null".to_string()))?;
        let client = handle.client.clone();
        let announce_json = read_cstr(announce_json, "announce_json")?;
        let spec = parse_announce_json(&announce_json)?;
        let announce_handle = block_on_ffi(async move { client.announce_loop(spec) })?;
        Ok(Box::into_raw(Box::new(LndAnnounceHandle {
            handle: Some(announce_handle),
        })))
    });
    match result {
        Ok(ptr) => ptr,
        Err(error) => {
            set_last_error(error);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
/// Stop and free an announce handle.
///
/// # Safety
/// `handle` must be null or a pointer returned by `lnd_announce_start` that has not been stopped yet.
pub unsafe extern "C" fn lnd_announce_stop(handle: *mut LndAnnounceHandle) {
    if handle.is_null() {
        return;
    }
    let result = catch_ffi(|| {
        let handle = unsafe { &mut *handle };
        if let Some(announce_handle) = handle.handle.take() {
            block_on_ffi(async move { announce_handle.stop().await })?;
        }
        unsafe { drop(Box::from_raw(handle)) };
        Ok(())
    });
    if let Err(error) = result {
        set_last_error(error);
    }
}

#[unsafe(no_mangle)]
/// Start a background watch stream and invoke the callback for every JSON event.
///
/// # Safety
/// `handle` must be a live pointer returned by `lnd_client_new`.
/// `filter_json` must be a valid, null-terminated UTF-8 string.
/// `callback` must remain valid until `lnd_watch_stop` is called.
/// `user_data` is passed through to the callback without validation.
pub unsafe extern "C" fn lnd_watch_start(
    handle: *mut LndClientHandle,
    filter_json: *const c_char,
    callback: Option<LndWatchCallback>,
    user_data: *mut c_void,
) -> *mut LndWatchHandle {
    clear_last_error();
    let result: Result<*mut LndWatchHandle, ClientError> = catch_ffi(|| {
        let handle = unsafe { handle.as_ref() }
            .ok_or_else(|| ClientError::InvalidConfig("client handle is null".to_string()))?;
        let callback =
            callback.ok_or_else(|| ClientError::InvalidConfig("watch callback is null".to_string()))?;
        let filter_json = read_cstr(filter_json, "filter_json")?;
        let filter = parse_filter_json(&filter_json)?;
        let client = handle.client.clone();
        let runtime = runtime()?.clone();
        let user_data = user_data as usize;
        let (stop_tx, mut stop_rx) = oneshot::channel();
        let join_handle = std::thread::spawn(move || {
            runtime.block_on(async move {
                let mut stream = client.watch(filter);
                loop {
                    tokio::select! {
                        _ = &mut stop_rx => break,
                        event = stream.next() => {
                            match event {
                                Some(Ok(event)) => {
                                    if let Ok(json) = watch_event_to_json(&event)
                                        && let Ok(c_string) = CString::new(json)
                                    {
                                        callback(c_string.as_ptr(), user_data as *mut c_void);
                                    }
                                }
                                Some(Err(error)) => {
                                    set_last_error(error);
                                    break;
                                }
                                None => break,
                            }
                        }
                    }
                }
            });
        });
        Ok(Box::into_raw(Box::new(LndWatchHandle {
            stop_tx: Some(stop_tx),
            join_handle: Some(join_handle),
        })))
    });
    match result {
        Ok(ptr) => ptr,
        Err(error) => {
            set_last_error(error);
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
/// Stop and free a watch handle.
///
/// # Safety
/// `handle` must be null or a pointer returned by `lnd_watch_start` that has not been stopped yet.
pub unsafe extern "C" fn lnd_watch_stop(handle: *mut LndWatchHandle) {
    if handle.is_null() {
        return;
    }
    let result = catch_ffi(|| {
        let handle = unsafe { &mut *handle };
        if let Some(stop_tx) = handle.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        if let Some(join_handle) = handle.join_handle.take() {
            let _ = join_handle.join();
        }
        unsafe { drop(Box::from_raw(handle)) };
        Ok(())
    });
    if let Err(error) = result {
        set_last_error(error);
    }
}

#[unsafe(no_mangle)]
/// Free a C string returned by this library.
///
/// # Safety
/// `ptr` must be null or a pointer previously returned by this library via `CString::into_raw`.
pub unsafe extern "C" fn lnd_string_free(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe { drop(CString::from_raw(ptr)) };
}

#[unsafe(no_mangle)]
pub extern "C" fn lnd_last_error() -> *const c_char {
    if let Ok(slot) = LAST_ERROR.lock()
        && let Some(error) = slot.as_ref()
    {
        return error.as_ptr();
    }
    ptr::null()
}
