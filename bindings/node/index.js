import path from "node:path";
import { fileURLToPath } from "node:url";
import koffi from "koffi";

function defaultRoot() {
  return path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
}

function defaultLibraryPath(root = defaultRoot()) {
  if (process.env.LND_LIBRARY_PATH) {
    return process.env.LND_LIBRARY_PATH;
  }
  const releaseDir = path.join(root, "target", "release");
  switch (process.platform) {
    case "darwin":
      return path.join(releaseDir, "liblnd.dylib");
    case "win32":
      return path.join(releaseDir, "lnd.dll");
    default:
      return path.join(releaseDir, "liblnd.so");
  }
}

function createBindings(libraryPath = defaultLibraryPath()) {
  const lib = koffi.load(libraryPath);
  const watchCallback = koffi.proto("LndWatchCallback", "void", ["str", "void *"]);
  return {
    lib,
    watchCallback,
    clientNew: lib.func("lnd_client_new", "void *", ["str", "str"]),
    clientFree: lib.func("lnd_client_free", "void", ["void *"]),
    clientSetServerUrl: lib.func("lnd_client_set_server_url", "bool", ["void *", "str"]),
    clientSetBearerToken: lib.func("lnd_client_set_bearer_token", "bool", ["void *", "str"]),
    clientSetTimeoutMs: lib.func("lnd_client_set_timeout_ms", "bool", ["void *", "uint64"]),
    clientSetReconnectBackoffMs: lib.func(
      "lnd_client_set_reconnect_backoff_ms",
      "bool",
      ["void *", "uint64", "uint64"],
    ),
    clientSetIncludeLoopback: lib.func(
      "lnd_client_set_include_loopback",
      "bool",
      ["void *", "bool"],
    ),
    clientSetIncludeIpv6: lib.func("lnd_client_set_include_ipv6", "bool", ["void *", "bool"]),
    clientSetIncludePrivateIpv4: lib.func(
      "lnd_client_set_include_private_ipv4",
      "bool",
      ["void *", "bool"],
    ),
    clientSetIncludeLinkLocalIpv4: lib.func(
      "lnd_client_set_include_link_local_ipv4",
      "bool",
      ["void *", "bool"],
    ),
    clientEnableInterface: lib.func("lnd_client_enable_interface", "bool", ["void *", "str"]),
    clientDisableInterface: lib.func("lnd_client_disable_interface", "bool", ["void *", "str"]),
    clientClearInterfaceFilters: lib.func(
      "lnd_client_clear_interface_filters",
      "bool",
      ["void *"],
    ),
    filterNew: lib.func("lnd_discovery_filter_new", "void *", ["str"]),
    filterFree: lib.func("lnd_discovery_filter_free", "void", ["void *"]),
    filterSetService: lib.func("lnd_discovery_filter_set_service", "bool", ["void *", "str"]),
    filterClearService: lib.func("lnd_discovery_filter_clear_service", "bool", ["void *"]),
    filterAddTag: lib.func("lnd_discovery_filter_add_tag", "bool", ["void *", "str"]),
    filterClearTags: lib.func("lnd_discovery_filter_clear_tags", "bool", ["void *"]),
    discover: lib.func("lnd_discover", "void *", ["void *", "void *"]),
    announceSpecNew: lib.func(
      "lnd_announce_spec_new",
      "void *",
      ["str", "str", "str", "str", "uint16"],
    ),
    announceSpecFree: lib.func("lnd_announce_spec_free", "void", ["void *"]),
    announceSpecSetAutoLanAddrs: lib.func(
      "lnd_announce_spec_set_auto_lan_addrs",
      "bool",
      ["void *", "bool"],
    ),
    announceSpecAddLanAddr: lib.func("lnd_announce_spec_add_lan_addr", "bool", ["void *", "str"]),
    announceSpecSetIncludeLoopback: lib.func(
      "lnd_announce_spec_set_include_loopback",
      "bool",
      ["void *", "bool"],
    ),
    announceSpecSetIncludeIpv6: lib.func(
      "lnd_announce_spec_set_include_ipv6",
      "bool",
      ["void *", "bool"],
    ),
    announceSpecSetIncludePrivateIpv4: lib.func(
      "lnd_announce_spec_set_include_private_ipv4",
      "bool",
      ["void *", "bool"],
    ),
    announceSpecSetIncludeLinkLocalIpv4: lib.func(
      "lnd_announce_spec_set_include_link_local_ipv4",
      "bool",
      ["void *", "bool"],
    ),
    announceSpecEnableInterface: lib.func(
      "lnd_announce_spec_enable_interface",
      "bool",
      ["void *", "str"],
    ),
    announceSpecDisableInterface: lib.func(
      "lnd_announce_spec_disable_interface",
      "bool",
      ["void *", "str"],
    ),
    announceSpecAddTag: lib.func("lnd_announce_spec_add_tag", "bool", ["void *", "str"]),
    announceSpecInsertMetadata: lib.func(
      "lnd_announce_spec_insert_metadata",
      "bool",
      ["void *", "str", "str"],
    ),
    announceSpecSetTtlSecs: lib.func("lnd_announce_spec_set_ttl_secs", "bool", ["void *", "uint64"]),
    resolveAnnounceAddrsJson: lib.func(
      "lnd_resolve_announce_addrs_json",
      "void *",
      ["void *", "void *"],
    ),
    announceOnce: lib.func("lnd_announce_once", "void *", ["void *", "void *"]),
    announceStartWithSpec: lib.func(
      "lnd_announce_start_with_spec",
      "void *",
      ["void *", "void *"],
    ),
    announceStop: lib.func("lnd_announce_stop", "void", ["void *"]),
    watchStartWithFilter: lib.func(
      "lnd_watch_start_with_filter",
      "void *",
      ["void *", "void *", watchCallback, "void *"],
    ),
    watchStop: lib.func("lnd_watch_stop", "void", ["void *"]),
    stringFree: lib.func("lnd_string_free", "void", ["void *"]),
    lastError: lib.func("lnd_last_error", "str", []),
  };
}

class LndError extends Error {}

function checkPtr(bindings, ptr) {
  if (!ptr) {
    throw new LndError(bindings.lastError() || "unknown lnd error");
  }
  return ptr;
}

function checkBool(bindings, ok) {
  if (!ok) {
    throw new LndError(bindings.lastError() || "unknown lnd error");
  }
}

function readJsonString(bindings, ptr) {
  try {
    return JSON.parse(koffi.decode(ptr, "char", -1));
  } finally {
    bindings.stringFree(ptr);
  }
}

export class DiscoveryFilter {
  constructor(networkId) {
    this.networkId = networkId;
    this.service = null;
    this.tags = [];
  }

  withService(service) {
    this.service = service;
    return this;
  }

  addTag(tag) {
    this.tags.push(tag);
    return this;
  }

  _intoHandle(bindings) {
    const handle = checkPtr(bindings, bindings.filterNew(this.networkId));
    try {
      if (this.service !== null) {
        checkBool(bindings, bindings.filterSetService(handle, this.service));
      }
      for (const tag of this.tags) {
        checkBool(bindings, bindings.filterAddTag(handle, tag));
      }
      return handle;
    } catch (error) {
      bindings.filterFree(handle);
      throw error;
    }
  }
}

export class AnnounceSpec {
  constructor(networkId, nodeId, service, displayName, port) {
    this.networkId = networkId;
    this.nodeId = nodeId;
    this.service = service;
    this.displayName = displayName;
    this.port = port;
    this.autoLanAddrs = true;
    this.ttlSecs = 30;
    this.lanAddrs = [];
    this.tags = [];
    this.metadata = {};
    this.includeLoopback = false;
    this.includeIpv6 = false;
    this.includePrivateIpv4 = true;
    this.includeLinkLocalIpv4 = false;
    this.interfaceAllowlist = [];
    this.interfaceDenylist = [];
  }

  addLanAddr(addr) {
    this.lanAddrs.push(addr);
    return this;
  }

  addTag(tag) {
    this.tags.push(tag);
    return this;
  }

  insertMetadata(key, value) {
    this.metadata[key] = value;
    return this;
  }

  enableInterface(name) {
    this.interfaceAllowlist.push(name);
    return this;
  }

  disableInterface(name) {
    this.interfaceDenylist.push(name);
    return this;
  }

  _intoHandle(bindings) {
    const handle = checkPtr(
      bindings,
      bindings.announceSpecNew(
        this.networkId,
        this.nodeId,
        this.service,
        this.displayName,
        this.port,
      ),
    );
    try {
      checkBool(bindings, bindings.announceSpecSetAutoLanAddrs(handle, this.autoLanAddrs));
      checkBool(bindings, bindings.announceSpecSetTtlSecs(handle, this.ttlSecs));
      checkBool(bindings, bindings.announceSpecSetIncludeLoopback(handle, this.includeLoopback));
      checkBool(bindings, bindings.announceSpecSetIncludeIpv6(handle, this.includeIpv6));
      checkBool(
        bindings,
        bindings.announceSpecSetIncludePrivateIpv4(handle, this.includePrivateIpv4),
      );
      checkBool(
        bindings,
        bindings.announceSpecSetIncludeLinkLocalIpv4(handle, this.includeLinkLocalIpv4),
      );
      for (const addr of this.lanAddrs) {
        checkBool(bindings, bindings.announceSpecAddLanAddr(handle, addr));
      }
      for (const tag of this.tags) {
        checkBool(bindings, bindings.announceSpecAddTag(handle, tag));
      }
      for (const [key, value] of Object.entries(this.metadata)) {
        checkBool(bindings, bindings.announceSpecInsertMetadata(handle, key, value));
      }
      for (const interfaceName of this.interfaceAllowlist) {
        checkBool(bindings, bindings.announceSpecEnableInterface(handle, interfaceName));
      }
      for (const interfaceName of this.interfaceDenylist) {
        checkBool(bindings, bindings.announceSpecDisableInterface(handle, interfaceName));
      }
      return handle;
    } catch (error) {
      bindings.announceSpecFree(handle);
      throw error;
    }
  }
}

export class AnnounceHandle {
  constructor(bindings, handle) {
    this.bindings = bindings;
    this.handle = handle;
  }

  close() {
    if (this.handle) {
      this.bindings.announceStop(this.handle);
      this.handle = null;
    }
  }
}

export class WatchHandle {
  constructor(bindings, handle, callbackRef) {
    this.bindings = bindings;
    this.handle = handle;
    this.callbackRef = callbackRef;
  }

  close() {
    if (this.handle) {
      this.bindings.watchStop(this.handle);
      this.handle = null;
      this.callbackRef = null;
    }
  }
}

export class Client {
  constructor(serverUrl, bearerToken = "", options = {}) {
    this.bindings = createBindings(options.libraryPath);
    this.handle = checkPtr(this.bindings, this.bindings.clientNew(serverUrl, bearerToken));
    this.setIncludeLoopback(options.includeLoopback ?? false);
    this.setIncludeIpv6(options.includeIpv6 ?? false);
    this.setIncludePrivateIpv4(options.includePrivateIpv4 ?? true);
    this.setIncludeLinkLocalIpv4(options.includeLinkLocalIpv4 ?? false);
    if (options.timeoutMs !== undefined) {
      this.setTimeoutMs(options.timeoutMs);
    }
    if (options.reconnectBackoffMs !== undefined) {
      this.setReconnectBackoffMs(
        options.reconnectBackoffMs[0],
        options.reconnectBackoffMs[1],
      );
    }
  }

  close() {
    if (this.handle) {
      this.bindings.clientFree(this.handle);
      this.handle = null;
    }
  }

  setServerUrl(serverUrl) {
    checkBool(this.bindings, this.bindings.clientSetServerUrl(this.handle, serverUrl));
    return this;
  }

  setBearerToken(bearerToken) {
    checkBool(this.bindings, this.bindings.clientSetBearerToken(this.handle, bearerToken));
    return this;
  }

  setTimeoutMs(timeoutMs) {
    checkBool(this.bindings, this.bindings.clientSetTimeoutMs(this.handle, timeoutMs));
    return this;
  }

  setReconnectBackoffMs(minMs, maxMs) {
    checkBool(
      this.bindings,
      this.bindings.clientSetReconnectBackoffMs(this.handle, minMs, maxMs),
    );
    return this;
  }

  setIncludeLoopback(on) {
    checkBool(this.bindings, this.bindings.clientSetIncludeLoopback(this.handle, on));
    return this;
  }

  setIncludeIpv6(on) {
    checkBool(this.bindings, this.bindings.clientSetIncludeIpv6(this.handle, on));
    return this;
  }

  setIncludePrivateIpv4(on) {
    checkBool(this.bindings, this.bindings.clientSetIncludePrivateIpv4(this.handle, on));
    return this;
  }

  setIncludeLinkLocalIpv4(on) {
    checkBool(this.bindings, this.bindings.clientSetIncludeLinkLocalIpv4(this.handle, on));
    return this;
  }

  enableInterface(name) {
    checkBool(this.bindings, this.bindings.clientEnableInterface(this.handle, name));
    return this;
  }

  disableInterface(name) {
    checkBool(this.bindings, this.bindings.clientDisableInterface(this.handle, name));
    return this;
  }

  clearInterfaceFilters() {
    checkBool(this.bindings, this.bindings.clientClearInterfaceFilters(this.handle));
    return this;
  }

  discover(filterSpec) {
    const filterHandle = filterSpec._intoHandle(this.bindings);
    try {
      return readJsonString(
        this.bindings,
        checkPtr(this.bindings, this.bindings.discover(this.handle, filterHandle)),
      );
    } finally {
      this.bindings.filterFree(filterHandle);
    }
  }

  resolveAnnounceAddrs(spec) {
    const specHandle = spec._intoHandle(this.bindings);
    try {
      return readJsonString(
        this.bindings,
        checkPtr(this.bindings, this.bindings.resolveAnnounceAddrsJson(this.handle, specHandle)),
      );
    } finally {
      this.bindings.announceSpecFree(specHandle);
    }
  }

  announceOnce(spec) {
    const specHandle = spec._intoHandle(this.bindings);
    try {
      return readJsonString(
        this.bindings,
        checkPtr(this.bindings, this.bindings.announceOnce(this.handle, specHandle)),
      );
    } finally {
      this.bindings.announceSpecFree(specHandle);
    }
  }

  announce(spec) {
    const specHandle = spec._intoHandle(this.bindings);
    try {
      return new AnnounceHandle(
        this.bindings,
        checkPtr(this.bindings, this.bindings.announceStartWithSpec(this.handle, specHandle)),
      );
    } finally {
      this.bindings.announceSpecFree(specHandle);
    }
  }

  watch(filterSpec, callback) {
    const filterHandle = filterSpec._intoHandle(this.bindings);
    try {
      const callbackRef = this.bindings.watchCallback((payload) => {
        callback(JSON.parse(payload));
      });
      return new WatchHandle(
        this.bindings,
        checkPtr(
          this.bindings,
          this.bindings.watchStartWithFilter(
            this.handle,
            filterHandle,
            callbackRef,
            null,
          ),
        ),
        callbackRef,
      );
    } finally {
      this.bindings.filterFree(filterHandle);
    }
  }
}
