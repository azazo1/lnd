use std::ffi::{CStr, CString, c_char, c_void};
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use futures::StreamExt;
use tokio::runtime::Runtime;
use tokio::sync::oneshot;

use crate::client::{
    AnnounceHandle, ClientConfig, ClientError, LndClient, discover_nodes_to_json,
    list_network_id_candidates, parse_announce_json, parse_filter_json, parse_socket_addrs,
    resolve_announce_addrs_with_defaults, resolve_network_id_with_selection, watch_event_to_json,
};
use crate::protocol::{AddressSelection, AnnounceSpec, DiscoveryFilter};

type SharedRuntime = Arc<Runtime>;

static RUNTIME: OnceLock<SharedRuntime> = OnceLock::new();
static LAST_ERROR: Mutex<Option<CString>> = Mutex::new(None);

/// Opaque client handle used for discovery, announce and watch operations.
///
/// Create with `lnd_client_new` or `lnd_client_new_default`, then release with
/// `lnd_client_free`.
#[repr(C)]
pub struct LndClientHandle {
    _private: [u8; 0],
}

/// Opaque handle for a background announce loop.
///
/// Stop and free it with `lnd_announce_stop`.
#[repr(C)]
pub struct LndAnnounceHandle {
    _private: [u8; 0],
}

/// Opaque handle for a background watch loop.
///
/// Stop and free it with `lnd_watch_stop`.
#[repr(C)]
pub struct LndWatchHandle {
    _private: [u8; 0],
}

/// Opaque discovery filter handle used to build list and watch queries.
///
/// Create with `lnd_discovery_filter_new`, mutate with the setter functions and
/// release with `lnd_discovery_filter_free`.
#[repr(C)]
pub struct LndDiscoveryFilterHandle {
    _private: [u8; 0],
}

/// Opaque announce spec handle used to describe one node registration.
///
/// Create with `lnd_announce_spec_new`, mutate with the setter functions and
/// release with `lnd_announce_spec_free`.
#[repr(C)]
pub struct LndAnnounceSpecHandle {
    _private: [u8; 0],
}

/// Callback invoked by watch streams.
///
/// `payload` points to a temporary UTF-8 JSON string representing one event
/// envelope. Copy it inside the callback if it must outlive the call.
///
/// `user_data` is the opaque pointer originally passed to `lnd_watch_start` or
/// `lnd_watch_start_with_filter`.
pub type LndWatchCallback = extern "C" fn(*const c_char, *mut c_void);

struct LndClientState {
    config: ClientConfig,
    client: LndClient,
}

struct LndAnnounceState {
    handle: Option<AnnounceHandle>,
}

struct LndWatchState {
    stop_tx: Option<oneshot::Sender<()>>,
    join_handle: Option<std::thread::JoinHandle<()>>,
}

struct LndDiscoveryFilterState {
    filter: DiscoveryFilter,
}

struct LndAnnounceSpecState {
    spec: AnnounceSpec,
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
    let text =
        CString::new(error.to_string()).unwrap_or_else(|_| CString::new("unknown error").unwrap());
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

fn read_optional_cstr(ptr: *const c_char, field: &str) -> Result<Option<String>, ClientError> {
    if ptr.is_null() {
        return Ok(None);
    }
    read_cstr(ptr, field).map(Some)
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

fn build_client_state(config: ClientConfig) -> Result<LndClientState, ClientError> {
    let client = LndClient::new(config.clone())?;
    Ok(LndClientState { config, client })
}

fn rebuild_client(state: &mut LndClientState) -> Result<(), ClientError> {
    state.client = LndClient::new(state.config.clone())?;
    Ok(())
}

fn ensure_spec_selection(spec: &mut AnnounceSpec) -> &mut AddressSelection {
    spec.address_selection
        .get_or_insert_with(AddressSelection::default)
}

fn bool_result(result: Result<(), ClientError>) -> bool {
    clear_last_error();
    match result {
        Ok(()) => true,
        Err(error) => {
            set_last_error(error);
            false
        }
    }
}

fn ptr_result<T>(result: Result<*mut T, ClientError>) -> *mut T {
    clear_last_error();
    match result {
        Ok(ptr) => ptr,
        Err(error) => {
            set_last_error(error);
            ptr::null_mut()
        }
    }
}

unsafe fn cast_ref<'a, T, H>(handle: *const H, field: &str) -> Result<&'a T, ClientError> {
    if handle.is_null() {
        return Err(ClientError::InvalidConfig(format!("{field} is null")));
    }
    Ok(&*(handle as *const T))
}

unsafe fn cast_mut<'a, T, H>(handle: *mut H, field: &str) -> Result<&'a mut T, ClientError> {
    if handle.is_null() {
        return Err(ClientError::InvalidConfig(format!("{field} is null")));
    }
    Ok(&mut *(handle as *mut T))
}

unsafe fn free_handle<T, H>(handle: *mut H) {
    if !handle.is_null() {
        drop(Box::from_raw(handle as *mut T));
    }
}

fn into_handle<T, H>(value: T) -> *mut H {
    Box::into_raw(Box::new(value)) as *mut H
}

fn client_discover_json(
    client: LndClient,
    filter: DiscoveryFilter,
) -> Result<*mut c_char, ClientError> {
    let nodes = block_on_ffi(async move { client.list(filter).await })?;
    let json = discover_nodes_to_json(&nodes)?;
    Ok(into_c_string(json))
}

fn client_announce_once_json(
    client: LndClient,
    config: ClientConfig,
    spec: AnnounceSpec,
) -> Result<*mut c_char, ClientError> {
    let addrs = resolve_announce_addrs_with_defaults(&spec, &config.default_address_selection)?;
    let announcement = spec.into_announcement(addrs);
    let node = block_on_ffi(async move { client.announce_once(announcement).await })?;
    let json = serde_json::to_string(&node)?;
    Ok(into_c_string(json))
}

fn client_announce_start(
    client: LndClient,
    spec: AnnounceSpec,
) -> Result<*mut LndAnnounceHandle, ClientError> {
    let announce_handle = block_on_ffi(async move { client.announce_loop(spec) })?;
    Ok(into_handle::<LndAnnounceState, LndAnnounceHandle>(
        LndAnnounceState {
            handle: Some(announce_handle),
        },
    ))
}

fn client_watch_start(
    client: LndClient,
    filter: DiscoveryFilter,
    callback: LndWatchCallback,
    user_data: *mut c_void,
) -> Result<*mut LndWatchHandle, ClientError> {
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
    Ok(into_handle::<LndWatchState, LndWatchHandle>(
        LndWatchState {
            stop_tx: Some(stop_tx),
            join_handle: Some(join_handle),
        },
    ))
}

#[unsafe(no_mangle)]
/// Create a client handle from default config values.
///
/// The default client uses the library default server URL, timeout, reconnect
/// backoff and automatic address selection policy.
///
/// Returns a new handle on success, or `NULL` on failure. Inspect
/// `lnd_last_error()` when `NULL` is returned.
///
/// # Safety
/// The returned handle must be released with `lnd_client_free`.
pub unsafe extern "C" fn lnd_client_new_default() -> *mut LndClientHandle {
    ptr_result(catch_ffi(|| {
        let state = build_client_state(ClientConfig::default())?;
        Ok(into_handle::<LndClientState, LndClientHandle>(state))
    }))
}

#[unsafe(no_mangle)]
/// Create a client handle for later discovery, announce and watch calls.
///
/// `server_url` should point at the server root, for example
/// `https://registry.example.com`. `bearer_token` may be empty or null when the
/// server does not require authentication.
///
/// Returns a new handle on success, or `NULL` on failure. The constructor does
/// not contact the server immediately, so later network failures surface from
/// discover, announce or watch calls.
///
/// # Safety
/// `server_url` must be a valid, null-terminated UTF-8 string.
/// `bearer_token` may be null, otherwise it must be a valid, null-terminated UTF-8 string.
pub unsafe extern "C" fn lnd_client_new(
    server_url: *const c_char,
    bearer_token: *const c_char,
) -> *mut LndClientHandle {
    ptr_result(catch_ffi(|| {
        let server_url = read_cstr(server_url, "server_url")?;
        let bearer_token = read_optional_cstr(bearer_token, "bearer_token")?.unwrap_or_default();
        let state = build_client_state(ClientConfig {
            server_url,
            bearer_token,
            ..ClientConfig::default()
        })?;
        Ok(into_handle::<LndClientState, LndClientHandle>(state))
    }))
}

#[unsafe(no_mangle)]
/// Free a client handle.
///
/// It is safe to pass `NULL`. After this call the handle must not be reused.
///
/// # Safety
/// `handle` must be null or a pointer returned by this library that has not been freed yet.
pub unsafe extern "C" fn lnd_client_free(handle: *mut LndClientHandle) {
    free_handle::<LndClientState, LndClientHandle>(handle);
}

#[unsafe(no_mangle)]
/// Set the client base URL.
///
/// Use this to retarget an existing client at another registry without
/// reallocating the higher level wrapper object.
///
/// Returns `true` on success. On failure returns `false` and stores a message
/// retrievable through `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live client handle.
/// `server_url` must be a valid, null-terminated UTF-8 string.
pub unsafe extern "C" fn lnd_client_set_server_url(
    handle: *mut LndClientHandle,
    server_url: *const c_char,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndClientState, LndClientHandle>(handle, "client handle")?;
        state.config.server_url = read_cstr(server_url, "server_url")?;
        rebuild_client(state)
    }))
}

#[unsafe(no_mangle)]
/// Set or clear the client bearer token.
///
/// Pass `NULL` or an empty string to clear the token. The new token is used by
/// later discovery, announce and watch requests.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live client handle.
/// `bearer_token` may be null, otherwise it must be valid UTF-8.
pub unsafe extern "C" fn lnd_client_set_bearer_token(
    handle: *mut LndClientHandle,
    bearer_token: *const c_char,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndClientState, LndClientHandle>(handle, "client handle")?;
        state.config.bearer_token =
            read_optional_cstr(bearer_token, "bearer_token")?.unwrap_or_default();
        rebuild_client(state)
    }))
}

#[unsafe(no_mangle)]
/// Set the client request timeout in milliseconds.
///
/// This timeout affects future list, announce and watch setup requests.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live client handle.
pub unsafe extern "C" fn lnd_client_set_timeout_ms(
    handle: *mut LndClientHandle,
    timeout_ms: u64,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndClientState, LndClientHandle>(handle, "client handle")?;
        state.config.timeout = Duration::from_millis(timeout_ms);
        rebuild_client(state)
    }))
}

#[unsafe(no_mangle)]
/// Set reconnect backoff bounds in milliseconds.
///
/// Background announce and watch loops use this range for exponential backoff
/// after transient errors.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live client handle.
pub unsafe extern "C" fn lnd_client_set_reconnect_backoff_ms(
    handle: *mut LndClientHandle,
    min_ms: u64,
    max_ms: u64,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndClientState, LndClientHandle>(handle, "client handle")?;
        state.config.reconnect_backoff_min = Duration::from_millis(min_ms);
        state.config.reconnect_backoff_max = Duration::from_millis(max_ms);
        rebuild_client(state)
    }))
}

#[unsafe(no_mangle)]
/// Set whether auto discovered addresses may include loopback interfaces.
///
/// This changes the client default used by later address resolution operations.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live client handle.
pub unsafe extern "C" fn lnd_client_set_include_loopback(
    handle: *mut LndClientHandle,
    on: bool,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndClientState, LndClientHandle>(handle, "client handle")?;
        state.config.default_address_selection.include_loopback = on;
        rebuild_client(state)
    }))
}

#[unsafe(no_mangle)]
/// Set whether auto discovered addresses may include IPv6.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live client handle.
pub unsafe extern "C" fn lnd_client_set_include_ipv6(
    handle: *mut LndClientHandle,
    on: bool,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndClientState, LndClientHandle>(handle, "client handle")?;
        state.config.default_address_selection.include_ipv6 = on;
        rebuild_client(state)
    }))
}

#[unsafe(no_mangle)]
/// Set whether auto discovered addresses may include private IPv4.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live client handle.
pub unsafe extern "C" fn lnd_client_set_include_private_ipv4(
    handle: *mut LndClientHandle,
    on: bool,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndClientState, LndClientHandle>(handle, "client handle")?;
        state.config.default_address_selection.include_private_ipv4 = on;
        rebuild_client(state)
    }))
}

#[unsafe(no_mangle)]
/// Set whether auto discovered addresses may include link local IPv4.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live client handle.
pub unsafe extern "C" fn lnd_client_set_include_link_local_ipv4(
    handle: *mut LndClientHandle,
    on: bool,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndClientState, LndClientHandle>(handle, "client handle")?;
        state
            .config
            .default_address_selection
            .include_link_local_ipv4 = on;
        rebuild_client(state)
    }))
}

#[unsafe(no_mangle)]
/// Allow only a named interface for auto discovered addresses.
///
/// Once at least one interface is allowed, only allowlisted interfaces are
/// considered. Deny rules still take precedence.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live client handle.
/// `interface_name` must be valid UTF-8.
pub unsafe extern "C" fn lnd_client_enable_interface(
    handle: *mut LndClientHandle,
    interface_name: *const c_char,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndClientState, LndClientHandle>(handle, "client handle")?;
        state
            .config
            .default_address_selection
            .interface_allowlist
            .push(read_cstr(interface_name, "interface_name")?);
        rebuild_client(state)
    }))
}

#[unsafe(no_mangle)]
/// Deny a named interface for auto discovered addresses.
///
/// Denied interfaces are ignored even when they are also present in the
/// allowlist.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live client handle.
/// `interface_name` must be valid UTF-8.
pub unsafe extern "C" fn lnd_client_disable_interface(
    handle: *mut LndClientHandle,
    interface_name: *const c_char,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndClientState, LndClientHandle>(handle, "client handle")?;
        state
            .config
            .default_address_selection
            .interface_denylist
            .push(read_cstr(interface_name, "interface_name")?);
        rebuild_client(state)
    }))
}

#[unsafe(no_mangle)]
/// Clear client interface allow and deny filters.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live client handle.
pub unsafe extern "C" fn lnd_client_clear_interface_filters(handle: *mut LndClientHandle) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndClientState, LndClientHandle>(handle, "client handle")?;
        state
            .config
            .default_address_selection
            .interface_allowlist
            .clear();
        state
            .config
            .default_address_selection
            .interface_denylist
            .clear();
        rebuild_client(state)
    }))
}

#[unsafe(no_mangle)]
/// Create a discovery filter handle.
///
/// `network_id` is required. Service and tag constraints can be added later.
///
/// Returns a new handle on success, or `NULL` on failure. Inspect
/// `lnd_last_error()` when `NULL` is returned.
///
/// # Safety
/// `network_id` must be valid UTF-8.
pub unsafe extern "C" fn lnd_discovery_filter_new(
    network_id: *const c_char,
) -> *mut LndDiscoveryFilterHandle {
    ptr_result(catch_ffi(|| {
        let filter = DiscoveryFilter::new(read_cstr(network_id, "network_id")?);
        Ok(into_handle::<
            LndDiscoveryFilterState,
            LndDiscoveryFilterHandle,
        >(LndDiscoveryFilterState { filter }))
    }))
}

#[unsafe(no_mangle)]
/// Free a discovery filter handle.
///
/// It is safe to pass `NULL`.
///
/// # Safety
/// `handle` must be null or a live discovery filter handle.
pub unsafe extern "C" fn lnd_discovery_filter_free(handle: *mut LndDiscoveryFilterHandle) {
    free_handle::<LndDiscoveryFilterState, LndDiscoveryFilterHandle>(handle);
}

#[unsafe(no_mangle)]
/// Set the discovery service filter.
///
/// The resulting filter matches only nodes that advertise this service.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live discovery filter handle.
/// `service` must be valid UTF-8.
pub unsafe extern "C" fn lnd_discovery_filter_set_service(
    handle: *mut LndDiscoveryFilterHandle,
    service: *const c_char,
) -> bool {
    bool_result(catch_ffi(|| {
        let state =
            cast_mut::<LndDiscoveryFilterState, LndDiscoveryFilterHandle>(handle, "filter handle")?;
        state.filter.service = Some(read_cstr(service, "service")?);
        Ok(())
    }))
}

#[unsafe(no_mangle)]
/// Clear the discovery service filter.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live discovery filter handle.
pub unsafe extern "C" fn lnd_discovery_filter_clear_service(
    handle: *mut LndDiscoveryFilterHandle,
) -> bool {
    bool_result(catch_ffi(|| {
        let state =
            cast_mut::<LndDiscoveryFilterState, LndDiscoveryFilterHandle>(handle, "filter handle")?;
        state.filter.service = None;
        Ok(())
    }))
}

#[unsafe(no_mangle)]
/// Add one discovery tag filter.
///
/// A node must contain every tag added to the filter to match.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live discovery filter handle.
/// `tag` must be valid UTF-8.
pub unsafe extern "C" fn lnd_discovery_filter_add_tag(
    handle: *mut LndDiscoveryFilterHandle,
    tag: *const c_char,
) -> bool {
    bool_result(catch_ffi(|| {
        let state =
            cast_mut::<LndDiscoveryFilterState, LndDiscoveryFilterHandle>(handle, "filter handle")?;
        state.filter.tags.push(read_cstr(tag, "tag")?);
        Ok(())
    }))
}

#[unsafe(no_mangle)]
/// Clear discovery tag filters.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live discovery filter handle.
pub unsafe extern "C" fn lnd_discovery_filter_clear_tags(
    handle: *mut LndDiscoveryFilterHandle,
) -> bool {
    bool_result(catch_ffi(|| {
        let state =
            cast_mut::<LndDiscoveryFilterState, LndDiscoveryFilterHandle>(handle, "filter handle")?;
        state.filter.tags.clear();
        Ok(())
    }))
}

#[unsafe(no_mangle)]
/// Run a one-shot discovery request from a filter handle and return JSON.
///
/// The returned JSON has the same shape as the Rust and higher level bindings,
/// typically `{ "nodes": [...], "cursor": 123 }` internally before wrappers
/// flatten the result. The exact schema should be treated as the public API of
/// the server, not as a C struct layout.
///
/// Returns a newly allocated UTF-8 string on success. On failure returns `NULL`
/// and stores a message in `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live client handle.
/// `filter` must be a live discovery filter handle.
/// The returned pointer must be released with `lnd_string_free`.
pub unsafe extern "C" fn lnd_discover(
    handle: *mut LndClientHandle,
    filter: *const LndDiscoveryFilterHandle,
) -> *mut c_char {
    ptr_result(catch_ffi(|| {
        let state = cast_ref::<LndClientState, LndClientHandle>(handle, "client handle")?;
        let filter_state =
            cast_ref::<LndDiscoveryFilterState, LndDiscoveryFilterHandle>(filter, "filter handle")?;
        client_discover_json(state.client.clone(), filter_state.filter.clone())
    }))
}

#[unsafe(no_mangle)]
/// Run a one-shot discovery request and return a newly allocated JSON string.
///
/// This variant accepts the filter as a JSON document instead of an opaque
/// handle. It is convenient for FFI users that already model requests in JSON.
///
/// Returns a newly allocated UTF-8 string on success. On failure returns `NULL`
/// and stores a message in `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live pointer returned by `lnd_client_new`.
/// `filter_json` must be a valid, null-terminated UTF-8 string.
/// The returned pointer must be released with `lnd_string_free`.
pub unsafe extern "C" fn lnd_discover_json(
    handle: *mut LndClientHandle,
    filter_json: *const c_char,
) -> *mut c_char {
    ptr_result(catch_ffi(|| {
        let state = cast_ref::<LndClientState, LndClientHandle>(handle, "client handle")?;
        let filter = parse_filter_json(&read_cstr(filter_json, "filter_json")?)?;
        client_discover_json(state.client.clone(), filter)
    }))
}

#[unsafe(no_mangle)]
/// Derive one local network_id from the client's default address selection.
///
/// This uses the same automatic selection policy as Rust and other SDKs. When
/// multiple equally valid subnets are visible, the function returns `NULL` and
/// sets `lnd_last_error()`.
///
/// Returns a newly allocated UTF-8 string on success. On failure returns `NULL`
/// and stores a message in `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live client handle.
/// The returned pointer must be released with `lnd_string_free`.
pub unsafe extern "C" fn lnd_resolve_network_id(
    handle: *mut LndClientHandle,
) -> *mut c_char {
    ptr_result(catch_ffi(|| {
        let state = cast_ref::<LndClientState, LndClientHandle>(handle, "client handle")?;
        let network_id = resolve_network_id_with_selection(&state.config.default_address_selection)?;
        Ok(into_c_string(network_id))
    }))
}

#[unsafe(no_mangle)]
/// List all locally derived network_id candidates as a JSON array.
///
/// Each JSON item contains `network_id` and `scope`. This is useful when a
/// higher level binding wants to show candidate subnets to the caller before
/// picking one explicitly.
///
/// Returns a newly allocated UTF-8 string on success. On failure returns `NULL`
/// and stores a message in `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live client handle.
/// The returned pointer must be released with `lnd_string_free`.
pub unsafe extern "C" fn lnd_list_network_id_candidates_json(
    handle: *mut LndClientHandle,
) -> *mut c_char {
    ptr_result(catch_ffi(|| {
        let state = cast_ref::<LndClientState, LndClientHandle>(handle, "client handle")?;
        let candidates = list_network_id_candidates(&state.config.default_address_selection)?;
        let json = serde_json::to_string(&candidates)?;
        Ok(into_c_string(json))
    }))
}

#[unsafe(no_mangle)]
/// Create an announce spec handle.
///
/// The returned spec starts with automatic LAN address discovery enabled and
/// `DEFAULT_TTL_SECS` as its lease duration.
///
/// Returns a new handle on success, or `NULL` on failure. Inspect
/// `lnd_last_error()` when `NULL` is returned.
///
/// # Safety
/// string arguments must be valid UTF-8.
pub unsafe extern "C" fn lnd_announce_spec_new(
    network_id: *const c_char,
    node_id: *const c_char,
    service: *const c_char,
    display_name: *const c_char,
    port: u16,
) -> *mut LndAnnounceSpecHandle {
    ptr_result(catch_ffi(|| {
        let spec = AnnounceSpec::new(
            read_cstr(network_id, "network_id")?,
            read_cstr(node_id, "node_id")?,
            read_cstr(service, "service")?,
            read_cstr(display_name, "display_name")?,
            port,
        );
        Ok(into_handle::<LndAnnounceSpecState, LndAnnounceSpecHandle>(
            LndAnnounceSpecState { spec },
        ))
    }))
}

#[unsafe(no_mangle)]
/// Free an announce spec handle.
///
/// It is safe to pass `NULL`.
///
/// # Safety
/// `handle` must be null or a live announce spec handle.
pub unsafe extern "C" fn lnd_announce_spec_free(handle: *mut LndAnnounceSpecHandle) {
    free_handle::<LndAnnounceSpecState, LndAnnounceSpecHandle>(handle);
}

#[unsafe(no_mangle)]
/// Set the announce network_id.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live announce spec handle.
/// `network_id` must be valid UTF-8.
pub unsafe extern "C" fn lnd_announce_spec_set_network_id(
    handle: *mut LndAnnounceSpecHandle,
    network_id: *const c_char,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndAnnounceSpecState, LndAnnounceSpecHandle>(
            handle,
            "announce spec handle",
        )?;
        state.spec.network_id = read_cstr(network_id, "network_id")?;
        Ok(())
    }))
}

#[unsafe(no_mangle)]
/// Set the announce node_id.
///
/// The node id should remain stable across restarts so other peers can treat
/// the node as the same logical instance.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live announce spec handle.
/// `node_id` must be valid UTF-8.
pub unsafe extern "C" fn lnd_announce_spec_set_node_id(
    handle: *mut LndAnnounceSpecHandle,
    node_id: *const c_char,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndAnnounceSpecState, LndAnnounceSpecHandle>(
            handle,
            "announce spec handle",
        )?;
        state.spec.node_id = read_cstr(node_id, "node_id")?;
        Ok(())
    }))
}

#[unsafe(no_mangle)]
/// Set the announce service name.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live announce spec handle.
/// `service` must be valid UTF-8.
pub unsafe extern "C" fn lnd_announce_spec_set_service(
    handle: *mut LndAnnounceSpecHandle,
    service: *const c_char,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndAnnounceSpecState, LndAnnounceSpecHandle>(
            handle,
            "announce spec handle",
        )?;
        state.spec.service = read_cstr(service, "service")?;
        Ok(())
    }))
}

#[unsafe(no_mangle)]
/// Set the announce display name.
///
/// The display name is intended for humans and does not need to be globally
/// unique.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live announce spec handle.
/// `display_name` must be valid UTF-8.
pub unsafe extern "C" fn lnd_announce_spec_set_display_name(
    handle: *mut LndAnnounceSpecHandle,
    display_name: *const c_char,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndAnnounceSpecState, LndAnnounceSpecHandle>(
            handle,
            "announce spec handle",
        )?;
        state.spec.display_name = read_cstr(display_name, "display_name")?;
        Ok(())
    }))
}

#[unsafe(no_mangle)]
/// Set the announce service port.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live announce spec handle.
pub unsafe extern "C" fn lnd_announce_spec_set_port(
    handle: *mut LndAnnounceSpecHandle,
    port: u16,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndAnnounceSpecState, LndAnnounceSpecHandle>(
            handle,
            "announce spec handle",
        )?;
        state.spec.port = port;
        Ok(())
    }))
}

#[unsafe(no_mangle)]
/// Set whether auto address discovery is enabled.
///
/// When enabled, the client combines eligible local interface addresses with
/// any explicit LAN addresses attached to the spec.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live announce spec handle.
pub unsafe extern "C" fn lnd_announce_spec_set_auto_lan_addrs(
    handle: *mut LndAnnounceSpecHandle,
    on: bool,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndAnnounceSpecState, LndAnnounceSpecHandle>(
            handle,
            "announce spec handle",
        )?;
        state.spec.auto_lan_addrs = on;
        Ok(())
    }))
}

#[unsafe(no_mangle)]
/// Add one explicit LAN address.
///
/// The address should be passed in `host:port` form.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live announce spec handle.
/// `addr` must be valid UTF-8.
pub unsafe extern "C" fn lnd_announce_spec_add_lan_addr(
    handle: *mut LndAnnounceSpecHandle,
    addr: *const c_char,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndAnnounceSpecState, LndAnnounceSpecHandle>(
            handle,
            "announce spec handle",
        )?;
        let values = parse_socket_addrs(&[read_cstr(addr, "addr")?], state.spec.port)
            .map_err(|error| ClientError::Api(error.to_string()))?;
        state
            .spec
            .lan_addrs
            .get_or_insert_with(Vec::new)
            .extend(values);
        Ok(())
    }))
}

#[unsafe(no_mangle)]
/// Clear explicit LAN addresses.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live announce spec handle.
pub unsafe extern "C" fn lnd_announce_spec_clear_lan_addrs(
    handle: *mut LndAnnounceSpecHandle,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndAnnounceSpecState, LndAnnounceSpecHandle>(
            handle,
            "announce spec handle",
        )?;
        state.spec.lan_addrs = None;
        Ok(())
    }))
}

#[unsafe(no_mangle)]
/// Set whether auto discovered addresses may include loopback.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live announce spec handle.
pub unsafe extern "C" fn lnd_announce_spec_set_include_loopback(
    handle: *mut LndAnnounceSpecHandle,
    on: bool,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndAnnounceSpecState, LndAnnounceSpecHandle>(
            handle,
            "announce spec handle",
        )?;
        ensure_spec_selection(&mut state.spec).include_loopback = on;
        Ok(())
    }))
}

#[unsafe(no_mangle)]
/// Set whether auto discovered addresses may include IPv6.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live announce spec handle.
pub unsafe extern "C" fn lnd_announce_spec_set_include_ipv6(
    handle: *mut LndAnnounceSpecHandle,
    on: bool,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndAnnounceSpecState, LndAnnounceSpecHandle>(
            handle,
            "announce spec handle",
        )?;
        ensure_spec_selection(&mut state.spec).include_ipv6 = on;
        Ok(())
    }))
}

#[unsafe(no_mangle)]
/// Set whether auto discovered addresses may include private IPv4.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live announce spec handle.
pub unsafe extern "C" fn lnd_announce_spec_set_include_private_ipv4(
    handle: *mut LndAnnounceSpecHandle,
    on: bool,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndAnnounceSpecState, LndAnnounceSpecHandle>(
            handle,
            "announce spec handle",
        )?;
        ensure_spec_selection(&mut state.spec).include_private_ipv4 = on;
        Ok(())
    }))
}

#[unsafe(no_mangle)]
/// Set whether auto discovered addresses may include link local IPv4.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live announce spec handle.
pub unsafe extern "C" fn lnd_announce_spec_set_include_link_local_ipv4(
    handle: *mut LndAnnounceSpecHandle,
    on: bool,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndAnnounceSpecState, LndAnnounceSpecHandle>(
            handle,
            "announce spec handle",
        )?;
        ensure_spec_selection(&mut state.spec).include_link_local_ipv4 = on;
        Ok(())
    }))
}

#[unsafe(no_mangle)]
/// Allow only a named interface for auto discovered announce addresses.
///
/// Once at least one interface is allowed, only allowlisted interfaces are
/// considered. Deny rules still take precedence.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live announce spec handle.
/// `interface_name` must be valid UTF-8.
pub unsafe extern "C" fn lnd_announce_spec_enable_interface(
    handle: *mut LndAnnounceSpecHandle,
    interface_name: *const c_char,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndAnnounceSpecState, LndAnnounceSpecHandle>(
            handle,
            "announce spec handle",
        )?;
        ensure_spec_selection(&mut state.spec)
            .interface_allowlist
            .push(read_cstr(interface_name, "interface_name")?);
        Ok(())
    }))
}

#[unsafe(no_mangle)]
/// Deny a named interface for auto discovered announce addresses.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live announce spec handle.
/// `interface_name` must be valid UTF-8.
pub unsafe extern "C" fn lnd_announce_spec_disable_interface(
    handle: *mut LndAnnounceSpecHandle,
    interface_name: *const c_char,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndAnnounceSpecState, LndAnnounceSpecHandle>(
            handle,
            "announce spec handle",
        )?;
        ensure_spec_selection(&mut state.spec)
            .interface_denylist
            .push(read_cstr(interface_name, "interface_name")?);
        Ok(())
    }))
}

#[unsafe(no_mangle)]
/// Clear announce interface allow and deny filters.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live announce spec handle.
pub unsafe extern "C" fn lnd_announce_spec_clear_interface_filters(
    handle: *mut LndAnnounceSpecHandle,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndAnnounceSpecState, LndAnnounceSpecHandle>(
            handle,
            "announce spec handle",
        )?;
        let selection = ensure_spec_selection(&mut state.spec);
        selection.interface_allowlist.clear();
        selection.interface_denylist.clear();
        Ok(())
    }))
}

#[unsafe(no_mangle)]
/// Add one announce tag.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live announce spec handle.
/// `tag` must be valid UTF-8.
pub unsafe extern "C" fn lnd_announce_spec_add_tag(
    handle: *mut LndAnnounceSpecHandle,
    tag: *const c_char,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndAnnounceSpecState, LndAnnounceSpecHandle>(
            handle,
            "announce spec handle",
        )?;
        state.spec.tags.push(read_cstr(tag, "tag")?);
        Ok(())
    }))
}

#[unsafe(no_mangle)]
/// Clear announce tags.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live announce spec handle.
pub unsafe extern "C" fn lnd_announce_spec_clear_tags(handle: *mut LndAnnounceSpecHandle) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndAnnounceSpecState, LndAnnounceSpecHandle>(
            handle,
            "announce spec handle",
        )?;
        state.spec.tags.clear();
        Ok(())
    }))
}

#[unsafe(no_mangle)]
/// Insert one announce metadata key/value pair.
///
/// Later calls with the same key replace the previous value.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live announce spec handle.
/// string arguments must be valid UTF-8.
pub unsafe extern "C" fn lnd_announce_spec_insert_metadata(
    handle: *mut LndAnnounceSpecHandle,
    key: *const c_char,
    value: *const c_char,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndAnnounceSpecState, LndAnnounceSpecHandle>(
            handle,
            "announce spec handle",
        )?;
        state
            .spec
            .metadata
            .insert(read_cstr(key, "key")?, read_cstr(value, "value")?);
        Ok(())
    }))
}

#[unsafe(no_mangle)]
/// Clear announce metadata.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live announce spec handle.
pub unsafe extern "C" fn lnd_announce_spec_clear_metadata(
    handle: *mut LndAnnounceSpecHandle,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndAnnounceSpecState, LndAnnounceSpecHandle>(
            handle,
            "announce spec handle",
        )?;
        state.spec.metadata.clear();
        Ok(())
    }))
}

#[unsafe(no_mangle)]
/// Set the announce TTL in seconds.
///
/// The background announce loop renews around every third of this value.
///
/// Returns `true` on success. On failure returns `false` and sets
/// `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live announce spec handle.
pub unsafe extern "C" fn lnd_announce_spec_set_ttl_secs(
    handle: *mut LndAnnounceSpecHandle,
    ttl_secs: u64,
) -> bool {
    bool_result(catch_ffi(|| {
        let state = cast_mut::<LndAnnounceSpecState, LndAnnounceSpecHandle>(
            handle,
            "announce spec handle",
        )?;
        state.spec.ttl_secs = ttl_secs;
        Ok(())
    }))
}

#[unsafe(no_mangle)]
/// Resolve the announce addresses from a client config and announce spec and return JSON.
///
/// The result is a JSON array of deduplicated `host:port` strings. It is useful
/// when higher level code wants to inspect or override the final LAN addresses
/// before registration.
///
/// Returns a newly allocated UTF-8 string on success. On failure returns `NULL`
/// and stores a message in `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live client handle.
/// `spec` must be a live announce spec handle.
/// The returned pointer must be released with `lnd_string_free`.
pub unsafe extern "C" fn lnd_resolve_announce_addrs_json(
    handle: *mut LndClientHandle,
    spec: *const LndAnnounceSpecHandle,
) -> *mut c_char {
    ptr_result(catch_ffi(|| {
        let state = cast_ref::<LndClientState, LndClientHandle>(handle, "client handle")?;
        let spec_state =
            cast_ref::<LndAnnounceSpecState, LndAnnounceSpecHandle>(spec, "announce spec handle")?;
        let addrs = resolve_announce_addrs_with_defaults(
            &spec_state.spec,
            &state.config.default_address_selection,
        )?;
        let json = serde_json::to_string(&addrs)?;
        Ok(into_c_string(json))
    }))
}

#[unsafe(no_mangle)]
/// Run one announce request from an announce spec handle and return JSON.
///
/// The returned JSON is the normalized node record produced by the server after
/// lease metadata is attached.
///
/// Returns a newly allocated UTF-8 string on success. On failure returns `NULL`
/// and stores a message in `lnd_last_error()`.
///
/// # Safety
/// `handle` must be a live client handle.
/// `spec` must be a live announce spec handle.
/// The returned pointer must be released with `lnd_string_free`.
pub unsafe extern "C" fn lnd_announce_once(
    handle: *mut LndClientHandle,
    spec: *const LndAnnounceSpecHandle,
) -> *mut c_char {
    ptr_result(catch_ffi(|| {
        let state = cast_ref::<LndClientState, LndClientHandle>(handle, "client handle")?;
        let spec_state =
            cast_ref::<LndAnnounceSpecState, LndAnnounceSpecHandle>(spec, "announce spec handle")?;
        client_announce_once_json(
            state.client.clone(),
            state.config.clone(),
            spec_state.spec.clone(),
        )
    }))
}

#[unsafe(no_mangle)]
/// Start a background announce loop from an announce spec handle.
///
/// The loop renews the server lease until `lnd_announce_stop` is called.
///
/// Returns a handle on success, or `NULL` on failure. Inspect `lnd_last_error()`
/// when `NULL` is returned.
///
/// # Safety
/// `handle` must be a live client handle.
/// `spec` must be a live announce spec handle.
pub unsafe extern "C" fn lnd_announce_start_with_spec(
    handle: *mut LndClientHandle,
    spec: *const LndAnnounceSpecHandle,
) -> *mut LndAnnounceHandle {
    ptr_result(catch_ffi(|| {
        let state = cast_ref::<LndClientState, LndClientHandle>(handle, "client handle")?;
        let spec_state =
            cast_ref::<LndAnnounceSpecState, LndAnnounceSpecHandle>(spec, "announce spec handle")?;
        client_announce_start(state.client.clone(), spec_state.spec.clone())
    }))
}

#[unsafe(no_mangle)]
/// Start a background announce loop from a JSON spec.
///
/// This variant accepts the announce spec as a UTF-8 JSON document instead of
/// an opaque handle.
///
/// Returns a handle on success, or `NULL` on failure. Inspect `lnd_last_error()`
/// when `NULL` is returned.
///
/// # Safety
/// `handle` must be a live pointer returned by `lnd_client_new`.
/// `announce_json` must be a valid, null-terminated UTF-8 string.
/// The returned pointer must be released with `lnd_announce_stop`.
pub unsafe extern "C" fn lnd_announce_start(
    handle: *mut LndClientHandle,
    announce_json: *const c_char,
) -> *mut LndAnnounceHandle {
    ptr_result(catch_ffi(|| {
        let state = cast_ref::<LndClientState, LndClientHandle>(handle, "client handle")?;
        let spec = parse_announce_json(&read_cstr(announce_json, "announce_json")?)?;
        client_announce_start(state.client.clone(), spec)
    }))
}

#[unsafe(no_mangle)]
/// Stop and free an announce handle.
///
/// It is safe to pass `NULL`. After this call the handle must not be reused.
///
/// # Safety
/// `handle` must be null or a pointer returned by this library that has not been stopped yet.
pub unsafe extern "C" fn lnd_announce_stop(handle: *mut LndAnnounceHandle) {
    if handle.is_null() {
        return;
    }
    if let Err(error) = catch_ffi(|| {
        let state = cast_mut::<LndAnnounceState, LndAnnounceHandle>(handle, "announce handle")?;
        if let Some(announce_handle) = state.handle.take() {
            block_on_ffi(async move { announce_handle.stop().await })?;
        }
        free_handle::<LndAnnounceState, LndAnnounceHandle>(handle);
        Ok(())
    }) {
        set_last_error(error);
    }
}

#[unsafe(no_mangle)]
/// Start a background watch stream from a discovery filter handle.
///
/// Each callback receives one UTF-8 JSON event envelope. Callers should copy the
/// payload inside the callback if it must be retained.
///
/// Returns a handle on success, or `NULL` on failure. Inspect `lnd_last_error()`
/// when `NULL` is returned.
///
/// # Safety
/// `handle` must be a live client handle.
/// `filter` must be a live discovery filter handle.
/// `callback` must remain valid until `lnd_watch_stop` is called.
pub unsafe extern "C" fn lnd_watch_start_with_filter(
    handle: *mut LndClientHandle,
    filter: *const LndDiscoveryFilterHandle,
    callback: LndWatchCallback,
    user_data: *mut c_void,
) -> *mut LndWatchHandle {
    ptr_result(catch_ffi(|| {
        let state = cast_ref::<LndClientState, LndClientHandle>(handle, "client handle")?;
        let filter_state =
            cast_ref::<LndDiscoveryFilterState, LndDiscoveryFilterHandle>(filter, "filter handle")?;
        client_watch_start(
            state.client.clone(),
            filter_state.filter.clone(),
            callback,
            user_data,
        )
    }))
}

#[unsafe(no_mangle)]
/// Start a background watch stream and invoke the callback for every JSON event.
///
/// This variant accepts the filter as a UTF-8 JSON document instead of an
/// opaque handle.
///
/// Returns a handle on success, or `NULL` on failure. Inspect `lnd_last_error()`
/// when `NULL` is returned.
///
/// # Safety
/// `handle` must be a live pointer returned by `lnd_client_new`.
/// `filter_json` must be a valid, null-terminated UTF-8 string.
/// `callback` must remain valid until `lnd_watch_stop` is called.
/// `user_data` is passed through to the callback without validation.
pub unsafe extern "C" fn lnd_watch_start(
    handle: *mut LndClientHandle,
    filter_json: *const c_char,
    callback: LndWatchCallback,
    user_data: *mut c_void,
) -> *mut LndWatchHandle {
    ptr_result(catch_ffi(|| {
        let state = cast_ref::<LndClientState, LndClientHandle>(handle, "client handle")?;
        let filter = parse_filter_json(&read_cstr(filter_json, "filter_json")?)?;
        client_watch_start(state.client.clone(), filter, callback, user_data)
    }))
}

#[unsafe(no_mangle)]
/// Stop and free a watch handle.
///
/// It is safe to pass `NULL`. After this call the handle must not be reused.
///
/// # Safety
/// `handle` must be null or a pointer returned by this library that has not been stopped yet.
pub unsafe extern "C" fn lnd_watch_stop(handle: *mut LndWatchHandle) {
    if handle.is_null() {
        return;
    }
    if let Err(error) = catch_ffi(|| {
        let state = cast_mut::<LndWatchState, LndWatchHandle>(handle, "watch handle")?;
        if let Some(stop_tx) = state.stop_tx.take() {
            let _ = stop_tx.send(());
        }
        if let Some(join_handle) = state.join_handle.take() {
            let _ = join_handle.join();
        }
        free_handle::<LndWatchState, LndWatchHandle>(handle);
        Ok(())
    }) {
        set_last_error(error);
    }
}

#[unsafe(no_mangle)]
/// Free a C string returned by this library.
///
/// This must be called for every non null string returned by functions such as
/// `lnd_discover`, `lnd_announce_once` and `lnd_resolve_announce_addrs_json`.
///
/// # Safety
/// `ptr` must be null or a pointer previously returned by this library via `CString::into_raw`.
pub unsafe extern "C" fn lnd_string_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
}

#[unsafe(no_mangle)]
/// Return the last thread local error message produced by this library.
///
/// The pointer is borrowed and must not be freed by the caller. It may be
/// overwritten by later calls from the same thread.
pub extern "C" fn lnd_last_error() -> *const c_char {
    if let Ok(slot) = LAST_ERROR.lock()
        && let Some(error) = slot.as_ref()
    {
        return error.as_ptr();
    }
    ptr::null()
}
