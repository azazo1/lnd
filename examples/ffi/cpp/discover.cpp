#include <cstdlib>
#include <iostream>
#include <memory>

extern "C" {
#include "lnd.h"
}

struct ClientDeleter {
  void operator()(LndClientHandle *ptr) const {
    lnd_client_free(ptr);
  }
};

struct FilterDeleter {
  void operator()(LndDiscoveryFilterHandle *ptr) const {
    lnd_discovery_filter_free(ptr);
  }
};

int main() {
  std::unique_ptr<LndClientHandle, ClientDeleter> client(
      lnd_client_new("http://127.0.0.1:8765", "dev-token"));
  if (!client) {
    std::cerr << "client init failed: " << lnd_last_error() << std::endl;
    return EXIT_FAILURE;
  }

  std::unique_ptr<LndDiscoveryFilterHandle, FilterDeleter> filter(
      lnd_discovery_filter_new("office-a"));
  if (!filter) {
    std::cerr << "filter init failed: " << lnd_last_error() << std::endl;
    return EXIT_FAILURE;
  }

  if (!lnd_discovery_filter_set_service(filter.get(), "_demo._tcp")) {
    std::cerr << "set service failed: " << lnd_last_error() << std::endl;
    return EXIT_FAILURE;
  }

  char *json = lnd_discover(client.get(), filter.get());
  if (json == nullptr) {
    std::cerr << "discover failed: " << lnd_last_error() << std::endl;
    return EXIT_FAILURE;
  }

  std::cout << json << std::endl;
  lnd_string_free(json);
  return EXIT_SUCCESS;
}
