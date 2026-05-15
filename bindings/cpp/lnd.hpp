#pragma once

#include <functional>
#include <map>
#include <memory>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

extern "C" {
#include "lnd.h"
}

namespace lnd {

class Error : public std::runtime_error {
public:
  explicit Error(const std::string& message) : std::runtime_error(message) {}
};

inline std::string last_error() {
  const char* value = lnd_last_error();
  if (value == nullptr) {
    return "unknown lnd error";
  }
  return value;
}

inline void check_bool(bool ok) {
  if (!ok) {
    throw Error(last_error());
  }
}

template <typename Handle>
inline Handle* check_ptr(Handle* ptr) {
  if (ptr == nullptr) {
    throw Error(last_error());
  }
  return ptr;
}

struct DiscoveryFilter {
  std::string network_id;
  std::string service;
  std::vector<std::string> tags;

  explicit DiscoveryFilter(std::string network_id_in) : network_id(std::move(network_id_in)) {}

  DiscoveryFilter& with_service(std::string value) {
    service = std::move(value);
    return *this;
  }

  DiscoveryFilter& add_tag(std::string value) {
    tags.push_back(std::move(value));
    return *this;
  }
};

struct AnnounceSpec {
  std::string network_id;
  std::string node_id;
  std::string service;
  std::string display_name;
  uint16_t port;
  bool auto_lan_addrs = true;
  uint64_t ttl_secs = DEFAULT_TTL_SECS;
  std::vector<std::string> lan_addrs;
  std::vector<std::string> tags;
  std::map<std::string, std::string> metadata;
  bool include_loopback = false;
  bool include_ipv6 = false;
  bool include_private_ipv4 = true;
  bool include_link_local_ipv4 = false;
  std::vector<std::string> interface_allowlist;
  std::vector<std::string> interface_denylist;

  AnnounceSpec(
      std::string network_id_in,
      std::string node_id_in,
      std::string service_in,
      std::string display_name_in,
      uint16_t port_in)
      : network_id(std::move(network_id_in)),
        node_id(std::move(node_id_in)),
        service(std::move(service_in)),
        display_name(std::move(display_name_in)),
        port(port_in) {}

  AnnounceSpec& add_lan_addr(std::string addr) {
    lan_addrs.push_back(std::move(addr));
    return *this;
  }

  AnnounceSpec& add_tag(std::string tag) {
    tags.push_back(std::move(tag));
    return *this;
  }

  AnnounceSpec& insert_metadata(std::string key, std::string value) {
    metadata.insert_or_assign(std::move(key), std::move(value));
    return *this;
  }

  AnnounceSpec& enable_interface(std::string name) {
    interface_allowlist.push_back(std::move(name));
    return *this;
  }

  AnnounceSpec& disable_interface(std::string name) {
    interface_denylist.push_back(std::move(name));
    return *this;
  }
};

class AnnounceHandle {
public:
  explicit AnnounceHandle(LndAnnounceHandle* handle = nullptr) : handle_(handle) {}
  AnnounceHandle(const AnnounceHandle&) = delete;
  AnnounceHandle& operator=(const AnnounceHandle&) = delete;

  AnnounceHandle(AnnounceHandle&& other) noexcept : handle_(other.handle_) {
    other.handle_ = nullptr;
  }

  AnnounceHandle& operator=(AnnounceHandle&& other) noexcept {
    if (this != &other) {
      close();
      handle_ = other.handle_;
      other.handle_ = nullptr;
    }
    return *this;
  }

  ~AnnounceHandle() {
    close();
  }

  void close() {
    if (handle_ != nullptr) {
      lnd_announce_stop(handle_);
      handle_ = nullptr;
    }
  }

private:
  LndAnnounceHandle* handle_;
};

class WatchHandle {
public:
  explicit WatchHandle(
      LndWatchHandle* handle = nullptr,
      std::shared_ptr<std::function<void(const std::string&)>> callback = nullptr)
      : handle_(handle), callback_(std::move(callback)) {}
  WatchHandle(const WatchHandle&) = delete;
  WatchHandle& operator=(const WatchHandle&) = delete;

  WatchHandle(WatchHandle&& other) noexcept
      : handle_(other.handle_), callback_(std::move(other.callback_)) {
    other.handle_ = nullptr;
  }

  WatchHandle& operator=(WatchHandle&& other) noexcept {
    if (this != &other) {
      close();
      handle_ = other.handle_;
      callback_ = std::move(other.callback_);
      other.handle_ = nullptr;
    }
    return *this;
  }

  ~WatchHandle() {
    close();
  }

  void close() {
    if (handle_ != nullptr) {
      lnd_watch_stop(handle_);
      handle_ = nullptr;
    }
    callback_.reset();
  }

private:
  static void callback_bridge(const char* payload, void* user_data) {
    if (payload == nullptr || user_data == nullptr) {
      return;
    }
    auto* callback = static_cast<std::function<void(const std::string&)>*>(user_data);
    (*callback)(std::string(payload));
  }

  friend class Client;

  LndWatchHandle* handle_;
  std::shared_ptr<std::function<void(const std::string&)>> callback_;
};

class Client {
public:
  explicit Client(const std::string& server_url, const std::string& bearer_token = "")
      : handle_(check_ptr(lnd_client_new(server_url.c_str(), bearer_token.c_str()))) {}

  Client(const Client&) = delete;
  Client& operator=(const Client&) = delete;

  Client(Client&& other) noexcept : handle_(other.handle_) {
    other.handle_ = nullptr;
  }

  Client& operator=(Client&& other) noexcept {
    if (this != &other) {
      close();
      handle_ = other.handle_;
      other.handle_ = nullptr;
    }
    return *this;
  }

  ~Client() {
    close();
  }

  void close() {
    if (handle_ != nullptr) {
      lnd_client_free(handle_);
      handle_ = nullptr;
    }
  }

  Client& set_server_url(const std::string& value) {
    check_bool(lnd_client_set_server_url(handle_, value.c_str()));
    return *this;
  }

  Client& set_bearer_token(const std::string& value) {
    check_bool(lnd_client_set_bearer_token(handle_, value.c_str()));
    return *this;
  }

  Client& set_timeout_ms(uint64_t value) {
    check_bool(lnd_client_set_timeout_ms(handle_, value));
    return *this;
  }

  Client& set_reconnect_backoff_ms(uint64_t min_ms, uint64_t max_ms) {
    check_bool(lnd_client_set_reconnect_backoff_ms(handle_, min_ms, max_ms));
    return *this;
  }

  Client& set_include_loopback(bool on) {
    check_bool(lnd_client_set_include_loopback(handle_, on));
    return *this;
  }

  Client& set_include_ipv6(bool on) {
    check_bool(lnd_client_set_include_ipv6(handle_, on));
    return *this;
  }

  Client& set_include_private_ipv4(bool on) {
    check_bool(lnd_client_set_include_private_ipv4(handle_, on));
    return *this;
  }

  Client& set_include_link_local_ipv4(bool on) {
    check_bool(lnd_client_set_include_link_local_ipv4(handle_, on));
    return *this;
  }

  Client& enable_interface(const std::string& value) {
    check_bool(lnd_client_enable_interface(handle_, value.c_str()));
    return *this;
  }

  Client& disable_interface(const std::string& value) {
    check_bool(lnd_client_disable_interface(handle_, value.c_str()));
    return *this;
  }

  Client& clear_interface_filters() {
    check_bool(lnd_client_clear_interface_filters(handle_));
    return *this;
  }

  std::string discover_json(const DiscoveryFilter& filter) const {
    LndDiscoveryFilterHandle* filter_handle = build_filter(filter);
    try {
      char* payload = check_ptr(lnd_discover(handle_, filter_handle));
      std::string value = take_string(payload);
      lnd_discovery_filter_free(filter_handle);
      return value;
    } catch (...) {
      lnd_discovery_filter_free(filter_handle);
      throw;
    }
  }

  std::string resolve_announce_addrs_json(const AnnounceSpec& spec) const {
    LndAnnounceSpecHandle* spec_handle = build_spec(spec);
    try {
      char* payload = check_ptr(lnd_resolve_announce_addrs_json(handle_, spec_handle));
      std::string value = take_string(payload);
      lnd_announce_spec_free(spec_handle);
      return value;
    } catch (...) {
      lnd_announce_spec_free(spec_handle);
      throw;
    }
  }

  std::string announce_once_json(const AnnounceSpec& spec) const {
    LndAnnounceSpecHandle* spec_handle = build_spec(spec);
    try {
      char* payload = check_ptr(lnd_announce_once(handle_, spec_handle));
      std::string value = take_string(payload);
      lnd_announce_spec_free(spec_handle);
      return value;
    } catch (...) {
      lnd_announce_spec_free(spec_handle);
      throw;
    }
  }

  AnnounceHandle announce(const AnnounceSpec& spec) const {
    LndAnnounceSpecHandle* spec_handle = build_spec(spec);
    try {
      AnnounceHandle value(check_ptr(lnd_announce_start_with_spec(handle_, spec_handle)));
      lnd_announce_spec_free(spec_handle);
      return value;
    } catch (...) {
      lnd_announce_spec_free(spec_handle);
      throw;
    }
  }

  WatchHandle watch(
      const DiscoveryFilter& filter,
      std::function<void(const std::string&)> callback) const {
    LndDiscoveryFilterHandle* filter_handle = build_filter(filter);
    auto callback_ref =
        std::make_shared<std::function<void(const std::string&)>>(std::move(callback));
    try {
      WatchHandle value(
          check_ptr(lnd_watch_start_with_filter(
              handle_,
              filter_handle,
              WatchHandle::callback_bridge,
              callback_ref.get())),
          callback_ref);
      lnd_discovery_filter_free(filter_handle);
      return value;
    } catch (...) {
      lnd_discovery_filter_free(filter_handle);
      throw;
    }
  }

private:
  static std::string take_string(char* payload) {
    std::string value(payload);
    lnd_string_free(payload);
    return value;
  }

  static LndDiscoveryFilterHandle* build_filter(const DiscoveryFilter& filter) {
    LndDiscoveryFilterHandle* handle = check_ptr(lnd_discovery_filter_new(filter.network_id.c_str()));
    try {
      if (!filter.service.empty()) {
        check_bool(lnd_discovery_filter_set_service(handle, filter.service.c_str()));
      }
      for (const auto& tag : filter.tags) {
        check_bool(lnd_discovery_filter_add_tag(handle, tag.c_str()));
      }
      return handle;
    } catch (...) {
      lnd_discovery_filter_free(handle);
      throw;
    }
  }

  static LndAnnounceSpecHandle* build_spec(const AnnounceSpec& spec) {
    LndAnnounceSpecHandle* handle = check_ptr(lnd_announce_spec_new(
        spec.network_id.c_str(),
        spec.node_id.c_str(),
        spec.service.c_str(),
        spec.display_name.c_str(),
        spec.port));
    try {
      check_bool(lnd_announce_spec_set_auto_lan_addrs(handle, spec.auto_lan_addrs));
      check_bool(lnd_announce_spec_set_ttl_secs(handle, spec.ttl_secs));
      check_bool(lnd_announce_spec_set_include_loopback(handle, spec.include_loopback));
      check_bool(lnd_announce_spec_set_include_ipv6(handle, spec.include_ipv6));
      check_bool(lnd_announce_spec_set_include_private_ipv4(handle, spec.include_private_ipv4));
      check_bool(
          lnd_announce_spec_set_include_link_local_ipv4(handle, spec.include_link_local_ipv4));
      for (const auto& addr : spec.lan_addrs) {
        check_bool(lnd_announce_spec_add_lan_addr(handle, addr.c_str()));
      }
      for (const auto& tag : spec.tags) {
        check_bool(lnd_announce_spec_add_tag(handle, tag.c_str()));
      }
      for (const auto& entry : spec.metadata) {
        check_bool(lnd_announce_spec_insert_metadata(
            handle,
            entry.first.c_str(),
            entry.second.c_str()));
      }
      for (const auto& interface_name : spec.interface_allowlist) {
        check_bool(lnd_announce_spec_enable_interface(handle, interface_name.c_str()));
      }
      for (const auto& interface_name : spec.interface_denylist) {
        check_bool(lnd_announce_spec_disable_interface(handle, interface_name.c_str()));
      }
      return handle;
    } catch (...) {
      lnd_announce_spec_free(handle);
      throw;
    }
  }

  LndClientHandle* handle_ = nullptr;
};

}  // namespace lnd
