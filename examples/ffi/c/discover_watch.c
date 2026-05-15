#include <stdio.h>

#include "lnd.h"

static void on_event(const char *json, void *user_data) {
  (void) user_data;
  printf("event: %s\n", json);
}

int main(void) {
  struct LndClientHandle *client = lnd_client_new("http://127.0.0.1:8765", "dev-token");
  if (client == NULL) {
    fprintf(stderr, "client init failed: %s\n", lnd_last_error());
    return 1;
  }

  struct LndDiscoveryFilterHandle *filter = lnd_discovery_filter_new("office-a");
  if (filter == NULL) {
    fprintf(stderr, "filter init failed: %s\n", lnd_last_error());
    lnd_client_free(client);
    return 1;
  }

  if (!lnd_discovery_filter_set_service(filter, "_demo._tcp")) {
    fprintf(stderr, "set service failed: %s\n", lnd_last_error());
    lnd_discovery_filter_free(filter);
    lnd_client_free(client);
    return 1;
  }

  char *nodes = lnd_discover(client, filter);
  if (nodes == NULL) {
    fprintf(stderr, "discover failed: %s\n", lnd_last_error());
    lnd_discovery_filter_free(filter);
    lnd_client_free(client);
    return 1;
  }
  printf("nodes: %s\n", nodes);
  lnd_string_free(nodes);

  struct LndWatchHandle *watch = lnd_watch_start_with_filter(client, filter, on_event, NULL);
  if (watch == NULL) {
    fprintf(stderr, "watch failed: %s\n", lnd_last_error());
    lnd_discovery_filter_free(filter);
    lnd_client_free(client);
    return 1;
  }

  puts("watching... press Enter to stop");
  getchar();

  lnd_watch_stop(watch);
  lnd_discovery_filter_free(filter);
  lnd_client_free(client);
  return 0;
}
