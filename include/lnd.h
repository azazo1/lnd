#pragma once

/* Generated with cbindgen:0.29.2 */

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * 默认租约 TTL, 单位为秒.
 *
 * v1 中 client 和 server 的默认值都会使用这个常量.
 */
#define DEFAULT_TTL_SECS 30

/**
 * 默认续租间隔, 单位为秒.
 *
 * 该值对应默认 TTL `30s` 的 `ttl / 3`.
 */
#define DEFAULT_RENEW_INTERVAL_SECS 10

/**
 * 默认 SSE keepalive 间隔, 单位为秒.
 */
#define DEFAULT_SSE_KEEPALIVE_SECS 15

/**
 * 默认事件缓冲区容量.
 */
#define DEFAULT_EVENT_BUFFER_CAPACITY 4096

/**
 * Opaque client handle used for discovery, announce and watch operations.
 *
 * Create with `lnd_client_new` or `lnd_client_new_default`, then release with
 * `lnd_client_free`.
 */
typedef struct LndClientHandle {
  uint8_t _private[0];
} LndClientHandle;

/**
 * Opaque filter handle used to build list and watch queries.
 *
 * Create with `lnd_filter_new`, mutate with the setter functions and release
 * with `lnd_filter_free`.
 */
typedef struct LndFilterHandle {
  uint8_t _private[0];
} LndFilterHandle;

/**
 * Opaque announce spec handle used to describe one node registration.
 *
 * Create with `lnd_announce_spec_new`, mutate with the setter functions and
 * release with `lnd_announce_spec_free`.
 */
typedef struct LndAnnounceSpecHandle {
  uint8_t _private[0];
} LndAnnounceSpecHandle;

/**
 * Opaque handle for a background announce loop.
 *
 * Stop and free it with `lnd_announce_stop`.
 */
typedef struct LndAnnounceHandle {
  uint8_t _private[0];
} LndAnnounceHandle;

/**
 * Opaque handle for a background watch loop.
 *
 * Stop and free it with `lnd_watch_stop`.
 */
typedef struct LndWatchHandle {
  uint8_t _private[0];
} LndWatchHandle;

/**
 * Callback invoked by watch streams.
 *
 * `payload` points to a temporary UTF-8 JSON string representing one event
 * envelope. Copy it inside the callback if it must outlive the call.
 *
 * `user_data` is the opaque pointer originally passed to `lnd_watch_start` or
 * `lnd_watch_start_with_filter`.
 */
typedef void (*LndWatchCallback)(const char*, void*);

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * Create a client handle from default config values.
 *
 * The default client uses the library default server URL, timeout, reconnect
 * backoff and automatic address selection policy.
 *
 * Returns a new handle on success, or `NULL` on failure. Inspect
 * `lnd_last_error()` when `NULL` is returned.
 *
 * # Safety
 * The returned handle must be released with `lnd_client_free`.
 */
struct LndClientHandle *lnd_client_new_default(void);

/**
 * Create a client handle for later discovery, announce and watch calls.
 *
 * `server_url` should point at the server root, for example
 * `https://registry.example.com`. `bearer_token` may be empty or null when the
 * server does not require authentication.
 *
 * Returns a new handle on success, or `NULL` on failure. The constructor does
 * not contact the server immediately, so later network failures surface from
 * discover, announce or watch calls.
 *
 * # Safety
 * `server_url` must be a valid, null-terminated UTF-8 string.
 * `bearer_token` may be null, otherwise it must be a valid, null-terminated UTF-8 string.
 */
struct LndClientHandle *lnd_client_new(const char *server_url, const char *bearer_token);

/**
 * Free a client handle.
 *
 * It is safe to pass `NULL`. After this call the handle must not be reused.
 *
 * # Safety
 * `handle` must be null or a pointer returned by this library that has not been freed yet.
 */
void lnd_client_free(struct LndClientHandle *handle);

/**
 * Set the client base URL.
 *
 * Use this to retarget an existing client at another registry without
 * reallocating the higher level wrapper object.
 *
 * Returns `true` on success. On failure returns `false` and stores a message
 * retrievable through `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live client handle.
 * `server_url` must be a valid, null-terminated UTF-8 string.
 */
bool lnd_client_set_server_url(struct LndClientHandle *handle, const char *server_url);

/**
 * Set or clear the client bearer token.
 *
 * Pass `NULL` or an empty string to clear the token. The new token is used by
 * later discovery, announce and watch requests.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live client handle.
 * `bearer_token` may be null, otherwise it must be valid UTF-8.
 */
bool lnd_client_set_bearer_token(struct LndClientHandle *handle, const char *bearer_token);

/**
 * Set the client request timeout in milliseconds.
 *
 * This timeout affects future list, announce and watch setup requests.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live client handle.
 */
bool lnd_client_set_timeout_ms(struct LndClientHandle *handle, uint64_t timeout_ms);

/**
 * Set reconnect backoff bounds in milliseconds.
 *
 * Background announce and watch loops use this range for exponential backoff
 * after transient errors.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
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
 * This changes the client default used by later address resolution operations.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live client handle.
 */
bool lnd_client_set_include_loopback(struct LndClientHandle *handle, bool on);

/**
 * Set whether auto discovered addresses may include IPv6.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live client handle.
 */
bool lnd_client_set_include_ipv6(struct LndClientHandle *handle, bool on);

/**
 * Set whether auto discovered addresses may include private IPv4.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live client handle.
 */
bool lnd_client_set_include_private_ipv4(struct LndClientHandle *handle, bool on);

/**
 * Set whether auto discovered addresses may include link local IPv4.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live client handle.
 */
bool lnd_client_set_include_link_local_ipv4(struct LndClientHandle *handle, bool on);

/**
 * Allow only a named interface for auto discovered addresses.
 *
 * Once at least one interface is allowed, only allowlisted interfaces are
 * considered. Deny rules still take precedence.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live client handle.
 * `interface_name` must be valid UTF-8.
 */
bool lnd_client_enable_interface(struct LndClientHandle *handle, const char *interface_name);

/**
 * Deny a named interface for auto discovered addresses.
 *
 * Denied interfaces are ignored even when they are also present in the
 * allowlist.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live client handle.
 * `interface_name` must be valid UTF-8.
 */
bool lnd_client_disable_interface(struct LndClientHandle *handle, const char *interface_name);

/**
 * Clear client interface allow and deny filters.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live client handle.
 */
bool lnd_client_clear_interface_filters(struct LndClientHandle *handle);

/**
 * Create a discovery filter handle.
 *
 * `network_id` may be null or empty. Service, tag and scope constraints can be
 * added later.
 *
 * Returns a new handle on success, or `NULL` on failure. Inspect
 * `lnd_last_error()` when `NULL` is returned.
 *
 * # Safety
 * `network_id` may be null, otherwise it must be valid UTF-8.
 */
struct LndFilterHandle *lnd_filter_new(const char *network_id);

/**
 * Free a discovery filter handle.
 *
 * It is safe to pass `NULL`.
 *
 * # Safety
 * `handle` must be null or a live discovery filter handle.
 */
void lnd_filter_free(struct LndFilterHandle *handle);

/**
 * Set the discovery service filter.
 *
 * The resulting filter matches only nodes that advertise this service.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live discovery filter handle.
 * `service` must be valid UTF-8.
 */
bool lnd_filter_set_service(struct LndFilterHandle *handle, const char *service);

/**
 * Set the logical network_id filter.
 *
 * Pass null or an empty string to clear the network_id constraint.
 *
 * # Safety
 * `handle` must be a live discovery filter handle.
 * `network_id` may be null, otherwise it must be valid UTF-8.
 */
bool lnd_filter_set_network_id(struct LndFilterHandle *handle, const char *network_id);

/**
 * Clear the discovery service filter.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live discovery filter handle.
 */
bool lnd_filter_clear_service(struct LndFilterHandle *handle);

/**
 * Add one discovery tag filter.
 *
 * A node must contain every tag added to the filter to match.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live discovery filter handle.
 * `tag` must be valid UTF-8.
 */
bool lnd_filter_add_tag(struct LndFilterHandle *handle, const char *tag);

/**
 * Clear discovery tag filters.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live discovery filter handle.
 */
bool lnd_filter_clear_tags(struct LndFilterHandle *handle);

/**
 * Add one reachability scope overlap filter.
 *
 * # Safety
 * `handle` must be a live discovery filter handle.
 * `scope` must be valid UTF-8.
 */
bool lnd_filter_add_scope(struct LndFilterHandle *handle, const char *scope);

/**
 * Clear discovery reachability scope filters.
 *
 * # Safety
 * `handle` must be a live discovery filter handle.
 */
bool lnd_filter_clear_scopes(struct LndFilterHandle *handle);

/**
 * Run a one-shot discovery request from a filter handle and return JSON.
 *
 * The returned JSON has the same shape as the Rust and higher level bindings,
 * typically `{ "nodes": [...], "cursor": 123 }` internally before wrappers
 * flatten the result. The exact schema should be treated as the public API of
 * the server, not as a C struct layout.
 *
 * Returns a newly allocated UTF-8 string on success. On failure returns `NULL`
 * and stores a message in `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live client handle.
 * `filter` must be a live discovery filter handle.
 * The returned pointer must be released with `lnd_string_free`.
 */
char *lnd_discover(struct LndClientHandle *handle, const struct LndFilterHandle *filter);

/**
 * Run a one-shot discovery request and return a newly allocated JSON string.
 *
 * This variant accepts the filter as a JSON document instead of an opaque
 * handle. It is convenient for FFI users that already model requests in JSON.
 *
 * Returns a newly allocated UTF-8 string on success. On failure returns `NULL`
 * and stores a message in `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live pointer returned by `lnd_client_new`.
 * `filter_json` must be a valid, null-terminated UTF-8 string.
 * The returned pointer must be released with `lnd_string_free`.
 */
char *lnd_discover_json(struct LndClientHandle *handle, const char *filter_json);

/**
 * Derive one local network_id from the client's default address selection.
 *
 * This uses the same automatic selection policy as Rust and other SDKs. When
 * multiple equally valid subnets are visible, the function returns `NULL` and
 * sets `lnd_last_error()`.
 *
 * Returns a newly allocated UTF-8 string on success. On failure returns `NULL`
 * and stores a message in `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live client handle.
 * The returned pointer must be released with `lnd_string_free`.
 */
char *lnd_resolve_network_id(struct LndClientHandle *handle);

/**
 * List all locally derived network_id candidates as a JSON array.
 *
 * Each JSON item contains `network_id` and `scope`. This is useful when a
 * higher level binding wants to show candidate subnets to the caller before
 * picking one explicitly.
 *
 * Returns a newly allocated UTF-8 string on success. On failure returns `NULL`
 * and stores a message in `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live client handle.
 * The returned pointer must be released with `lnd_string_free`.
 */
char *lnd_list_network_id_candidates_json(struct LndClientHandle *handle);

/**
 * Create an announce spec handle.
 *
 * The returned spec starts with automatic LAN address discovery enabled and
 * `DEFAULT_TTL_SECS` as its lease duration.
 *
 * Returns a new handle on success, or `NULL` on failure. Inspect
 * `lnd_last_error()` when `NULL` is returned.
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
 * It is safe to pass `NULL`.
 *
 * # Safety
 * `handle` must be null or a live announce spec handle.
 */
void lnd_announce_spec_free(struct LndAnnounceSpecHandle *handle);

/**
 * Set the announce network_id.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 * `network_id` must be valid UTF-8.
 */
bool lnd_announce_spec_set_network_id(struct LndAnnounceSpecHandle *handle, const char *network_id);

/**
 * Set the announce node_id.
 *
 * The node id should remain stable across restarts so other peers can treat
 * the node as the same logical instance.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 * `node_id` must be valid UTF-8.
 */
bool lnd_announce_spec_set_node_id(struct LndAnnounceSpecHandle *handle, const char *node_id);

/**
 * Set the announce service name.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 * `service` must be valid UTF-8.
 */
bool lnd_announce_spec_set_service(struct LndAnnounceSpecHandle *handle, const char *service);

/**
 * Set the announce display name.
 *
 * The display name is intended for humans and does not need to be globally
 * unique.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
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
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 */
bool lnd_announce_spec_set_port(struct LndAnnounceSpecHandle *handle, uint16_t port);

/**
 * Set whether auto address discovery is enabled.
 *
 * When enabled, the client combines eligible local interface addresses with
 * any explicit LAN addresses attached to the spec.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 */
bool lnd_announce_spec_set_auto_lan_addrs(struct LndAnnounceSpecHandle *handle, bool on);

/**
 * Set whether automatic reachability scope collection is enabled.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 */
bool lnd_announce_spec_set_auto_reachability_scopes(struct LndAnnounceSpecHandle *handle, bool on);

/**
 * Add one explicit LAN address.
 *
 * The address should be passed in `host:port` form.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 * `addr` must be valid UTF-8.
 */
bool lnd_announce_spec_add_lan_addr(struct LndAnnounceSpecHandle *handle, const char *addr);

/**
 * Clear explicit LAN addresses.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 */
bool lnd_announce_spec_clear_lan_addrs(struct LndAnnounceSpecHandle *handle);

/**
 * Add one explicit reachability scope.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 * `scope` must be valid UTF-8.
 */
bool lnd_announce_spec_add_scope(struct LndAnnounceSpecHandle *handle, const char *scope);

/**
 * Clear explicit reachability scopes.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 */
bool lnd_announce_spec_clear_scopes(struct LndAnnounceSpecHandle *handle);

/**
 * Set whether auto discovered addresses may include loopback.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 */
bool lnd_announce_spec_set_include_loopback(struct LndAnnounceSpecHandle *handle, bool on);

/**
 * Set whether auto discovered addresses may include IPv6.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 */
bool lnd_announce_spec_set_include_ipv6(struct LndAnnounceSpecHandle *handle, bool on);

/**
 * Set whether auto discovered addresses may include private IPv4.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 */
bool lnd_announce_spec_set_include_private_ipv4(struct LndAnnounceSpecHandle *handle, bool on);

/**
 * Set whether auto discovered addresses may include link local IPv4.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 */
bool lnd_announce_spec_set_include_link_local_ipv4(struct LndAnnounceSpecHandle *handle, bool on);

/**
 * Allow only a named interface for auto discovered announce addresses.
 *
 * Once at least one interface is allowed, only allowlisted interfaces are
 * considered. Deny rules still take precedence.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
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
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
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
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 */
bool lnd_announce_spec_clear_interface_filters(struct LndAnnounceSpecHandle *handle);

/**
 * Add one announce tag.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 * `tag` must be valid UTF-8.
 */
bool lnd_announce_spec_add_tag(struct LndAnnounceSpecHandle *handle, const char *tag);

/**
 * Clear announce tags.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 */
bool lnd_announce_spec_clear_tags(struct LndAnnounceSpecHandle *handle);

/**
 * Insert one announce metadata key/value pair.
 *
 * Later calls with the same key replace the previous value.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
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
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 */
bool lnd_announce_spec_clear_metadata(struct LndAnnounceSpecHandle *handle);

/**
 * Set the announce TTL in seconds.
 *
 * The background announce loop renews around every third of this value.
 *
 * Returns `true` on success. On failure returns `false` and sets
 * `lnd_last_error()`.
 *
 * # Safety
 * `handle` must be a live announce spec handle.
 */
bool lnd_announce_spec_set_ttl_secs(struct LndAnnounceSpecHandle *handle, uint64_t ttl_secs);

/**
 * Resolve the announce addresses from a client config and announce spec and return JSON.
 *
 * The result is a JSON array of deduplicated `host:port` strings. It is useful
 * when higher level code wants to inspect or override the final LAN addresses
 * before registration.
 *
 * Returns a newly allocated UTF-8 string on success. On failure returns `NULL`
 * and stores a message in `lnd_last_error()`.
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
 * The returned JSON is the normalized node record produced by the server after
 * lease metadata is attached.
 *
 * Returns a newly allocated UTF-8 string on success. On failure returns `NULL`
 * and stores a message in `lnd_last_error()`.
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
 * The loop renews the server lease until `lnd_announce_stop` is called.
 *
 * Returns a handle on success, or `NULL` on failure. Inspect `lnd_last_error()`
 * when `NULL` is returned.
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
 * This variant accepts the announce spec as a UTF-8 JSON document instead of
 * an opaque handle.
 *
 * Returns a handle on success, or `NULL` on failure. Inspect `lnd_last_error()`
 * when `NULL` is returned.
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
 * It is safe to pass `NULL`. After this call the handle must not be reused.
 *
 * # Safety
 * `handle` must be null or a pointer returned by this library that has not been stopped yet.
 */
void lnd_announce_stop(struct LndAnnounceHandle *handle);

/**
 * Start a background watch stream from a filter handle.
 *
 * Each callback receives one UTF-8 JSON event envelope. Callers should copy the
 * payload inside the callback if it must be retained.
 *
 * Returns a handle on success, or `NULL` on failure. Inspect `lnd_last_error()`
 * when `NULL` is returned.
 *
 * # Safety
 * `handle` must be a live client handle.
 * `filter` must be a live filter handle.
 * `callback` must remain valid until `lnd_watch_stop` is called.
 */
struct LndWatchHandle *lnd_watch_start_with_filter(struct LndClientHandle *handle,
                                                   const struct LndFilterHandle *filter,
                                                   LndWatchCallback callback,
                                                   void *user_data);

/**
 * Start a background watch stream and invoke the callback for every JSON event.
 *
 * This variant accepts the filter as a UTF-8 JSON document instead of an
 * opaque handle.
 *
 * Returns a handle on success, or `NULL` on failure. Inspect `lnd_last_error()`
 * when `NULL` is returned.
 *
 * # Safety
 * `handle` must be a live pointer returned by `lnd_client_new`.
 * `filter_json` must be a valid, null-terminated UTF-8 string.
 * `callback` must remain valid until `lnd_watch_stop` is called.
 * `user_data` is passed through to the callback without validation.
 */
struct LndWatchHandle *lnd_watch_start(struct LndClientHandle *handle,
                                       const char *filter_json,
                                       LndWatchCallback callback,
                                       void *user_data);

/**
 * Stop and free a watch handle.
 *
 * It is safe to pass `NULL`. After this call the handle must not be reused.
 *
 * # Safety
 * `handle` must be null or a pointer returned by this library that has not been stopped yet.
 */
void lnd_watch_stop(struct LndWatchHandle *handle);

/**
 * Free a C string returned by this library.
 *
 * This must be called for every non null string returned by functions such as
 * `lnd_discover`, `lnd_announce_once` and `lnd_resolve_announce_addrs_json`.
 *
 * # Safety
 * `ptr` must be null or a pointer previously returned by this library via `CString::into_raw`.
 */
void lnd_string_free(char *ptr);

/**
 * Return the last thread local error message produced by this library.
 *
 * The pointer is borrowed and must not be freed by the caller. It may be
 * overwritten by later calls from the same thread.
 */
const char *lnd_last_error(void);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus
