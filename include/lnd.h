#pragma once

/* Generated with cbindgen:0.29.2 */

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

#define DEFAULT_TTL_SECS 30

#define DEFAULT_RENEW_INTERVAL_SECS 10

#define DEFAULT_SSE_KEEPALIVE_SECS 15

#define DEFAULT_EVENT_BUFFER_CAPACITY 4096

typedef struct Option_LndWatchCallback Option_LndWatchCallback;

typedef struct LndClientHandle {
  uint8_t _private[0];
} LndClientHandle;

typedef struct LndDiscoveryFilterHandle {
  uint8_t _private[0];
} LndDiscoveryFilterHandle;

typedef struct LndAnnounceSpecHandle {
  uint8_t _private[0];
} LndAnnounceSpecHandle;

typedef struct LndAnnounceHandle {
  uint8_t _private[0];
} LndAnnounceHandle;

typedef struct LndWatchHandle {
  uint8_t _private[0];
} LndWatchHandle;

typedef void (*LndWatchCallback)(const char*, void*);

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * Create a client handle from default config values.
 *
 * # Safety
 * The returned handle must be released with `lnd_client_free`.
 */
struct LndClientHandle *lnd_client_new_default(void);

/**
 * Create a client handle for later discovery, announce and watch calls.
 *
 * # Safety
 * `server_url` must be a valid, null-terminated UTF-8 string.
 * `bearer_token` may be null, otherwise it must be a valid, null-terminated UTF-8 string.
 */
struct LndClientHandle *lnd_client_new(const char *server_url, const char *bearer_token);

/**
 * Free a client handle.
 *
 * # Safety
 * `handle` must be null or a pointer returned by this library that has not been freed yet.
 */
void lnd_client_free(struct LndClientHandle *handle);

/**
 * Set the client base URL.
 *
 * # Safety
 * `handle` must be a live client handle.
 * `server_url` must be a valid, null-terminated UTF-8 string.
 */
bool lnd_client_set_server_url(struct LndClientHandle *handle, const char *server_url);

/**
 * Set or clear the client bearer token.
 *
 * # Safety
 * `handle` must be a live client handle.
 * `bearer_token` may be null, otherwise it must be valid UTF-8.
 */
bool lnd_client_set_bearer_token(struct LndClientHandle *handle, const char *bearer_token);

/**
 * Set the client request timeout in milliseconds.
 *
 * # Safety
 * `handle` must be a live client handle.
 */
bool lnd_client_set_timeout_ms(struct LndClientHandle *handle, uint64_t timeout_ms);

/**
 * Set reconnect backoff bounds in milliseconds.
 *
 * # Safety
 * `handle` must be a live client handle.
 */
bool lnd_client_set_reconnect_backoff_ms(struct LndClientHandle *handle,
                                         uint64_t min_ms,
                                         uint64_t max_ms);

/**
 * Set whether auto discovered addresses may include loopback interfaces.
 *
 * # Safety
 * `handle` must be a live client handle.
 */
bool lnd_client_set_include_loopback(struct LndClientHandle *handle, bool on);

/**
 * Set whether auto discovered addresses may include IPv6.
 *
 * # Safety
 * `handle` must be a live client handle.
 */
bool lnd_client_set_include_ipv6(struct LndClientHandle *handle, bool on);

/**
 * Set whether auto discovered addresses may include private IPv4.
 *
 * # Safety
 * `handle` must be a live client handle.
 */
bool lnd_client_set_include_private_ipv4(struct LndClientHandle *handle, bool on);

/**
 * Set whether auto discovered addresses may include link local IPv4.
 *
 * # Safety
 * `handle` must be a live client handle.
 */
bool lnd_client_set_include_link_local_ipv4(struct LndClientHandle *handle, bool on);

/**
 * Allow only a named interface for auto discovered addresses.
 *
 * # Safety
 * `handle` must be a live client handle.
 * `interface_name` must be valid UTF-8.
 */
bool lnd_client_enable_interface(struct LndClientHandle *handle, const char *interface_name);

/**
 * Deny a named interface for auto discovered addresses.
 *
 * # Safety
 * `handle` must be a live client handle.
 * `interface_name` must be valid UTF-8.
 */
bool lnd_client_disable_interface(struct LndClientHandle *handle, const char *interface_name);

/**
 * Clear client interface allow and deny filters.
 *
 * # Safety
 * `handle` must be a live client handle.
 */
bool lnd_client_clear_interface_filters(struct LndClientHandle *handle);

/**
 * Create a discovery filter handle.
 *
 * # Safety
 * `network_id` must be valid UTF-8.
 */
struct LndDiscoveryFilterHandle *lnd_discovery_filter_new(const char *network_id);

/**
 * Free a discovery filter handle.
 *
 * # Safety
 * `handle` must be null or a live discovery filter handle.
 */
void lnd_discovery_filter_free(struct LndDiscoveryFilterHandle *handle);

/**
 * Set the discovery service filter.
 *
 * # Safety
 * `handle` must be a live discovery filter handle.
 * `service` must be valid UTF-8.
 */
bool lnd_discovery_filter_set_service(struct LndDiscoveryFilterHandle *handle, const char *service);

/**
 * Clear the discovery service filter.
 *
 * # Safety
 * `handle` must be a live discovery filter handle.
 */
bool lnd_discovery_filter_clear_service(struct LndDiscoveryFilterHandle *handle);

/**
 * Add one discovery tag filter.
 *
 * # Safety
 * `handle` must be a live discovery filter handle.
 * `tag` must be valid UTF-8.
 */
bool lnd_discovery_filter_add_tag(struct LndDiscoveryFilterHandle *handle, const char *tag);

/**
 * Clear discovery tag filters.
 *
 * # Safety
 * `handle` must be a live discovery filter handle.
 */
bool lnd_discovery_filter_clear_tags(struct LndDiscoveryFilterHandle *handle);

/**
 * Run a one-shot discovery request from a filter handle and return JSON.
 *
 * # Safety
 * `handle` must be a live client handle.
 * `filter` must be a live discovery filter handle.
 * The returned pointer must be released with `lnd_string_free`.
 */
char *lnd_discover(struct LndClientHandle *handle, const struct LndDiscoveryFilterHandle *filter);

/**
 * Run a one-shot discovery request and return a newly allocated JSON string.
 *
 * # Safety
 * `handle` must be a live pointer returned by `lnd_client_new`.
 * `filter_json` must be a valid, null-terminated UTF-8 string.
 * The returned pointer must be released with `lnd_string_free`.
 */
char *lnd_discover_json(struct LndClientHandle *handle, const char *filter_json);

/**
 * Create an announce spec handle.
 *
 * # Safety
 * string arguments must be valid UTF-8.
 */
struct LndAnnounceSpecHandle *lnd_announce_spec_new(const char *network_id,
                                                    const char *node_id,
                                                    const char *service,
                                                    const char *display_name,
                                                    uint16_t port);

/**
 * Free an announce spec handle.
 *
 * # Safety
 * `handle` must be null or a live announce spec handle.
 */
void lnd_announce_spec_free(struct LndAnnounceSpecHandle *handle);

/**
 * Set the announce network_id.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 * `network_id` must be valid UTF-8.
 */
bool lnd_announce_spec_set_network_id(struct LndAnnounceSpecHandle *handle, const char *network_id);

/**
 * Set the announce node_id.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 * `node_id` must be valid UTF-8.
 */
bool lnd_announce_spec_set_node_id(struct LndAnnounceSpecHandle *handle, const char *node_id);

/**
 * Set the announce service name.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 * `service` must be valid UTF-8.
 */
bool lnd_announce_spec_set_service(struct LndAnnounceSpecHandle *handle, const char *service);

/**
 * Set the announce display name.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 * `display_name` must be valid UTF-8.
 */
bool lnd_announce_spec_set_display_name(struct LndAnnounceSpecHandle *handle,
                                        const char *display_name);

/**
 * Set the announce service port.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 */
bool lnd_announce_spec_set_port(struct LndAnnounceSpecHandle *handle, uint16_t port);

/**
 * Set whether auto address discovery is enabled.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 */
bool lnd_announce_spec_set_auto_lan_addrs(struct LndAnnounceSpecHandle *handle, bool on);

/**
 * Add one explicit LAN address.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 * `addr` must be valid UTF-8.
 */
bool lnd_announce_spec_add_lan_addr(struct LndAnnounceSpecHandle *handle, const char *addr);

/**
 * Clear explicit LAN addresses.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 */
bool lnd_announce_spec_clear_lan_addrs(struct LndAnnounceSpecHandle *handle);

/**
 * Set whether auto discovered addresses may include loopback.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 */
bool lnd_announce_spec_set_include_loopback(struct LndAnnounceSpecHandle *handle, bool on);

/**
 * Set whether auto discovered addresses may include IPv6.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 */
bool lnd_announce_spec_set_include_ipv6(struct LndAnnounceSpecHandle *handle, bool on);

/**
 * Set whether auto discovered addresses may include private IPv4.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 */
bool lnd_announce_spec_set_include_private_ipv4(struct LndAnnounceSpecHandle *handle, bool on);

/**
 * Set whether auto discovered addresses may include link local IPv4.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 */
bool lnd_announce_spec_set_include_link_local_ipv4(struct LndAnnounceSpecHandle *handle, bool on);

/**
 * Allow only a named interface for auto discovered announce addresses.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 * `interface_name` must be valid UTF-8.
 */
bool lnd_announce_spec_enable_interface(struct LndAnnounceSpecHandle *handle,
                                        const char *interface_name);

/**
 * Deny a named interface for auto discovered announce addresses.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 * `interface_name` must be valid UTF-8.
 */
bool lnd_announce_spec_disable_interface(struct LndAnnounceSpecHandle *handle,
                                         const char *interface_name);

/**
 * Clear announce interface allow and deny filters.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 */
bool lnd_announce_spec_clear_interface_filters(struct LndAnnounceSpecHandle *handle);

/**
 * Add one announce tag.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 * `tag` must be valid UTF-8.
 */
bool lnd_announce_spec_add_tag(struct LndAnnounceSpecHandle *handle, const char *tag);

/**
 * Clear announce tags.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 */
bool lnd_announce_spec_clear_tags(struct LndAnnounceSpecHandle *handle);

/**
 * Insert one announce metadata key/value pair.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 * string arguments must be valid UTF-8.
 */
bool lnd_announce_spec_insert_metadata(struct LndAnnounceSpecHandle *handle,
                                       const char *key,
                                       const char *value);

/**
 * Clear announce metadata.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 */
bool lnd_announce_spec_clear_metadata(struct LndAnnounceSpecHandle *handle);

/**
 * Set the announce TTL in seconds.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 */
bool lnd_announce_spec_set_ttl_secs(struct LndAnnounceSpecHandle *handle, uint64_t ttl_secs);

/**
 * Resolve the announce addresses from a client config and announce spec and return JSON.
 *
 * # Safety
 * `handle` must be a live client handle.
 * `spec` must be a live announce spec handle.
 * The returned pointer must be released with `lnd_string_free`.
 */
char *lnd_resolve_announce_addrs_json(struct LndClientHandle *handle,
                                      const struct LndAnnounceSpecHandle *spec);

/**
 * Run one announce request from an announce spec handle and return JSON.
 *
 * # Safety
 * `handle` must be a live client handle.
 * `spec` must be a live announce spec handle.
 * The returned pointer must be released with `lnd_string_free`.
 */
char *lnd_announce_once(struct LndClientHandle *handle, const struct LndAnnounceSpecHandle *spec);

/**
 * Start a background announce loop from an announce spec handle.
 *
 * # Safety
 * `handle` must be a live client handle.
 * `spec` must be a live announce spec handle.
 */
struct LndAnnounceHandle *lnd_announce_start_with_spec(struct LndClientHandle *handle,
                                                       const struct LndAnnounceSpecHandle *spec);

/**
 * Start a background announce loop from a JSON spec.
 *
 * # Safety
 * `handle` must be a live pointer returned by `lnd_client_new`.
 * `announce_json` must be a valid, null-terminated UTF-8 string.
 * The returned pointer must be released with `lnd_announce_stop`.
 */
struct LndAnnounceHandle *lnd_announce_start(struct LndClientHandle *handle,
                                             const char *announce_json);

/**
 * Stop and free an announce handle.
 *
 * # Safety
 * `handle` must be null or a pointer returned by this library that has not been stopped yet.
 */
void lnd_announce_stop(struct LndAnnounceHandle *handle);

/**
 * Start a background watch stream from a discovery filter handle.
 *
 * # Safety
 * `handle` must be a live client handle.
 * `filter` must be a live discovery filter handle.
 * `callback` must remain valid until `lnd_watch_stop` is called.
 */
struct LndWatchHandle *lnd_watch_start_with_filter(struct LndClientHandle *handle,
                                                   const struct LndDiscoveryFilterHandle *filter,
                                                   struct Option_LndWatchCallback callback,
                                                   void *user_data);

/**
 * Start a background watch stream and invoke the callback for every JSON event.
 *
 * # Safety
 * `handle` must be a live pointer returned by `lnd_client_new`.
 * `filter_json` must be a valid, null-terminated UTF-8 string.
 * `callback` must remain valid until `lnd_watch_stop` is called.
 * `user_data` is passed through to the callback without validation.
 */
struct LndWatchHandle *lnd_watch_start(struct LndClientHandle *handle,
                                       const char *filter_json,
                                       struct Option_LndWatchCallback callback,
                                       void *user_data);

/**
 * Stop and free a watch handle.
 *
 * # Safety
 * `handle` must be null or a pointer returned by this library that has not been stopped yet.
 */
void lnd_watch_stop(struct LndWatchHandle *handle);

/**
 * Free a C string returned by this library.
 *
 * # Safety
 * `ptr` must be null or a pointer previously returned by this library via `CString::into_raw`.
 */
void lnd_string_free(char *ptr);

const char *lnd_last_error(void);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus
