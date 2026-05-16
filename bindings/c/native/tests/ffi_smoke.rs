mod common;

use std::ffi::{CStr, CString, c_char};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use common::{TestServer, sample_spec};
use lnd::ffi::{
    lnd_announce_once, lnd_announce_spec_add_tag, lnd_announce_spec_free,
    lnd_announce_spec_insert_metadata, lnd_announce_spec_new, lnd_announce_spec_set_auto_lan_addrs,
    lnd_announce_spec_set_include_loopback, lnd_announce_spec_set_ttl_secs,
    lnd_announce_start_with_spec, lnd_announce_stop, lnd_client_enable_interface, lnd_client_free,
    lnd_client_new_default, lnd_client_set_bearer_token, lnd_client_set_include_loopback,
    lnd_client_set_server_url, lnd_discover, lnd_filter_add_tag, lnd_filter_free, lnd_filter_new,
    lnd_filter_set_service, lnd_last_error, lnd_string_free, lnd_watch_start_with_filter,
    lnd_watch_stop,
};

static EVENTS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

extern "C" fn watch_callback(payload: *const c_char, _user_data: *mut std::ffi::c_void) {
    let payload = unsafe { CStr::from_ptr(payload) }
        .to_str()
        .unwrap()
        .to_string();
    EVENTS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap()
        .push(payload);
}

#[tokio::test]
async fn ffi_discover_and_watch_work() {
    let server = TestServer::spawn().await.unwrap();
    let server_url = CString::new(format!("http://{}", server.addr)).unwrap();
    let token = CString::new(server.bearer_token.clone()).unwrap();
    let iface = CString::new("lo0").unwrap();
    let discovery_domain = CString::new("prod").unwrap();
    let service = CString::new("svc").unwrap();
    let tag = CString::new("alpha").unwrap();
    let display_name = CString::new("node-ffi").unwrap();
    let node_id = CString::new("node-ffi").unwrap();
    let meta_key = CString::new("role").unwrap();
    let meta_value = CString::new("api").unwrap();

    let client = unsafe { lnd_client_new_default() };
    assert!(!client.is_null(), "ffi client init failed");
    assert!(unsafe { lnd_client_set_server_url(client, server_url.as_ptr()) });
    assert!(unsafe { lnd_client_set_bearer_token(client, token.as_ptr()) });
    assert!(unsafe { lnd_client_set_include_loopback(client, true) });
    assert!(unsafe { lnd_client_enable_interface(client, iface.as_ptr()) });

    let filter = unsafe { lnd_filter_new(discovery_domain.as_ptr()) };
    assert!(!filter.is_null(), "ffi filter init failed");
    assert!(unsafe { lnd_filter_set_service(filter, service.as_ptr()) });
    assert!(unsafe { lnd_filter_add_tag(filter, tag.as_ptr()) });

    let watch = unsafe {
        lnd_watch_start_with_filter(client, filter, watch_callback, std::ptr::null_mut())
    };
    assert!(!watch.is_null(), "ffi watch init failed");

    let announce = unsafe {
        lnd_announce_spec_new(
            discovery_domain.as_ptr(),
            node_id.as_ptr(),
            service.as_ptr(),
            display_name.as_ptr(),
            8080,
        )
    };
    assert!(!announce.is_null(), "ffi announce spec init failed");
    assert!(unsafe { lnd_announce_spec_set_auto_lan_addrs(announce, true) });
    assert!(unsafe { lnd_announce_spec_set_include_loopback(announce, true) });
    assert!(unsafe {
        lnd_announce_spec_set_ttl_secs(announce, sample_spec("node-ffi", 30).ttl_secs)
    });
    assert!(unsafe { lnd_announce_spec_add_tag(announce, tag.as_ptr()) });
    assert!(unsafe {
        lnd_announce_spec_insert_metadata(announce, meta_key.as_ptr(), meta_value.as_ptr())
    });

    let once_ptr = unsafe { lnd_announce_once(client, announce) };
    assert!(
        !once_ptr.is_null(),
        "ffi announce once failed: {}",
        last_error_string()
    );
    unsafe { lnd_string_free(once_ptr) };

    let announce_handle = unsafe { lnd_announce_start_with_spec(client, announce) };
    assert!(!announce_handle.is_null(), "ffi announce init failed");

    tokio::time::sleep(Duration::from_secs(2)).await;

    let discovered_ptr = unsafe { lnd_discover(client, filter) };
    assert!(
        !discovered_ptr.is_null(),
        "ffi discover failed: {}",
        last_error_string()
    );
    let discovered = unsafe { CStr::from_ptr(discovered_ptr) }
        .to_str()
        .unwrap()
        .to_string();
    assert!(discovered.contains("node-ffi"));

    let events = EVENTS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap()
        .clone();
    assert!(!events.is_empty());

    unsafe {
        lnd_string_free(discovered_ptr);
        lnd_announce_stop(announce_handle);
        lnd_watch_stop(watch);
        lnd_announce_spec_free(announce);
        lnd_filter_free(filter);
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
