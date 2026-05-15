import com.sun.jna.Callback;
import com.sun.jna.Library;
import com.sun.jna.Native;
import com.sun.jna.Pointer;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

public final class Lnd {
    private Lnd() {}

    /**
     * Raw JNA mapping for the native C ABI.
     *
     * Most callers should prefer the higher level nested classes in this file.
     */
    public interface NativeApi extends Library {
        NativeApi INSTANCE = Native.load("lnd", NativeApi.class);

        Pointer lnd_client_new(String serverUrl, String bearerToken);
        void lnd_client_free(Pointer handle);
        boolean lnd_client_set_server_url(Pointer handle, String serverUrl);
        boolean lnd_client_set_bearer_token(Pointer handle, String bearerToken);
        boolean lnd_client_set_timeout_ms(Pointer handle, long timeoutMs);
        boolean lnd_client_set_reconnect_backoff_ms(Pointer handle, long minMs, long maxMs);
        boolean lnd_client_set_include_loopback(Pointer handle, boolean on);
        boolean lnd_client_set_include_ipv6(Pointer handle, boolean on);
        boolean lnd_client_set_include_private_ipv4(Pointer handle, boolean on);
        boolean lnd_client_set_include_link_local_ipv4(Pointer handle, boolean on);
        boolean lnd_client_enable_interface(Pointer handle, String interfaceName);
        boolean lnd_client_disable_interface(Pointer handle, String interfaceName);

        Pointer lnd_discovery_filter_new(String networkId);
        void lnd_discovery_filter_free(Pointer handle);
        boolean lnd_discovery_filter_set_service(Pointer handle, String service);
        boolean lnd_discovery_filter_add_tag(Pointer handle, String tag);

        Pointer lnd_announce_spec_new(
            String networkId,
            String nodeId,
            String service,
            String displayName,
            int port
        );
        void lnd_announce_spec_free(Pointer handle);
        boolean lnd_announce_spec_set_auto_lan_addrs(Pointer handle, boolean on);
        boolean lnd_announce_spec_add_lan_addr(Pointer handle, String addr);
        boolean lnd_announce_spec_set_include_loopback(Pointer handle, boolean on);
        boolean lnd_announce_spec_set_include_ipv6(Pointer handle, boolean on);
        boolean lnd_announce_spec_set_include_private_ipv4(Pointer handle, boolean on);
        boolean lnd_announce_spec_set_include_link_local_ipv4(Pointer handle, boolean on);
        boolean lnd_announce_spec_enable_interface(Pointer handle, String interfaceName);
        boolean lnd_announce_spec_disable_interface(Pointer handle, String interfaceName);
        boolean lnd_announce_spec_add_tag(Pointer handle, String tag);
        boolean lnd_announce_spec_insert_metadata(Pointer handle, String key, String value);
        boolean lnd_announce_spec_set_ttl_secs(Pointer handle, long ttlSecs);

        Pointer lnd_discover(Pointer client, Pointer filter);
        Pointer lnd_resolve_announce_addrs_json(Pointer client, Pointer spec);
        Pointer lnd_announce_once(Pointer client, Pointer spec);
        Pointer lnd_announce_start_with_spec(Pointer client, Pointer spec);
        void lnd_announce_stop(Pointer handle);

        interface WatchCallback extends Callback {
            void invoke(String payload, Pointer userData);
        }

        Pointer lnd_watch_start_with_filter(
            Pointer client,
            Pointer filter,
            WatchCallback callback,
            Pointer userData
        );
        void lnd_watch_stop(Pointer handle);

        void lnd_string_free(Pointer value);
        String lnd_last_error();
    }

    /**
     * Runtime exception raised when the native layer rejects an operation.
     *
     * Typical causes include invalid arguments, transport failures, auth
     * failures or malformed server responses.
     */
    public static final class LndException extends RuntimeException {
        public LndException(String message) {
            super(message);
        }
    }

    private static void checkBool(boolean ok) {
        if (!ok) {
            throw new LndException(NativeApi.INSTANCE.lnd_last_error());
        }
    }

    private static Pointer checkPtr(Pointer ptr) {
        if (ptr == null) {
            throw new LndException(NativeApi.INSTANCE.lnd_last_error());
        }
        return ptr;
    }

    private static String takeString(Pointer ptr) {
        try {
            return ptr.getString(0);
        } finally {
            NativeApi.INSTANCE.lnd_string_free(ptr);
        }
    }

    /**
     * Discovery filter used by {@link Client#discoverJson(DiscoveryFilter)} and
     * {@link Client#watch(DiscoveryFilter, EventHandler)}.
     *
     * <p>Example:
     *
     * <pre>{@code
     * Lnd.DiscoveryFilter filter = new Lnd.DiscoveryFilter("office-net")
     *     .withService("_demo._tcp")
     *     .addTag("printer");
     * }</pre>
     */
    public static final class DiscoveryFilter {
        public final String networkId;
        public String service;
        public final List<String> tags = new ArrayList<>();

        /**
         * Create a filter for one logical discovery domain.
         *
         * @param networkId required network identifier
         */
        public DiscoveryFilter(String networkId) {
            this.networkId = networkId;
        }

        /**
         * Restrict matches to one service name.
         *
         * @param value service identifier such as {@code _demo._tcp}
         * @return this filter for chaining
         */
        public DiscoveryFilter withService(String value) {
            this.service = value;
            return this;
        }

        /**
         * Add one required tag.
         *
         * @param value tag value
         * @return this filter for chaining
         */
        public DiscoveryFilter addTag(String value) {
            this.tags.add(value);
            return this;
        }

        private Pointer intoHandle() {
            Pointer handle = checkPtr(NativeApi.INSTANCE.lnd_discovery_filter_new(networkId));
            try {
                if (service != null) {
                    checkBool(NativeApi.INSTANCE.lnd_discovery_filter_set_service(handle, service));
                }
                for (String tag : tags) {
                    checkBool(NativeApi.INSTANCE.lnd_discovery_filter_add_tag(handle, tag));
                }
                return handle;
            } catch (RuntimeException error) {
                NativeApi.INSTANCE.lnd_discovery_filter_free(handle);
                throw error;
            }
        }
    }

    /**
     * Announce specification used by register and watch related operations.
     *
     * <p>The spec can merge explicit LAN addresses with automatic interface
     * based address discovery.
     */
    public static final class AnnounceSpec {
        public final String networkId;
        public final String nodeId;
        public final String service;
        public final String displayName;
        public final int port;
        public boolean autoLanAddrs = true;
        public long ttlSecs = 30;
        public final List<String> lanAddrs = new ArrayList<>();
        public final List<String> tags = new ArrayList<>();
        public final Map<String, String> metadata = new LinkedHashMap<>();
        public boolean includeLoopback = false;
        public boolean includeIpv6 = false;
        public boolean includePrivateIpv4 = true;
        public boolean includeLinkLocalIpv4 = false;
        public final List<String> interfaceAllowlist = new ArrayList<>();
        public final List<String> interfaceDenylist = new ArrayList<>();

        /**
         * Create a node registration specification.
         *
         * @param networkId logical discovery domain
         * @param nodeId stable node identifier
         * @param service service identifier such as {@code _demo._tcp}
         * @param displayName human readable label
         * @param port service port visible to peers
         */
        public AnnounceSpec(
            String networkId,
            String nodeId,
            String service,
            String displayName,
            int port
        ) {
            this.networkId = networkId;
            this.nodeId = nodeId;
            this.service = service;
            this.displayName = displayName;
            this.port = port;
        }

        /**
         * Append one explicit LAN address in {@code host:port} form.
         *
         * @param value explicit LAN address
         * @return this spec for chaining
         */
        public AnnounceSpec addLanAddr(String value) {
            this.lanAddrs.add(value);
            return this;
        }

        /**
         * Append one announce tag.
         *
         * @param value tag value
         * @return this spec for chaining
         */
        public AnnounceSpec addTag(String value) {
            this.tags.add(value);
            return this;
        }

        /**
         * Add or replace one metadata entry.
         *
         * @param key metadata key
         * @param value metadata value
         * @return this spec for chaining
         */
        public AnnounceSpec insertMetadata(String key, String value) {
            this.metadata.put(key, value);
            return this;
        }

        /**
         * Allow one interface during automatic address discovery.
         *
         * @param value interface name such as {@code en0} or {@code eth0}
         * @return this spec for chaining
         */
        public AnnounceSpec enableInterface(String value) {
            this.interfaceAllowlist.add(value);
            return this;
        }

        /**
         * Deny one interface during automatic address discovery.
         *
         * @param value interface name such as {@code en0} or {@code eth0}
         * @return this spec for chaining
         */
        public AnnounceSpec disableInterface(String value) {
            this.interfaceDenylist.add(value);
            return this;
        }

        private Pointer intoHandle() {
            Pointer handle = checkPtr(NativeApi.INSTANCE.lnd_announce_spec_new(
                networkId,
                nodeId,
                service,
                displayName,
                port
            ));
            try {
                checkBool(NativeApi.INSTANCE.lnd_announce_spec_set_auto_lan_addrs(handle, autoLanAddrs));
                checkBool(NativeApi.INSTANCE.lnd_announce_spec_set_ttl_secs(handle, ttlSecs));
                checkBool(NativeApi.INSTANCE.lnd_announce_spec_set_include_loopback(handle, includeLoopback));
                checkBool(NativeApi.INSTANCE.lnd_announce_spec_set_include_ipv6(handle, includeIpv6));
                checkBool(NativeApi.INSTANCE.lnd_announce_spec_set_include_private_ipv4(handle, includePrivateIpv4));
                checkBool(NativeApi.INSTANCE.lnd_announce_spec_set_include_link_local_ipv4(handle, includeLinkLocalIpv4));
                for (String addr : lanAddrs) {
                    checkBool(NativeApi.INSTANCE.lnd_announce_spec_add_lan_addr(handle, addr));
                }
                for (String tag : tags) {
                    checkBool(NativeApi.INSTANCE.lnd_announce_spec_add_tag(handle, tag));
                }
                for (Map.Entry<String, String> entry : metadata.entrySet()) {
                    checkBool(NativeApi.INSTANCE.lnd_announce_spec_insert_metadata(
                        handle,
                        entry.getKey(),
                        entry.getValue()
                    ));
                }
                for (String interfaceName : interfaceAllowlist) {
                    checkBool(NativeApi.INSTANCE.lnd_announce_spec_enable_interface(handle, interfaceName));
                }
                for (String interfaceName : interfaceDenylist) {
                    checkBool(NativeApi.INSTANCE.lnd_announce_spec_disable_interface(handle, interfaceName));
                }
                return handle;
            } catch (RuntimeException error) {
                NativeApi.INSTANCE.lnd_announce_spec_free(handle);
                throw error;
            }
        }
    }

    /**
     * Handle for a background announce loop.
     *
     * <p>Close the handle to stop lease renewals.
     */
    public static final class AnnounceHandle implements AutoCloseable {
        private Pointer handle;

        private AnnounceHandle(Pointer handle) {
            this.handle = handle;
        }

        /**
         * Stop the background announce loop.
         *
         * <p>The method is idempotent.
         */
        @Override
        public void close() {
            if (handle != null) {
                NativeApi.INSTANCE.lnd_announce_stop(handle);
                handle = null;
            }
        }
    }

    /**
     * Handle for a reconnecting watch loop.
     *
     * <p>Close the handle to stop the SSE stream and release the callback.
     */
    public static final class WatchHandle implements AutoCloseable {
        private Pointer handle;
        private final NativeApi.WatchCallback callbackRef;

        private WatchHandle(Pointer handle, NativeApi.WatchCallback callbackRef) {
            this.handle = handle;
            this.callbackRef = callbackRef;
        }

        /**
         * Stop the background watch loop.
         *
         * <p>The method is idempotent.
         */
        @Override
        public void close() {
            if (handle != null) {
                NativeApi.INSTANCE.lnd_watch_stop(handle);
                handle = null;
            }
        }
    }

    /**
     * Callback used by {@link Client#watch(DiscoveryFilter, EventHandler)}.
     *
     * <p>The payload is a UTF-8 JSON string representing one watch event
     * envelope.
     */
    public interface EventHandler {
        void onEvent(String json);
    }

    /**
     * High level Java client for discovery, announce and watch operations.
     *
     * <p>Example:
     *
     * <pre>{@code
     * try (Lnd.Client client = new Lnd.Client("https://registry.example.com", "secret-token")) {
     *     String json = client.discoverJson(new Lnd.DiscoveryFilter("office-net"));
     *     System.out.println(json);
     * }
     * }</pre>
     */
    public static final class Client implements AutoCloseable {
        private Pointer handle;

        /**
         * Create a client bound to one server base URL.
         *
         * @param serverUrl server root URL
         * @param bearerToken optional Bearer token, may be empty
         * @throws LndException if the native client cannot be created
         */
        public Client(String serverUrl, String bearerToken) {
            this.handle = checkPtr(NativeApi.INSTANCE.lnd_client_new(serverUrl, bearerToken));
        }

        /**
         * Release the native client handle.
         *
         * <p>After this call the instance must not be used again.
         */
        @Override
        public void close() {
            if (handle != null) {
                NativeApi.INSTANCE.lnd_client_free(handle);
                handle = null;
            }
        }

        /**
         * Update the server base URL for future requests.
         *
         * @param value server root URL
         * @return this client for chaining
         * @throws LndException if the native layer rejects the value
         */
        public Client setServerUrl(String value) {
            checkBool(NativeApi.INSTANCE.lnd_client_set_server_url(handle, value));
            return this;
        }

        /**
         * Update the Bearer token used for future requests.
         *
         * @param value token value, use an empty string to clear it
         * @return this client for chaining
         * @throws LndException if the native layer rejects the value
         */
        public Client setBearerToken(String value) {
            checkBool(NativeApi.INSTANCE.lnd_client_set_bearer_token(handle, value));
            return this;
        }

        /**
         * Set the HTTP timeout used by future requests.
         *
         * @param value timeout in milliseconds
         * @return this client for chaining
         * @throws LndException if the native layer rejects the value
         */
        public Client setTimeoutMs(long value) {
            checkBool(NativeApi.INSTANCE.lnd_client_set_timeout_ms(handle, value));
            return this;
        }

        /**
         * Configure reconnect backoff for background announce and watch loops.
         *
         * @param minMs minimum reconnect delay in milliseconds
         * @param maxMs maximum reconnect delay in milliseconds
         * @return this client for chaining
         * @throws LndException if the native layer rejects the values
         */
        public Client setReconnectBackoffMs(long minMs, long maxMs) {
            checkBool(NativeApi.INSTANCE.lnd_client_set_reconnect_backoff_ms(handle, minMs, maxMs));
            return this;
        }

        /**
         * Control whether automatic address discovery may include loopback.
         *
         * @param on whether loopback addresses are allowed
         * @return this client for chaining
         * @throws LndException if the native layer rejects the value
         */
        public Client setIncludeLoopback(boolean on) {
            checkBool(NativeApi.INSTANCE.lnd_client_set_include_loopback(handle, on));
            return this;
        }

        /**
         * Control whether automatic address discovery may include IPv6.
         *
         * @param on whether IPv6 addresses are allowed
         * @return this client for chaining
         * @throws LndException if the native layer rejects the value
         */
        public Client setIncludeIpv6(boolean on) {
            checkBool(NativeApi.INSTANCE.lnd_client_set_include_ipv6(handle, on));
            return this;
        }

        /**
         * Control whether automatic address discovery may include private IPv4.
         *
         * @param on whether private IPv4 addresses are allowed
         * @return this client for chaining
         * @throws LndException if the native layer rejects the value
         */
        public Client setIncludePrivateIpv4(boolean on) {
            checkBool(NativeApi.INSTANCE.lnd_client_set_include_private_ipv4(handle, on));
            return this;
        }

        /**
         * Control whether automatic address discovery may include link local IPv4.
         *
         * @param on whether link local IPv4 addresses are allowed
         * @return this client for chaining
         * @throws LndException if the native layer rejects the value
         */
        public Client setIncludeLinkLocalIpv4(boolean on) {
            checkBool(NativeApi.INSTANCE.lnd_client_set_include_link_local_ipv4(handle, on));
            return this;
        }

        /**
         * Allow one interface during automatic address discovery.
         *
         * @param value interface name such as {@code en0} or {@code eth0}
         * @return this client for chaining
         * @throws LndException if the native layer rejects the value
         */
        public Client enableInterface(String value) {
            checkBool(NativeApi.INSTANCE.lnd_client_enable_interface(handle, value));
            return this;
        }

        /**
         * Deny one interface during automatic address discovery.
         *
         * @param value interface name such as {@code en0} or {@code eth0}
         * @return this client for chaining
         * @throws LndException if the native layer rejects the value
         */
        public Client disableInterface(String value) {
            checkBool(NativeApi.INSTANCE.lnd_client_disable_interface(handle, value));
            return this;
        }

        /**
         * Perform one discovery request and return the raw JSON payload.
         *
         * @param filter discovery filter
         * @return JSON array wrapper returned by the server
         * @throws LndException if request setup or the native call fails
         */
        public String discoverJson(DiscoveryFilter filter) {
            Pointer filterHandle = filter.intoHandle();
            try {
                return takeString(checkPtr(NativeApi.INSTANCE.lnd_discover(handle, filterHandle)));
            } finally {
                NativeApi.INSTANCE.lnd_discovery_filter_free(filterHandle);
            }
        }

        /**
         * Resolve the final LAN address list for one announce specification.
         *
         * @param spec announce specification
         * @return JSON encoded array of `host:port` strings
         * @throws LndException if local address resolution fails
         */
        public String resolveAnnounceAddrsJson(AnnounceSpec spec) {
            Pointer specHandle = spec.intoHandle();
            try {
                return takeString(checkPtr(NativeApi.INSTANCE.lnd_resolve_announce_addrs_json(handle, specHandle)));
            } finally {
                NativeApi.INSTANCE.lnd_announce_spec_free(specHandle);
            }
        }

        /**
         * Perform one registration request and return the server JSON response.
         *
         * @param spec announce specification
         * @return JSON encoded discovered node
         * @throws LndException if address resolution or the request fails
         */
        public String announceOnceJson(AnnounceSpec spec) {
            Pointer specHandle = spec.intoHandle();
            try {
                return takeString(checkPtr(NativeApi.INSTANCE.lnd_announce_once(handle, specHandle)));
            } finally {
                NativeApi.INSTANCE.lnd_announce_spec_free(specHandle);
            }
        }

        /**
         * Start a background announce loop.
         *
         * @param spec announce specification
         * @return handle used to stop the loop
         * @throws LndException if the native loop cannot be started
         */
        public AnnounceHandle announce(AnnounceSpec spec) {
            Pointer specHandle = spec.intoHandle();
            try {
                return new AnnounceHandle(checkPtr(NativeApi.INSTANCE.lnd_announce_start_with_spec(handle, specHandle)));
            } finally {
                NativeApi.INSTANCE.lnd_announce_spec_free(specHandle);
            }
        }

        /**
         * Start a reconnecting watch loop.
         *
         * <p>Each callback receives a UTF-8 JSON event envelope. On replay
         * failure the stream emits a {@code reset} event followed by a fresh
         * {@code snapshot}.
         *
         * @param filter discovery filter to watch
         * @param handler callback invoked for every JSON event
         * @return handle used to stop the loop
         * @throws LndException if the native watch cannot be started
         */
        public WatchHandle watch(DiscoveryFilter filter, EventHandler handler) {
            Pointer filterHandle = filter.intoHandle();
            try {
                NativeApi.WatchCallback callback = (payload, userData) -> handler.onEvent(payload);
                return new WatchHandle(
                    checkPtr(NativeApi.INSTANCE.lnd_watch_start_with_filter(handle, filterHandle, callback, null)),
                    callback
                );
            } finally {
                NativeApi.INSTANCE.lnd_discovery_filter_free(filterHandle);
            }
        }
    }
}
