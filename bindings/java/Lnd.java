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

    public static final class DiscoveryFilter {
        public final String networkId;
        public String service;
        public final List<String> tags = new ArrayList<>();

        public DiscoveryFilter(String networkId) {
            this.networkId = networkId;
        }

        public DiscoveryFilter withService(String value) {
            this.service = value;
            return this;
        }

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

        public AnnounceSpec addLanAddr(String value) {
            this.lanAddrs.add(value);
            return this;
        }

        public AnnounceSpec addTag(String value) {
            this.tags.add(value);
            return this;
        }

        public AnnounceSpec insertMetadata(String key, String value) {
            this.metadata.put(key, value);
            return this;
        }

        public AnnounceSpec enableInterface(String value) {
            this.interfaceAllowlist.add(value);
            return this;
        }

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

    public static final class AnnounceHandle implements AutoCloseable {
        private Pointer handle;

        private AnnounceHandle(Pointer handle) {
            this.handle = handle;
        }

        @Override
        public void close() {
            if (handle != null) {
                NativeApi.INSTANCE.lnd_announce_stop(handle);
                handle = null;
            }
        }
    }

    public static final class WatchHandle implements AutoCloseable {
        private Pointer handle;
        private final NativeApi.WatchCallback callbackRef;

        private WatchHandle(Pointer handle, NativeApi.WatchCallback callbackRef) {
            this.handle = handle;
            this.callbackRef = callbackRef;
        }

        @Override
        public void close() {
            if (handle != null) {
                NativeApi.INSTANCE.lnd_watch_stop(handle);
                handle = null;
            }
        }
    }

    public interface EventHandler {
        void onEvent(String json);
    }

    public static final class Client implements AutoCloseable {
        private Pointer handle;

        public Client(String serverUrl, String bearerToken) {
            this.handle = checkPtr(NativeApi.INSTANCE.lnd_client_new(serverUrl, bearerToken));
        }

        @Override
        public void close() {
            if (handle != null) {
                NativeApi.INSTANCE.lnd_client_free(handle);
                handle = null;
            }
        }

        public Client setServerUrl(String value) {
            checkBool(NativeApi.INSTANCE.lnd_client_set_server_url(handle, value));
            return this;
        }

        public Client setBearerToken(String value) {
            checkBool(NativeApi.INSTANCE.lnd_client_set_bearer_token(handle, value));
            return this;
        }

        public Client setTimeoutMs(long value) {
            checkBool(NativeApi.INSTANCE.lnd_client_set_timeout_ms(handle, value));
            return this;
        }

        public Client setReconnectBackoffMs(long minMs, long maxMs) {
            checkBool(NativeApi.INSTANCE.lnd_client_set_reconnect_backoff_ms(handle, minMs, maxMs));
            return this;
        }

        public Client setIncludeLoopback(boolean on) {
            checkBool(NativeApi.INSTANCE.lnd_client_set_include_loopback(handle, on));
            return this;
        }

        public Client setIncludeIpv6(boolean on) {
            checkBool(NativeApi.INSTANCE.lnd_client_set_include_ipv6(handle, on));
            return this;
        }

        public Client setIncludePrivateIpv4(boolean on) {
            checkBool(NativeApi.INSTANCE.lnd_client_set_include_private_ipv4(handle, on));
            return this;
        }

        public Client setIncludeLinkLocalIpv4(boolean on) {
            checkBool(NativeApi.INSTANCE.lnd_client_set_include_link_local_ipv4(handle, on));
            return this;
        }

        public Client enableInterface(String value) {
            checkBool(NativeApi.INSTANCE.lnd_client_enable_interface(handle, value));
            return this;
        }

        public Client disableInterface(String value) {
            checkBool(NativeApi.INSTANCE.lnd_client_disable_interface(handle, value));
            return this;
        }

        public String discoverJson(DiscoveryFilter filter) {
            Pointer filterHandle = filter.intoHandle();
            try {
                return takeString(checkPtr(NativeApi.INSTANCE.lnd_discover(handle, filterHandle)));
            } finally {
                NativeApi.INSTANCE.lnd_discovery_filter_free(filterHandle);
            }
        }

        public String resolveAnnounceAddrsJson(AnnounceSpec spec) {
            Pointer specHandle = spec.intoHandle();
            try {
                return takeString(checkPtr(NativeApi.INSTANCE.lnd_resolve_announce_addrs_json(handle, specHandle)));
            } finally {
                NativeApi.INSTANCE.lnd_announce_spec_free(specHandle);
            }
        }

        public String announceOnceJson(AnnounceSpec spec) {
            Pointer specHandle = spec.intoHandle();
            try {
                return takeString(checkPtr(NativeApi.INSTANCE.lnd_announce_once(handle, specHandle)));
            } finally {
                NativeApi.INSTANCE.lnd_announce_spec_free(specHandle);
            }
        }

        public AnnounceHandle announce(AnnounceSpec spec) {
            Pointer specHandle = spec.intoHandle();
            try {
                return new AnnounceHandle(checkPtr(NativeApi.INSTANCE.lnd_announce_start_with_spec(handle, specHandle)));
            } finally {
                NativeApi.INSTANCE.lnd_announce_spec_free(specHandle);
            }
        }

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
