#include <iostream>

#include "lnd.hpp"

int main() {
  lnd::Client client("http://127.0.0.1:8765", "dev-token");
  auto nodes = client.discover_json(
      lnd::DiscoveryFilter("office-a").with_service("_demo._tcp").add_tag("stable"));
  std::cout << nodes << std::endl;
  return 0;
}
