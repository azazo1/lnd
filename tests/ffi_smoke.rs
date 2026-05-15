mod common;

use std::ffi::{CStr, CString, c_char};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use common::{TestServer, sample_spec};
use lnd::ffi::{
    lnd_announce_start, lnd_announce_stop, lnd_client_free, lnd_client_new, lnd_discover_json,
    lnd_last_error, lnd_string_free, lnd_watch_start, lnd_watch_stop,
};

static EVENTS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

extern "C" fn watch_callback(payload: *const c_char, _user_data: *mut std::ffi::c_void) {
    let payload = unsafe { CStr::from_ptr(payload) }.to_str().unwrap().to_string();
    EVENTS.get_or_init(|| Mutex::new(Vec::new())).lock().unwrap().push(payload);
}

#[tokio::test]
async fn ffi_discover_and_watch_work() {
    let server = TestServer::spawn().await.unwrap();
    let server_url = CString::new(format!("http://{}", server.addr)).unwrap();
    let token = CString::new(server.bearer_token.clone()).unwrap();
    let filter = CString::new(r#"{"network_id":"net-a","service":"svc","tags":["alpha"]}"#).unwrap();
    let announce = CString::new(
        serde_json::to_string(&sample_spec("node-ffi", 30)).unwrap(),
    )
    .unwrap();

    let client = unsafe { lnd_client_new(server_url.as_ptr(), token.as_ptr()) };
    assert!(!client.is_null(), "ffi client init failed");

    let watch = unsafe { lnd_watch_start(client, filter.as_ptr(), Some(watch_callback), std::ptr::null_mut()) };
    assert!(!watch.is_null(), "ffi watch init failed");

    let announce_handle = unsafe { lnd_announce_start(client, announce.as_ptr()) };
    assert!(!announce_handle.is_null(), "ffi announce init failed");

    tokio::time::sleep(Duration::from_secs(2)).await;

    let discovered_ptr = unsafe { lnd_discover_json(client, filter.as_ptr()) };
    assert!(!discovered_ptr.is_null(), "ffi discover failed: {}", last_error_string());
    let discovered = unsafe { CStr::from_ptr(discovered_ptr) }.to_str().unwrap().to_string();
    assert!(discovered.contains("node-ffi"));

    let events = EVENTS.get_or_init(|| Mutex::new(Vec::new())).lock().unwrap().clone();
    assert!(!events.is_empty());

    unsafe {
        lnd_string_free(discovered_ptr);
        lnd_announce_stop(announce_handle);
        lnd_watch_stop(watch);
        lnd_client_free(client);
    }
}

fn last_error_string() -> String {
    let ptr = lnd_last_error();
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr) }.to_str().unwrap().to_string()
}
