package io.github.azazo1.lnd;

import java.io.BufferedReader;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.Inet4Address;
import java.net.Inet6Address;
import java.net.InetAddress;
import java.net.InterfaceAddress;
import java.net.NetworkInterface;
import java.net.URI;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.Comparator;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * `lnd` 的高层 Java 客户端入口.
 *
 * <p>功能简介:
 *
 * <ul>
 *   <li>执行一次性 discover
 *   <li>执行一次性 announce
 *   <li>启动后台 announce 续租循环
 *   <li>启动后台 watch 监听循环
 *   <li>推导 `network_id` 和 `reachability_scopes`
 * </ul>
 *
 * <p>设计目标:
 *
 * <ul>
 *   <li>尽量保持与 Rust / Go SDK 的能力和语义等价
 *   <li>避免引入 `JNI` 或额外三方运行时
 *   <li>可以直接用于 JVM 和 Android 项目
 * </ul>
 *
 * <p>最小示例:
 *
 * <pre>{@code
 * Client client = new Client("http://127.0.0.1:8765", "dev-token");
 * String networkId = client.resolveNetworkId();
 * List<String> scopes = client.listReachabilityScopes();
 *
 * DiscoveryFilter filter = new DiscoveryFilter()
 *     .withNetworkId(networkId)
 *     .withService("_demo._tcp");
 * for (String scope : scopes) {
 *     filter.addReachabilityScope(scope);
 * }
 *
 * System.out.println(client.discover(filter));
 * }</pre>
 */
public final class Client {
    /** 默认服务端 base URL. */
    public static final String DEFAULT_BASE_URL = "http://127.0.0.1:8765";
    /** 默认 HTTP 超时, 单位为毫秒. */
    public static final int DEFAULT_TIMEOUT_MILLIS = 10_000;
    /** 默认重连最小退避, 单位为毫秒. */
    public static final long DEFAULT_RECONNECT_BACKOFF_MIN_MILLIS = 500L;
    /** 默认重连最大退避, 单位为毫秒. */
    public static final long DEFAULT_RECONNECT_BACKOFF_MAX_MILLIS = 15_000L;

    private volatile String baseUrl;
    private volatile String bearerToken;
    private volatile int timeoutMillis = DEFAULT_TIMEOUT_MILLIS;
    private volatile long reconnectBackoffMinMillis = DEFAULT_RECONNECT_BACKOFF_MIN_MILLIS;
    private volatile long reconnectBackoffMaxMillis = DEFAULT_RECONNECT_BACKOFF_MAX_MILLIS;
    private AddressSelection defaultAddressSelection = AddressSelection.defaults();

    /**
     * 使用默认 Bearer token 创建 client.
     *
     * @param baseUrl 服务端 base URL, 例如 `http://127.0.0.1:8765`
     */
    public Client(String baseUrl) {
        this(baseUrl, "");
    }

    /**
     * 创建可复用的 client.
     *
     * @param baseUrl 服务端 base URL, 例如 `http://127.0.0.1:8765`
     * @param bearerToken 预共享 Bearer token, 可以为空字符串
     */
    public Client(String baseUrl, String bearerToken) {
        this.baseUrl = trimTrailingSlash(baseUrl == null ? DEFAULT_BASE_URL : baseUrl);
        this.bearerToken = bearerToken == null ? "" : bearerToken;
    }

    /**
     * 返回当前服务端 base URL.
     *
     * @return 当前 base URL
     */
    public String getBaseUrl() {
        return baseUrl;
    }

    /**
     * 设置服务端 base URL.
     *
     * @param baseUrl 新的服务端 base URL
     * @return 当前对象, 便于链式调用
     */
    public Client setBaseUrl(String baseUrl) {
        this.baseUrl = trimTrailingSlash(baseUrl);
        return this;
    }

    /**
     * 兼容 `serverUrl` 命名的 setter.
     *
     * @param serverUrl 新的服务端 base URL
     * @return 当前对象, 便于链式调用
     */
    public Client setServerUrl(String serverUrl) {
        return setBaseUrl(serverUrl);
    }

    /**
     * 返回当前 Bearer token.
     *
     * @return token
     */
    public String getBearerToken() {
        return bearerToken;
    }

    /**
     * 设置 Bearer token.
     *
     * @param bearerToken 新的 Bearer token, 传空字符串表示禁用
     * @return 当前对象, 便于链式调用
     */
    public Client setBearerToken(String bearerToken) {
        this.bearerToken = bearerToken == null ? "" : bearerToken;
        return this;
    }

    /**
     * 返回当前 HTTP 超时.
     *
     * @return 超时毫秒数
     */
    public int getTimeoutMillis() {
        return timeoutMillis;
    }

    /**
     * 设置 HTTP 超时.
     *
     * @param timeoutMillis 超时毫秒数
     * @return 当前对象, 便于链式调用
     */
    public Client setTimeoutMillis(int timeoutMillis) {
        this.timeoutMillis = timeoutMillis;
        return this;
    }

    /**
     * 设置重连退避区间.
     *
     * @param minMillis 最小退避毫秒数
     * @param maxMillis 最大退避毫秒数
     * @return 当前对象, 便于链式调用
     */
    public Client setReconnectBackoffMillis(long minMillis, long maxMillis) {
        this.reconnectBackoffMinMillis = minMillis;
        this.reconnectBackoffMaxMillis = maxMillis;
        return this;
    }

    /**
     * 返回默认地址选择策略副本.
     *
     * @return 独立副本
     */
    public AddressSelection getDefaultAddressSelection() {
        return defaultAddressSelection.copy();
    }

    /**
     * 设置默认地址选择策略.
     *
     * @param selection 新策略
     * @return 当前对象, 便于链式调用
     */
    public Client setDefaultAddressSelection(AddressSelection selection) {
        this.defaultAddressSelection = selection == null ? AddressSelection.defaults() : selection.copy();
        return this;
    }

    /**
     * 设置默认自动选址是否包含 loopback.
     *
     * @param includeLoopback 为 true 时允许 loopback
     * @return 当前对象, 便于链式调用
     */
    public Client setIncludeLoopback(boolean includeLoopback) {
        defaultAddressSelection.withLoopback(includeLoopback);
        return this;
    }

    /**
     * 设置默认自动选址是否包含私网 IPv4.
     *
     * @param includePrivateIpv4 为 true 时允许私网 IPv4
     * @return 当前对象, 便于链式调用
     */
    public Client setIncludePrivateIpv4(boolean includePrivateIpv4) {
        defaultAddressSelection.withPrivateIpv4(includePrivateIpv4);
        return this;
    }

    /**
     * 设置默认自动选址是否包含 link local IPv4.
     *
     * @param includeLinkLocalIpv4 为 true 时允许 `169.254.0.0/16`
     * @return 当前对象, 便于链式调用
     */
    public Client setIncludeLinkLocalIpv4(boolean includeLinkLocalIpv4) {
        defaultAddressSelection.withLinkLocalIpv4(includeLinkLocalIpv4);
        return this;
    }

    /**
     * 设置默认自动选址是否包含 IPv6.
     *
     * @param includeIpv6 为 true 时允许 IPv6
     * @return 当前对象, 便于链式调用
     */
    public Client setIncludeIpv6(boolean includeIpv6) {
        defaultAddressSelection.withIpv6(includeIpv6);
        return this;
    }

    /**
     * 追加一个默认接口白名单.
     *
     * @param interfaceName 接口名
     * @return 当前对象, 便于链式调用
     */
    public Client enableInterface(String interfaceName) {
        defaultAddressSelection.enableInterface(interfaceName);
        return this;
    }

    /**
     * 追加一个默认接口黑名单.
     *
     * @param interfaceName 接口名
     * @return 当前对象, 便于链式调用
     */
    public Client disableInterface(String interfaceName) {
        defaultAddressSelection.disableInterface(interfaceName);
        return this;
    }

    /**
     * 清空默认接口白名单和黑名单.
     *
     * @return 当前对象, 便于链式调用
     */
    public Client clearInterfaceFilters() {
        defaultAddressSelection.clearInterfaceFilters();
        return this;
    }

    /**
     * 执行一次性 discover.
     *
     * @param filter 发现过滤器
     * @return 当前匹配节点快照
     * @throws LndException 当请求失败, 服务端返回错误, 或响应 JSON 非法时抛出
     */
    public List<DiscoveredNode> discover(DiscoveryFilter filter) throws LndException {
        DiscoverResponse response = discoverResponse(filter);
        return response.nodes;
    }

    /**
     * 执行一次带自动可达域补充的 discover.
     *
     * <p>注意事项:
     *
     * <ul>
     *   <li>如果传入过滤器已经包含 `reachability_scopes`, 仍会额外合并当前 client 推导出的 scopes
     *   <li>适合零配置或多网卡场景
     * </ul>
     *
     * @param filter 发现过滤器
     * @return 当前匹配节点快照
     * @throws LndException 当自动 scope 推导或网络请求失败时抛出
     */
    public List<DiscoveredNode> discoverWithAutoScopeOverlap(DiscoveryFilter filter) throws LndException {
        return discover(withAutoScopes(filter));
    }

    /**
     * 执行一次性 announce.
     *
     * <p>功能简介:
     *
     * <ul>
     *   <li>解析最终要上报的 LAN 地址和 reachability scopes
     *   <li>调用 `PUT /v1/nodes/{node_id}`
     *   <li>返回服务端标准化后的节点记录
     * </ul>
     *
     * @param spec 注册规格
     * @return 服务端返回的标准节点记录
     * @throws LndException 当地址解析, 请求发送, 或 JSON 解析失败时抛出
     */
    public DiscoveredNode announceOnce(AnnounceSpec spec) throws LndException {
        Map<String, Object> payload = buildAnnouncementPayload(spec);
        String path = "/v1/nodes/" + urlEncodePathSegment(spec.getNodeId());
        String body = request("PUT", path, Collections.<String, List<String>>emptyMap(), Json.stringify(payload), "application/json");
        return parseDiscoveredNode(Json.asObject(Json.parse(body), "announce response"));
    }

    /**
     * 启动后台 announce 续租循环.
     *
     * <p>行为语义:
     *
     * <ul>
     *   <li>先立即尝试一次 announce
     *   <li>成功后按 `ttl / 3` 左右加抖动续租
     *   <li>失败后做指数退避重试
     * </ul>
     *
     * @param spec 注册规格
     * @return 可停止和等待的 handle
     */
    public AnnounceHandle announce(final AnnounceSpec spec) {
        final AnnounceHandle handle = new AnnounceHandle();
        Thread thread = new Thread(new Runnable() {
            @Override
            public void run() {
                try {
                    announceLoop(handle, spec.copy());
                } catch (LndException error) {
                    handle.fail(error);
                } finally {
                    handle.finish();
                }
            }
        }, "lnd-announce");
        thread.setDaemon(true);
        handle.bindThread(thread);
        thread.start();
        return handle;
    }

    /**
     * 启动后台 watch 循环.
     *
     * <p>行为语义:
     *
     * <ul>
     *   <li>自动重连
     *   <li>自动 cursor 恢复
     *   <li>收到 `reset` 时自动拉取快照并补发 `snapshot`
     * </ul>
     *
     * @param filter 发现过滤器
     * @param listener 事件监听器
     * @return 可停止和等待的 handle
     */
    public WatchHandle watch(final DiscoveryFilter filter, final DiscoveryEventListener listener) {
        final WatchHandle handle = new WatchHandle();
        Thread thread = new Thread(new Runnable() {
            @Override
            public void run() {
                try {
                    watchLoop(handle, filter.copy(), listener);
                } catch (LndException error) {
                    handle.fail(error);
                } finally {
                    handle.finish();
                }
            }
        }, "lnd-watch");
        thread.setDaemon(true);
        handle.bindThread(thread);
        thread.start();
        return handle;
    }

    /**
     * 启动一个会自动附加本机可达域的 watch 循环.
     *
     * @param filter 发现过滤器
     * @param listener 事件监听器
     * @return 可停止和等待的 handle
     * @throws LndException 当自动可达域推导失败时抛出
     */
    public WatchHandle watchWithAutoScopeOverlap(DiscoveryFilter filter, DiscoveryEventListener listener) throws LndException {
        return watch(withAutoScopes(filter), listener);
    }

    /**
     * 返回当前 client 视角下可推导的 `network_id`.
     *
     * <p>异常语义:
     *
     * <ul>
     *   <li>没有候选时抛错
     *   <li>多个同等候选时抛错
     *   <li>仅有一个 IPv4 候选时优先选择它
     * </ul>
     *
     * @return 推导出的 `network_id`
     * @throws LndException 当没有唯一候选时抛出
     */
    public String resolveNetworkId() throws LndException {
        return resolveNetworkId(defaultAddressSelection);
    }

    /**
     * 使用给定地址选择策略推导一个 `network_id`.
     *
     * @param selection 地址选择策略
     * @return 推导出的 `network_id`
     * @throws LndException 当没有唯一候选时抛出
     */
    public static String resolveNetworkId(AddressSelection selection) throws LndException {
        List<DerivedNetworkId> candidates = listNetworkIdCandidates(selection);
        if (candidates.isEmpty()) {
            throw new LndException("failed to derive network_id: no eligible local network prefix found");
        }
        if (candidates.size() == 1) {
            return candidates.get(0).getNetworkId();
        }
        List<DerivedNetworkId> ipv4Candidates = new ArrayList<DerivedNetworkId>();
        for (DerivedNetworkId candidate : candidates) {
            if (candidate.getScope().indexOf('.') >= 0) {
                ipv4Candidates.add(candidate);
            }
        }
        if (ipv4Candidates.size() == 1) {
            return ipv4Candidates.get(0).getNetworkId();
        }
        StringBuilder visible = new StringBuilder();
        for (int index = 0; index < candidates.size(); index++) {
            if (index > 0) {
                visible.append(", ");
            }
            DerivedNetworkId candidate = candidates.get(index);
            visible.append(candidate.getNetworkId()).append('(').append(candidate.getScope()).append(')');
        }
        throw new LndException(
            "failed to derive network_id: multiple eligible network prefixes found: "
                + visible
                + "; specify network_id explicitly or narrow interfaces"
        );
    }

    /**
     * 列出当前 client 视角下的所有 `network_id` 候选项.
     *
     * @return 已排序候选列表
     * @throws LndException 当枚举本机接口失败时抛出
     */
    public List<DerivedNetworkId> listNetworkIdCandidates() throws LndException {
        return listNetworkIdCandidates(defaultAddressSelection);
    }

    /**
     * 使用给定地址选择策略列出所有 `network_id` 候选项.
     *
     * @param selection 地址选择策略
     * @return 已排序候选列表
     * @throws LndException 当枚举本机接口失败时抛出
     */
    public static List<DerivedNetworkId> listNetworkIdCandidates(AddressSelection selection) throws LndException {
        List<DerivedNetworkId> candidates = collectDerivedNetworkIds(selection);
        Collections.sort(candidates, new Comparator<DerivedNetworkId>() {
            @Override
            public int compare(DerivedNetworkId left, DerivedNetworkId right) {
                int byScope = left.getScope().compareTo(right.getScope());
                if (byScope != 0) {
                    return byScope;
                }
                return left.getNetworkId().compareTo(right.getNetworkId());
            }
        });
        List<DerivedNetworkId> deduped = new ArrayList<DerivedNetworkId>();
        String last = null;
        for (DerivedNetworkId candidate : candidates) {
            String key = candidate.getNetworkId() + "@" + candidate.getScope();
            if (!key.equals(last)) {
                deduped.add(candidate);
                last = key;
            }
        }
        return Collections.unmodifiableList(deduped);
    }

    /**
     * 列出当前 client 视角下的所有 reachability scopes.
     *
     * @return 已排序的 scope 列表
     * @throws LndException 当枚举本机接口失败时抛出
     */
    public List<String> listReachabilityScopes() throws LndException {
        return listReachabilityScopes(defaultAddressSelection);
    }

    /**
     * 使用给定地址选择策略列出所有 reachability scopes.
     *
     * @param selection 地址选择策略
     * @return 已排序的 scope 列表
     * @throws LndException 当枚举本机接口失败时抛出
     */
    public static List<String> listReachabilityScopes(AddressSelection selection) throws LndException {
        List<DerivedNetworkId> candidates = listNetworkIdCandidates(selection);
        Set<String> scopes = new LinkedHashSet<String>();
        for (DerivedNetworkId candidate : candidates) {
            scopes.add(candidate.getScope());
        }
        List<String> values = new ArrayList<String>(scopes);
        Collections.sort(values);
        return Collections.unmodifiableList(values);
    }

    /**
     * 解析最终 announce 地址列表.
     *
     * <p>行为语义:
     *
     * <ul>
     *   <li>显式 `lan_addrs` 与自动解析地址合并
     *   <li>最后按字符串排序并去重
     * </ul>
     *
     * @param spec 注册规格
     * @return 去重后的最终地址列表
     * @throws LndException 当自动地址解析失败时抛出
     */
    public List<String> resolveAnnounceAddrs(AnnounceSpec spec) throws LndException {
        AddressSelection selection = mergedAddressSelection(spec.getAddressSelection());
        List<String> addrs = new ArrayList<String>(spec.getLanAddrs());
        if (spec.isAutoLanAddrs()) {
            addrs.addAll(resolveLanAddrsWithSelection(spec.getPort(), selection));
        }
        return dedupeSorted(addrs);
    }

    /**
     * 解析最终 reachability scopes 列表.
     *
     * @param spec 注册规格
     * @return 去重后的最终 scope 列表
     * @throws LndException 当自动 scope 推导失败时抛出
     */
    public List<String> resolveReachabilityScopes(AnnounceSpec spec) throws LndException {
        AddressSelection selection = mergedAddressSelection(spec.getAddressSelection());
        List<String> scopes = new ArrayList<String>(spec.getReachabilityScopes());
        if (spec.isAutoReachabilityScopes()) {
            scopes.addAll(listReachabilityScopes(selection));
        }
        return dedupeSorted(scopes);
    }

    private DiscoverResponse discoverResponse(DiscoveryFilter filter) throws LndException {
        String body = request("GET", "/v1/nodes", queryMap(filter, null), null, null);
        Map<String, Object> object = Json.asObject(Json.parse(body), "discover response");
        List<Object> nodesValue = Json.asArray(object.get("nodes"), "discover response nodes");
        List<DiscoveredNode> nodes = new ArrayList<DiscoveredNode>(nodesValue.size());
        for (Object item : nodesValue) {
            nodes.add(parseDiscoveredNode(Json.asObject(item, "discovered node")));
        }
        long cursor = Json.requireLong(object, "cursor");
        return new DiscoverResponse(Collections.unmodifiableList(nodes), cursor);
    }

    private void announceLoop(AnnounceHandle handle, AnnounceSpec spec) throws LndException {
        int attempt = 0;
        while (!handle.isStopRequested()) {
            try {
                if (attempt > 0) {
                    sleepWithStop(handle, backoffDelayMillis(attempt));
                }
                announceOnce(spec);
                attempt = 0;
                long renewInterval = Math.max(1L, spec.getTtlSecs() / 3L) * 1_000L;
                sleepWithStop(handle, withJitterMillis(renewInterval));
            } catch (LndException error) {
                if (handle.isStopRequested()) {
                    return;
                }
                attempt++;
                if (attempt > 1000) {
                    throw error;
                }
            }
        }
    }

    private void watchLoop(WatchHandle handle, DiscoveryFilter filter, DiscoveryEventListener listener) throws LndException {
        Long cursor = null;
        int attempt = 0;
        while (!handle.isStopRequested()) {
            try {
                cursor = watchOnce(handle, filter, listener, cursor);
                attempt = 0;
            } catch (LndException error) {
                if (handle.isStopRequested()) {
                    return;
                }
                attempt++;
                if (attempt > 1000) {
                    throw error;
                }
                sleepWithStop(handle, backoffDelayMillis(attempt));
            }
        }
    }

    private Long watchOnce(
        WatchHandle handle,
        DiscoveryFilter filter,
        DiscoveryEventListener listener,
        Long cursor
    ) throws LndException {
        HttpURLConnection connection = null;
        try {
            connection = openConnection("GET", "/v1/watch", queryMap(filter, cursor), null, "text/event-stream");
            int status = connection.getResponseCode();
            if (status == 409) {
                listener.onEvent(new DiscoveryEventEnvelope(cursor, DiscoveryEvent.reset()));
                DiscoverResponse snapshot = discoverResponse(filter);
                listener.onEvent(new DiscoveryEventEnvelope(snapshot.cursor, DiscoveryEvent.snapshot(snapshot.nodes)));
                return snapshot.cursor;
            }
            if (status < 200 || status >= 300) {
                throw apiError(connection);
            }
            BufferedReader reader = new BufferedReader(new InputStreamReader(connection.getInputStream(), StandardCharsets.UTF_8));
            Long latestCursor = cursor;
            while (!handle.isStopRequested()) {
                String payload = readSsePayload(reader);
                if (payload == null) {
                    throw new LndException("watch stream closed");
                }
                if (payload.length() == 0) {
                    continue;
                }
                DiscoveryEventEnvelope envelope = parseDiscoveryEventEnvelope(payload);
                if (envelope.getCursor() != null) {
                    latestCursor = envelope.getCursor();
                }
                listener.onEvent(envelope);
                if (envelope.getEvent().getType() == DiscoveryEvent.Type.RESET) {
                    DiscoverResponse snapshot = discoverResponse(filter);
                    latestCursor = snapshot.cursor;
                    listener.onEvent(new DiscoveryEventEnvelope(snapshot.cursor, DiscoveryEvent.snapshot(snapshot.nodes)));
                }
            }
            return latestCursor;
        } catch (IOException error) {
            if (handle.isStopRequested()) {
                return cursor;
            }
            throw new LndException("watch failed", error);
        } finally {
            if (connection != null) {
                connection.disconnect();
            }
        }
    }

    private DiscoveryFilter withAutoScopes(DiscoveryFilter filter) throws LndException {
        DiscoveryFilter copy = filter.copy();
        List<String> scopes = listReachabilityScopes();
        for (String scope : scopes) {
            copy.addReachabilityScope(scope);
        }
        return copy;
    }

    private AddressSelection mergedAddressSelection(AddressSelection overrideSelection) {
        if (overrideSelection == null) {
            return defaultAddressSelection.copy();
        }
        AddressSelection merged = defaultAddressSelection.copy();
        merged
            .withPrivateIpv4(overrideSelection.isIncludePrivateIpv4())
            .withLoopback(overrideSelection.isIncludeLoopback())
            .withLinkLocalIpv4(overrideSelection.isIncludeLinkLocalIpv4())
            .withIpv6(overrideSelection.isIncludeIpv6());
        if (!overrideSelection.getInterfaceAllowlist().isEmpty()) {
            merged.clearInterfaceFilters();
            for (String name : overrideSelection.getInterfaceAllowlist()) {
                merged.enableInterface(name);
            }
            for (String name : overrideSelection.getInterfaceDenylist()) {
                merged.disableInterface(name);
            }
            return merged;
        }
        for (String name : overrideSelection.getInterfaceDenylist()) {
            merged.disableInterface(name);
        }
        return merged;
    }

    private Map<String, Object> buildAnnouncementPayload(AnnounceSpec spec) throws LndException {
        Map<String, Object> payload = new LinkedHashMap<String, Object>();
        if (spec.getNetworkId() != null && spec.getNetworkId().length() > 0) {
            payload.put("network_id", spec.getNetworkId());
        }
        payload.put("node_id", spec.getNodeId());
        payload.put("service", spec.getService());
        payload.put("display_name", spec.getDisplayName());
        payload.put("port", Integer.valueOf(spec.getPort()));
        payload.put("lan_addrs", resolveAnnounceAddrs(spec));
        payload.put("reachability_scopes", resolveReachabilityScopes(spec));
        if (!spec.getTags().isEmpty()) {
            payload.put("tags", dedupeSorted(new ArrayList<String>(spec.getTags())));
        }
        if (!spec.getMetadata().isEmpty()) {
            payload.put("metadata", new LinkedHashMap<String, String>(spec.getMetadata()));
        }
        payload.put("ttl_secs", Long.valueOf(spec.getTtlSecs()));
        return payload;
    }

    private HttpURLConnection openConnection(
        String method,
        String path,
        Map<String, List<String>> query,
        String requestBody,
        String accept
    ) throws LndException {
        try {
            URL url = URI.create(buildUrl(path, query)).toURL();
            HttpURLConnection connection = (HttpURLConnection) url.openConnection();
            connection.setRequestMethod(method);
            connection.setConnectTimeout(timeoutMillis);
            if ("text/event-stream".equals(accept)) {
                connection.setReadTimeout(Math.max(timeoutMillis, 45_000));
            } else {
                connection.setReadTimeout(timeoutMillis);
            }
            connection.setUseCaches(false);
            if (accept != null) {
                connection.setRequestProperty("Accept", accept);
            }
            if (bearerToken != null && bearerToken.length() > 0) {
                connection.setRequestProperty("Authorization", "Bearer " + bearerToken);
            }
            if (requestBody != null) {
                connection.setDoOutput(true);
                connection.setRequestProperty("Content-Type", "application/json");
                byte[] bytes = requestBody.getBytes(StandardCharsets.UTF_8);
                connection.setFixedLengthStreamingMode(bytes.length);
                OutputStream output = connection.getOutputStream();
                try {
                    output.write(bytes);
                } finally {
                    output.close();
                }
            }
            return connection;
        } catch (IOException error) {
            throw new LndException("failed to open connection", error);
        }
    }

    private String request(
        String method,
        String path,
        Map<String, List<String>> query,
        String requestBody,
        String accept
    ) throws LndException {
        HttpURLConnection connection = null;
        try {
            connection = openConnection(method, path, query, requestBody, accept);
            int status = connection.getResponseCode();
            String body = readResponseBody(connection, status);
            if (status < 200 || status >= 300) {
                throw parseApiError(status, body);
            }
            return body;
        } catch (IOException error) {
            throw new LndException("http request failed", error);
        } finally {
            if (connection != null) {
                connection.disconnect();
            }
        }
    }

    private String buildUrl(String path, Map<String, List<String>> query) {
        StringBuilder builder = new StringBuilder();
        builder.append(baseUrl).append(path);
        if (!query.isEmpty()) {
            builder.append('?');
            boolean first = true;
            for (Map.Entry<String, List<String>> entry : query.entrySet()) {
                for (String value : entry.getValue()) {
                    if (!first) {
                        builder.append('&');
                    }
                    first = false;
                    builder.append(urlEncodeQueryComponent(entry.getKey())).append('=').append(urlEncodeQueryComponent(value));
                }
            }
        }
        return builder.toString();
    }

    private Map<String, List<String>> queryMap(DiscoveryFilter filter, Long cursor) {
        Map<String, List<String>> values = new LinkedHashMap<String, List<String>>();
        if (filter.getNetworkId() != null && filter.getNetworkId().length() > 0) {
            values.put("network_id", Collections.singletonList(filter.getNetworkId()));
        }
        if (filter.getService() != null && filter.getService().length() > 0) {
            values.put("service", Collections.singletonList(filter.getService()));
        }
        if (!filter.getTags().isEmpty()) {
            values.put("tag", new ArrayList<String>(filter.getTags()));
        }
        if (!filter.getReachabilityScopes().isEmpty()) {
            values.put("scope", new ArrayList<String>(filter.getReachabilityScopes()));
        }
        if (cursor != null) {
            values.put("cursor", Collections.singletonList(String.valueOf(cursor.longValue())));
        }
        return values;
    }

    private static String readResponseBody(HttpURLConnection connection, int status) throws IOException {
        InputStream stream = status >= 200 && status < 300 ? connection.getInputStream() : connection.getErrorStream();
        if (stream == null) {
            return "";
        }
        try {
            ByteArrayOutputStream buffer = new ByteArrayOutputStream();
            byte[] chunk = new byte[4096];
            int read;
            while ((read = stream.read(chunk)) >= 0) {
                buffer.write(chunk, 0, read);
            }
            return new String(buffer.toByteArray(), StandardCharsets.UTF_8);
        } finally {
            stream.close();
        }
    }

    private static String readSsePayload(BufferedReader reader) throws IOException {
        StringBuilder payload = new StringBuilder();
        while (true) {
            String line = reader.readLine();
            if (line == null) {
                return payload.length() == 0 ? null : payload.toString();
            }
            if (line.length() == 0) {
                return payload.toString();
            }
            if (line.startsWith(":")) {
                continue;
            }
            if (line.startsWith("data:")) {
                String value = line.substring(5).trim();
                if (payload.length() > 0) {
                    payload.append('\n');
                }
                payload.append(value);
            }
        }
    }

    private static LndException apiError(HttpURLConnection connection) throws LndException {
        try {
            int status = connection.getResponseCode();
            String body = readResponseBody(connection, status);
            return parseApiError(status, body);
        } catch (IOException error) {
            return new LndException("http request failed", error);
        }
    }

    private static LndException parseApiError(int status, String body) throws LndException {
        if (body != null && body.length() > 0) {
            try {
                Map<String, Object> object = Json.asObject(Json.parse(body), "api error");
                String error = Json.optString(object, "error");
                if (error != null && error.length() > 0) {
                    return new LndException(error);
                }
            } catch (LndException ignored) {
                // ignore parse error and fall through
            }
        }
        return new LndException("http " + status + ": " + body);
    }

    private static DiscoveryEventEnvelope parseDiscoveryEventEnvelope(String json) throws LndException {
        Map<String, Object> object = Json.asObject(Json.parse(json), "watch envelope");
        Long cursor = Json.optLong(object, "cursor");
        Map<String, Object> eventObject = Json.asObject(object.get("event"), "watch event");
        return new DiscoveryEventEnvelope(cursor, parseDiscoveryEvent(eventObject));
    }

    private static DiscoveryEvent parseDiscoveryEvent(Map<String, Object> object) throws LndException {
        String typeName = Json.requireString(object, "type");
        DiscoveryEvent.Type type = DiscoveryEvent.Type.fromWireName(typeName);
        switch (type) {
            case SNAPSHOT:
                List<Object> rawNodes = Json.asArray(object.get("nodes"), "snapshot nodes");
                List<DiscoveredNode> nodes = new ArrayList<DiscoveredNode>(rawNodes.size());
                for (Object item : rawNodes) {
                    nodes.add(parseDiscoveredNode(Json.asObject(item, "snapshot node")));
                }
                return DiscoveryEvent.snapshot(nodes);
            case UPSERT:
                return DiscoveryEvent.upsert(parseDiscoveredNode(Json.asObject(object.get("node"), "upsert node")));
            case REMOVE:
                return DiscoveryEvent.remove(parseDiscoveredNode(Json.asObject(object.get("node"), "remove node")));
            case RESET:
                return DiscoveryEvent.reset();
            case KEEPALIVE:
                return DiscoveryEvent.keepalive();
            default:
                throw new LndException("unsupported discovery event type: " + typeName);
        }
    }

    private static DiscoveredNode parseDiscoveredNode(Map<String, Object> object) throws LndException {
        LeaseInfo lease = parseLeaseInfo(Json.asObject(object.get("lease"), "lease"));
        return new DiscoveredNode(
            Json.optString(object, "network_id"),
            Json.requireString(object, "node_id"),
            Json.requireString(object, "service"),
            Json.requireString(object, "display_name"),
            (int) Json.requireLong(object, "port"),
            Json.optStringList(object, "lan_addrs"),
            Json.optStringList(object, "reachability_scopes"),
            Json.optStringList(object, "tags"),
            Json.optStringMap(object, "metadata"),
            lease
        );
    }

    private static LeaseInfo parseLeaseInfo(Map<String, Object> object) throws LndException {
        return new LeaseInfo(
            Json.requireLong(object, "revision"),
            Json.requireLong(object, "ttl_secs"),
            Json.requireLong(object, "expires_at_unix_ms"),
            Json.requireLong(object, "last_seen_unix_ms")
        );
    }

    private static List<String> resolveLanAddrsWithSelection(int port, AddressSelection selection) throws LndException {
        try {
            List<String> addrs = new ArrayList<String>();
            NetworkInterface[] interfaces = Collections.list(NetworkInterface.getNetworkInterfaces()).toArray(new NetworkInterface[0]);
            for (NetworkInterface networkInterface : interfaces) {
                if (!selection.allowsInterface(networkInterface.getName())) {
                    continue;
                }
                boolean isLoopback = networkInterface.isLoopback();
                for (InterfaceAddress interfaceAddress : networkInterface.getInterfaceAddresses()) {
                    InetAddress address = interfaceAddress.getAddress();
                    if (address == null) {
                        continue;
                    }
                    if (!allowsAddress(selection, address, isLoopback)) {
                        continue;
                    }
                    addrs.add(formatHostPort(normalizeHost(address), port));
                }
            }
            return dedupeSorted(addrs);
        } catch (IOException error) {
            throw new LndException("failed to enumerate local interfaces", error);
        }
    }

    private static List<DerivedNetworkId> collectDerivedNetworkIds(AddressSelection selection) throws LndException {
        try {
            List<DerivedNetworkId> candidates = new ArrayList<DerivedNetworkId>();
            NetworkInterface[] interfaces = Collections.list(NetworkInterface.getNetworkInterfaces()).toArray(new NetworkInterface[0]);
            for (NetworkInterface networkInterface : interfaces) {
                if (!selection.allowsInterface(networkInterface.getName())) {
                    continue;
                }
                boolean isLoopback = networkInterface.isLoopback();
                for (InterfaceAddress interfaceAddress : networkInterface.getInterfaceAddresses()) {
                    InetAddress address = interfaceAddress.getAddress();
                    if (address == null || !allowsAddress(selection, address, isLoopback)) {
                        continue;
                    }
                    short prefixLength = interfaceAddress.getNetworkPrefixLength();
                    if (address instanceof Inet4Address) {
                        String scope = ipv4Scope((Inet4Address) address, prefixLength);
                        candidates.add(new DerivedNetworkId("lan-" + shortStableHex("v4:" + scope), scope));
                    } else if (address instanceof Inet6Address) {
                        String scope = ipv6Scope((Inet6Address) address, prefixLength);
                        candidates.add(new DerivedNetworkId("lan-" + shortStableHex("v6:" + scope), scope));
                    }
                }
            }
            return candidates;
        } catch (IOException error) {
            throw new LndException("failed to enumerate local interfaces", error);
        }
    }

    private static boolean allowsAddress(AddressSelection selection, InetAddress address, boolean isLoopbackInterface) {
        if (address.isLoopbackAddress() || isLoopbackInterface) {
            return selection.isIncludeLoopback();
        }
        if (address instanceof Inet4Address) {
            byte[] bytes = address.getAddress();
            int b0 = bytes[0] & 0xff;
            int b1 = bytes[1] & 0xff;
            boolean privateIpv4 = b0 == 10
                || (b0 == 172 && b1 >= 16 && b1 <= 31)
                || (b0 == 192 && b1 == 168);
            boolean linkLocalIpv4 = b0 == 169 && b1 == 254;
            return privateIpv4 && selection.isIncludePrivateIpv4()
                || linkLocalIpv4 && selection.isIncludeLinkLocalIpv4();
        }
        return selection.isIncludeIpv6() && !address.isAnyLocalAddress();
    }

    private static String ipv4Scope(Inet4Address address, short prefixLength) {
        int prefix = clampPrefix(prefixLength, 32);
        int ip = toInt(address.getAddress());
        int mask = prefix == 0 ? 0 : (-1 << (32 - prefix));
        int network = ip & mask;
        byte[] bytes = new byte[] {
            (byte) ((network >>> 24) & 0xff),
            (byte) ((network >>> 16) & 0xff),
            (byte) ((network >>> 8) & 0xff),
            (byte) (network & 0xff)
        };
        try {
            return InetAddress.getByAddress(bytes).getHostAddress() + "/" + prefix;
        } catch (IOException error) {
            throw new IllegalStateException(error);
        }
    }

    private static String ipv6Scope(Inet6Address address, short prefixLength) {
        int prefix = clampPrefix(prefixLength, 128);
        byte[] bytes = Arrays.copyOf(address.getAddress(), 16);
        int fullBytes = prefix / 8;
        int remBits = prefix % 8;
        if (fullBytes < bytes.length) {
            if (remBits != 0) {
                int mask = 0xff << (8 - remBits);
                bytes[fullBytes] = (byte) (bytes[fullBytes] & mask);
                fullBytes++;
            }
            for (int index = fullBytes; index < bytes.length; index++) {
                bytes[index] = 0;
            }
        }
        try {
            return stripScopeId(InetAddress.getByAddress(bytes).getHostAddress()) + "/" + prefix;
        } catch (IOException error) {
            throw new IllegalStateException(error);
        }
    }

    private static int clampPrefix(short prefixLength, int max) {
        if (prefixLength < 0) {
            return 0;
        }
        if (prefixLength > max) {
            return max;
        }
        return prefixLength;
    }

    private static int toInt(byte[] bytes) {
        return ((bytes[0] & 0xff) << 24)
            | ((bytes[1] & 0xff) << 16)
            | ((bytes[2] & 0xff) << 8)
            | (bytes[3] & 0xff);
    }

    private static String normalizeHost(InetAddress address) {
        return stripScopeId(address.getHostAddress());
    }

    private static String stripScopeId(String host) {
        int marker = host.indexOf('%');
        if (marker >= 0) {
            return host.substring(0, marker);
        }
        return host;
    }

    private static String formatHostPort(String host, int port) {
        if (host.indexOf(':') >= 0) {
            return "[" + host + "]:" + port;
        }
        return host + ":" + port;
    }

    private long backoffDelayMillis(int attempt) {
        long base = reconnectBackoffMinMillis <= 0 ? DEFAULT_RECONNECT_BACKOFF_MIN_MILLIS : reconnectBackoffMinMillis;
        long max = reconnectBackoffMaxMillis < base ? base : reconnectBackoffMaxMillis;
        int power = Math.min(attempt, 10);
        long delay = base;
        for (int index = 0; index < power; index++) {
            if (delay >= max) {
                delay = max;
                break;
            }
            delay = Math.min(max, delay * 2L);
        }
        long jitter = (long) (Math.random() * (double) Math.max(1L, base + 1L));
        return Math.min(max, delay + jitter);
    }

    private long withJitterMillis(long durationMillis) {
        if (durationMillis <= 0L) {
            return 1_000L;
        }
        long jitter = (long) (Math.random() * (double) Math.max(1L, durationMillis / 5L));
        return durationMillis + jitter;
    }

    private static List<String> dedupeSorted(List<String> values) {
        Set<String> deduped = new LinkedHashSet<String>(values);
        List<String> sorted = new ArrayList<String>(deduped);
        Collections.sort(sorted);
        return Collections.unmodifiableList(sorted);
    }

    private static String shortStableHex(String value) {
        long hash = 0xcbf29ce484222325L;
        long prime = 0x100000001b3L;
        byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
        for (byte b : bytes) {
            hash ^= (long) (b & 0xff);
            hash *= prime;
        }
        return String.format("%016x", hash);
    }

    private static String trimTrailingSlash(String value) {
        if (value == null || value.length() == 0) {
            return DEFAULT_BASE_URL;
        }
        int end = value.length();
        while (end > 0 && value.charAt(end - 1) == '/') {
            end--;
        }
        return end == 0 ? DEFAULT_BASE_URL : value.substring(0, end);
    }

    private static String urlEncodeQueryComponent(String value) {
        byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
        StringBuilder builder = new StringBuilder(bytes.length * 2);
        for (byte b : bytes) {
            int ch = b & 0xff;
            if (isUnreserved(ch)) {
                builder.append((char) ch);
            } else {
                builder.append('%');
                builder.append(Character.toUpperCase(Character.forDigit((ch >>> 4) & 0x0f, 16)));
                builder.append(Character.toUpperCase(Character.forDigit(ch & 0x0f, 16)));
            }
        }
        return builder.toString();
    }

    private static String urlEncodePathSegment(String value) {
        return urlEncodeQueryComponent(value).replace("+", "%20");
    }

    private static boolean isUnreserved(int ch) {
        return ch >= 'A' && ch <= 'Z'
            || ch >= 'a' && ch <= 'z'
            || ch >= '0' && ch <= '9'
            || ch == '-'
            || ch == '.'
            || ch == '_'
            || ch == '~';
    }

    private static void sleepWithStop(AnnounceHandle handle, long millis) throws LndException {
        try {
            Thread.sleep(Math.max(1L, millis));
        } catch (InterruptedException error) {
            Thread.currentThread().interrupt();
            if (!handle.isStopRequested()) {
                throw new LndException("announce loop interrupted", error);
            }
        }
    }

    private static void sleepWithStop(WatchHandle handle, long millis) throws LndException {
        try {
            Thread.sleep(Math.max(1L, millis));
        } catch (InterruptedException error) {
            Thread.currentThread().interrupt();
            if (!handle.isStopRequested()) {
                throw new LndException("watch loop interrupted", error);
            }
        }
    }

    private static final class DiscoverResponse {
        private final List<DiscoveredNode> nodes;
        private final Long cursor;

        private DiscoverResponse(List<DiscoveredNode> nodes, long cursor) {
            this.nodes = nodes;
            this.cursor = Long.valueOf(cursor);
        }
    }
}
