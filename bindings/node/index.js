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

/**
 * Error raised by the Node.js binding when the native library reports a failure.
 *
 * Typical causes include invalid arguments, failed HTTP requests, auth errors,
 * or malformed JSON returned by the server.
 */
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

/**
 * Discovery filter used by discover and watch calls.
 *
 * @example
 * const filter = new DiscoveryFilter("office-net")
 *   .withService("_demo._tcp")
 *   .addTag("printer");
 */
export class DiscoveryFilter {
  /**
   * Create a discovery filter for one logical network.
   *
   * @param {string} networkId Required discovery domain identifier.
   */
  constructor(networkId) {
    this.networkId = networkId;
    this.service = null;
    this.tags = [];
  }

  /**
   * Restrict matches to one service name.
   *
   * @param {string} service Service identifier such as `_demo._tcp`.
   * @returns {DiscoveryFilter} The same filter for chaining.
   */
  withService(service) {
    this.service = service;
    return this;
  }

  /**
   * Add one required tag to the filter.
   *
   * A node must contain every tag added here to match.
   *
   * @param {string} tag Required tag value.
   * @returns {DiscoveryFilter} The same filter for chaining.
   */
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

/**
 * Announce specification used by announceOnce, announce and resolveAnnounceAddrs.
 *
 * The object can mix explicit LAN addresses with automatic interface based
 * address discovery.
 *
 * @example
 * const spec = new AnnounceSpec("office-net", "node-1", "_demo._tcp", "Demo Node", 8080)
 *   .addTag("blue")
 *   .insertMetadata("role", "api");
 */
export class AnnounceSpec {
  /**
   * Create a node registration spec.
   *
   * @param {string} networkId Logical discovery domain.
   * @param {string} nodeId Stable node identifier.
   * @param {string} service Service identifier such as `_demo._tcp`.
   * @param {string} displayName Human readable label.
   * @param {number} port LAN service port peers should connect to.
   */
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

  /**
   * Append one explicit LAN host:port address.
   *
   * Keep `autoLanAddrs` enabled to merge these values with automatically
   * discovered interface addresses.
   *
   * @param {string} addr Address in `host:port` form.
   * @returns {AnnounceSpec} The same spec for chaining.
   */
  addLanAddr(addr) {
    this.lanAddrs.push(addr);
    return this;
  }

  /**
   * Append one tag to the announced node.
   *
   * @param {string} tag Tag value to advertise.
   * @returns {AnnounceSpec} The same spec for chaining.
   */
  addTag(tag) {
    this.tags.push(tag);
    return this;
  }

  /**
   * Add or replace one metadata entry.
   *
   * @param {string} key Metadata key.
   * @param {string} value Metadata value.
   * @returns {AnnounceSpec} The same spec for chaining.
   */
  insertMetadata(key, value) {
    this.metadata[key] = value;
    return this;
  }

  /**
   * Allow one interface name during automatic address selection.
   *
   * @param {string} name Interface name such as `en0` or `eth0`.
   * @returns {AnnounceSpec} The same spec for chaining.
   */
  enableInterface(name) {
    this.interfaceAllowlist.push(name);
    return this;
  }

  /**
   * Deny one interface name during automatic address selection.
   *
   * @param {string} name Interface name such as `en0` or `eth0`.
   * @returns {AnnounceSpec} The same spec for chaining.
   */
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

/**
 * Handle returned by Client.announce.
 *
 * Call close to stop the background announce loop.
 */
export class AnnounceHandle {
  constructor(bindings, handle) {
    this.bindings = bindings;
    this.handle = handle;
  }

  /**
   * Stop the background announce loop.
   *
   * The method is idempotent and may be called multiple times.
   */
  close() {
    if (this.handle) {
      this.bindings.announceStop(this.handle);
      this.handle = null;
    }
  }
}

/**
 * Handle returned by Client.watch.
 *
 * Call close to stop the reconnecting SSE watch loop.
 */
export class WatchHandle {
  constructor(bindings, handle, callbackRef) {
    this.bindings = bindings;
    this.handle = handle;
    this.callbackRef = callbackRef;
  }

  /**
   * Stop the watch loop and release the native callback reference.
   *
   * The method is idempotent and may be called multiple times.
   */
  close() {
    if (this.handle) {
      this.bindings.watchStop(this.handle);
      this.handle = null;
      this.callbackRef = null;
    }
  }
}

/**
 * High level Node.js client for discovery, announce and watch operations.
 *
 * @example
 * const client = new Client("https://registry.example.com", "secret-token");
 * const nodes = client.discover(new DiscoveryFilter("office-net"));
 * console.log(nodes);
 * client.close();
 */
export class Client {
  /**
   * Create a client bound to one server base URL.
   *
   * @param {string} serverUrl Server root URL such as `https://registry.example.com`.
   * @param {string} [bearerToken=""] Optional Bearer token.
   * @param {object} [options={}] Optional client defaults.
   * @param {string} [options.libraryPath] Explicit path to the native library.
   * @param {boolean} [options.includeLoopback=false] Whether auto discovery may include loopback.
   * @param {boolean} [options.includeIpv6=false] Whether auto discovery may include IPv6.
   * @param {boolean} [options.includePrivateIpv4=true] Whether auto discovery may include private IPv4.
   * @param {boolean} [options.includeLinkLocalIpv4=false] Whether auto discovery may include link local IPv4.
   * @param {number} [options.timeoutMs] HTTP timeout in milliseconds.
   * @param {[number, number]} [options.reconnectBackoffMs] Min and max reconnect backoff in milliseconds.
   * @throws {LndError} Thrown when the native library cannot be loaded or the client cannot be created.
   */
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

  /**
   * Release the underlying native client handle.
   *
   * After calling this method, the instance must not be used again.
   */
  close() {
    if (this.handle) {
      this.bindings.clientFree(this.handle);
      this.handle = null;
    }
  }

  /**
   * Update the server base URL for future requests.
   *
   * @param {string} serverUrl Server root URL.
   * @returns {Client} The same client for chaining.
   * @throws {LndError} Thrown when the URL cannot be applied by the native client.
   */
  setServerUrl(serverUrl) {
    checkBool(this.bindings, this.bindings.clientSetServerUrl(this.handle, serverUrl));
    return this;
  }

  /**
   * Update the Bearer token used for future requests.
   *
   * @param {string} bearerToken Token value. Pass an empty string to clear it.
   * @returns {Client} The same client for chaining.
   * @throws {LndError} Thrown when the token cannot be applied by the native client.
   */
  setBearerToken(bearerToken) {
    checkBool(this.bindings, this.bindings.clientSetBearerToken(this.handle, bearerToken));
    return this;
  }

  /**
   * Set the HTTP timeout used by future requests.
   *
   * @param {number} timeoutMs Timeout in milliseconds.
   * @returns {Client} The same client for chaining.
   * @throws {LndError} Thrown when the timeout cannot be applied.
   */
  setTimeoutMs(timeoutMs) {
    checkBool(this.bindings, this.bindings.clientSetTimeoutMs(this.handle, timeoutMs));
    return this;
  }

  /**
   * Configure the reconnect backoff range for announce and watch loops.
   *
   * @param {number} minMs Minimum backoff in milliseconds.
   * @param {number} maxMs Maximum backoff in milliseconds.
   * @returns {Client} The same client for chaining.
   * @throws {LndError} Thrown when the backoff range cannot be applied.
   */
  setReconnectBackoffMs(minMs, maxMs) {
    checkBool(
      this.bindings,
      this.bindings.clientSetReconnectBackoffMs(this.handle, minMs, maxMs),
    );
    return this;
  }

  /**
   * Control whether automatic address resolution may include loopback addresses.
   *
   * @param {boolean} on Whether loopback addresses are allowed.
   * @returns {Client} The same client for chaining.
   * @throws {LndError} Thrown when the policy cannot be applied.
   */
  setIncludeLoopback(on) {
    checkBool(this.bindings, this.bindings.clientSetIncludeLoopback(this.handle, on));
    return this;
  }

  /**
   * Control whether automatic address resolution may include IPv6 addresses.
   *
   * @param {boolean} on Whether IPv6 addresses are allowed.
   * @returns {Client} The same client for chaining.
   * @throws {LndError} Thrown when the policy cannot be applied.
   */
  setIncludeIpv6(on) {
    checkBool(this.bindings, this.bindings.clientSetIncludeIpv6(this.handle, on));
    return this;
  }

  /**
   * Control whether automatic address resolution may include private IPv4 addresses.
   *
   * @param {boolean} on Whether private IPv4 addresses are allowed.
   * @returns {Client} The same client for chaining.
   * @throws {LndError} Thrown when the policy cannot be applied.
   */
  setIncludePrivateIpv4(on) {
    checkBool(this.bindings, this.bindings.clientSetIncludePrivateIpv4(this.handle, on));
    return this;
  }

  /**
   * Control whether automatic address resolution may include link local IPv4 addresses.
   *
   * @param {boolean} on Whether link local IPv4 addresses are allowed.
   * @returns {Client} The same client for chaining.
   * @throws {LndError} Thrown when the policy cannot be applied.
   */
  setIncludeLinkLocalIpv4(on) {
    checkBool(this.bindings, this.bindings.clientSetIncludeLinkLocalIpv4(this.handle, on));
    return this;
  }

  /**
   * Allow one interface during automatic address resolution.
   *
   * @param {string} name Interface name such as `en0` or `eth0`.
   * @returns {Client} The same client for chaining.
   * @throws {LndError} Thrown when the rule cannot be applied.
   */
  enableInterface(name) {
    checkBool(this.bindings, this.bindings.clientEnableInterface(this.handle, name));
    return this;
  }

  /**
   * Deny one interface during automatic address resolution.
   *
   * @param {string} name Interface name such as `en0` or `eth0`.
   * @returns {Client} The same client for chaining.
   * @throws {LndError} Thrown when the rule cannot be applied.
   */
  disableInterface(name) {
    checkBool(this.bindings, this.bindings.clientDisableInterface(this.handle, name));
    return this;
  }

  /**
   * Clear all interface allow and deny rules.
   *
   * @returns {Client} The same client for chaining.
   * @throws {LndError} Thrown when the filters cannot be cleared.
   */
  clearInterfaceFilters() {
    checkBool(this.bindings, this.bindings.clientClearInterfaceFilters(this.handle));
    return this;
  }

  /**
   * Perform one discovery request and parse the JSON response.
   *
   * @param {DiscoveryFilter} filterSpec Discovery filter to send.
   * @returns {Array<object>} Parsed discovered nodes.
   * @throws {LndError} Thrown when request setup or the native call fails.
   */
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

  /**
   * Resolve the final LAN address list for one announce spec.
   *
   * @param {AnnounceSpec} spec Announce spec to resolve locally.
   * @returns {Array<string>} Final deduplicated `host:port` addresses.
   * @throws {LndError} Thrown when local interface enumeration or argument conversion fails.
   */
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

  /**
   * Perform one registration request and return the normalized node record.
   *
   * @param {AnnounceSpec} spec Node registration spec.
   * @returns {object} Parsed discovered node returned by the server.
   * @throws {LndError} Thrown when address resolution or the request fails.
   */
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

  /**
   * Start a background announce loop.
   *
   * The loop renews the lease until the returned handle is closed.
   *
   * @param {AnnounceSpec} spec Node registration spec.
   * @returns {AnnounceHandle} Handle used to stop the background loop.
   * @throws {LndError} Thrown when the loop cannot be started.
   */
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

  /**
   * Start a reconnecting watch loop.
   *
   * callback receives parsed SSE envelopes. On cursor replay failure the stream
   * emits a `reset` event followed by a fresh `snapshot`.
   *
   * @param {DiscoveryFilter} filterSpec Discovery filter to watch.
   * @param {(event: object) => void} callback Function called for every event.
   * @returns {WatchHandle} Handle used to stop the background watch loop.
   * @throws {LndError} Thrown when the watch cannot be started.
   */
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
