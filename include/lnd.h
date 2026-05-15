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

typedef struct LndClient LndClient;

typedef struct Option_AnnounceHandle Option_AnnounceHandle;

typedef struct Option_JoinHandle Option_JoinHandle;

typedef struct Option_LndWatchCallback Option_LndWatchCallback;

typedef struct Option_Sender Option_Sender;

typedef struct LndClientHandle {
  struct LndClient client;
} LndClientHandle;

typedef struct LndAnnounceHandle {
  struct Option_AnnounceHandle handle;
} LndAnnounceHandle;

typedef struct LndWatchHandle {
  struct Option_Sender stop_tx;
  struct Option_JoinHandle join_handle;
} LndWatchHandle;

typedef void (*LndWatchCallback)(const char*, void*);

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * Create a client handle for later discovery, announce and watch calls.
 *
 * # Safety
 * `server_url` must be a valid, null-terminated UTF-8 string.
 * `bearer_token` may be null, otherwise it must be a valid, null-terminated UTF-8 string.
 */
struct LndClientHandle *lnd_client_new(const char *server_url, const char *bearer_token);

/**
 * Free a client handle previously returned by `lnd_client_new`.
 *
 * # Safety
 * `handle` must be null or a pointer returned by `lnd_client_new` that has not been freed yet.
 */
void lnd_client_free(struct LndClientHandle *handle);

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
 * `handle` must be null or a pointer returned by `lnd_announce_start` that has not been stopped yet.
 */
void lnd_announce_stop(struct LndAnnounceHandle *handle);

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
 * `handle` must be null or a pointer returned by `lnd_watch_start` that has not been stopped yet.
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
