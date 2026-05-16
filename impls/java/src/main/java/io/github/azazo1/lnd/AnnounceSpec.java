package io.github.azazo1.lnd;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * 节点注册规格.
 *
 * <p>功能简介:
 *
 * <ul>
 *   <li>描述一次注册或续租时客户端需要提供的参数
 *   <li>既可携带显式 `lan_addrs`, 也可启用自动地址收集
 *   <li>既可携带显式 `reachability_scopes`, 也可启用自动 scope 推导
 * </ul>
 *
 * <p>默认值:
 *
 * <ul>
 *   <li>`autoLanAddrs = true`
 *   <li>`autoReachabilityScopes = true`
 *   <li>`ttlSecs = 30`
 * </ul>
 *
 * <p>使用示例:
 *
 * <pre>{@code
 * AnnounceSpec spec = new AnnounceSpec("node-a", "_http._tcp", "devbox-a", 8080)
 *     .withNetworkId("office-a")
 *     .addTag("stable")
 *     .insertMetadata("role", "api");
 * }</pre>
 */
public final class AnnounceSpec {
    /** 默认 TTL, 单位为秒. */
    public static final long DEFAULT_TTL_SECONDS = 30L;

    private String networkId;
    private final String nodeId;
    private String service;
    private String displayName;
    private int port;
    private final List<String> lanAddrs = new ArrayList<String>();
    private boolean autoLanAddrs = true;
    private AddressSelection addressSelection;
    private final List<String> reachabilityScopes = new ArrayList<String>();
    private boolean autoReachabilityScopes = true;
    private final List<String> tags = new ArrayList<String>();
    private final Map<String, String> metadata = new LinkedHashMap<String, String>();
    private long ttlSecs = DEFAULT_TTL_SECONDS;

    /**
     * 创建一个最小可用的注册规格.
     *
     * @param nodeId 节点持久标识, 调用方应自行保证跨重启稳定
     * @param service 服务名, 例如 `_http._tcp`, 建议使用 mDNS / DNS-SD 常见的 service type
     * @param displayName 面向人类的显示名
     * @param port 服务端口
     */
    public AnnounceSpec(String nodeId, String service, String displayName, int port) {
        this.nodeId = nodeId;
        this.service = service;
        this.displayName = displayName;
        this.port = port;
    }

    /**
     * 设置逻辑发现域.
     *
     * @param networkId 逻辑发现域, 例如 `office-a`
     * @return 当前对象, 便于链式调用
     */
    public AnnounceSpec withNetworkId(String networkId) {
        this.networkId = networkId;
        return this;
    }

    /**
     * 清空逻辑发现域.
     *
     * @return 当前对象, 便于链式调用
     */
    public AnnounceSpec withoutNetworkId() {
        this.networkId = null;
        return this;
    }

    /**
     * 设置服务名.
     *
     * @param service 服务名
     * @return 当前对象, 便于链式调用
     */
    public AnnounceSpec withService(String service) {
        this.service = service;
        return this;
    }

    /**
     * 设置显示名.
     *
     * @param displayName 显示名
     * @return 当前对象, 便于链式调用
     */
    public AnnounceSpec withDisplayName(String displayName) {
        this.displayName = displayName;
        return this;
    }

    /**
     * 设置服务端口.
     *
     * @param port 服务端口
     * @return 当前对象, 便于链式调用
     */
    public AnnounceSpec withPort(int port) {
        this.port = port;
        return this;
    }

    /**
     * 追加一个显式 LAN 地址.
     *
     * <p>注意事项:
     *
     * <ul>
     *   <li>地址格式应为 `host:port`
     *   <li>如果同时开启 `autoLanAddrs`, 会与自动地址合并后去重
     * </ul>
     *
     * @param lanAddr 显式地址
     * @return 当前对象, 便于链式调用
     */
    public AnnounceSpec addLanAddr(String lanAddr) {
        lanAddrs.add(lanAddr);
        return this;
    }

    /**
     * 使用给定显式地址覆盖当前 LAN 地址列表.
     *
     * @param lanAddrs 地址列表
     * @return 当前对象, 便于链式调用
     */
    public AnnounceSpec withLanAddrs(List<String> lanAddrs) {
        this.lanAddrs.clear();
        this.lanAddrs.addAll(lanAddrs);
        return this;
    }

    /**
     * 设置是否自动收集 LAN 地址.
     *
     * @param autoLanAddrs 为 true 时自动从本机接口收集地址
     * @return 当前对象, 便于链式调用
     */
    public AnnounceSpec withAutoLanAddrs(boolean autoLanAddrs) {
        this.autoLanAddrs = autoLanAddrs;
        return this;
    }

    /**
     * 覆盖地址选择规则.
     *
     * @param addressSelection 单条 announce 的地址选择规则
     * @return 当前对象, 便于链式调用
     */
    public AnnounceSpec withAddressSelection(AddressSelection addressSelection) {
        this.addressSelection = addressSelection == null ? null : addressSelection.copy();
        return this;
    }

    /**
     * 追加一个显式可达域.
     *
     * @param scope 可达域, 例如 `192.168.1.0/24`
     * @return 当前对象, 便于链式调用
     */
    public AnnounceSpec addReachabilityScope(String scope) {
        reachabilityScopes.add(scope);
        return this;
    }

    /**
     * 使用给定 scopes 覆盖当前可达域列表.
     *
     * @param scopes scope 列表
     * @return 当前对象, 便于链式调用
     */
    public AnnounceSpec withReachabilityScopes(List<String> scopes) {
        reachabilityScopes.clear();
        reachabilityScopes.addAll(scopes);
        return this;
    }

    /**
     * 设置是否自动推导可达域.
     *
     * @param autoReachabilityScopes 为 true 时自动根据本机子网前缀补充 scope
     * @return 当前对象, 便于链式调用
     */
    public AnnounceSpec withAutoReachabilityScopes(boolean autoReachabilityScopes) {
        this.autoReachabilityScopes = autoReachabilityScopes;
        return this;
    }

    /**
     * 追加一个 tag.
     *
     * @param tag 节点 tag
     * @return 当前对象, 便于链式调用
     */
    public AnnounceSpec addTag(String tag) {
        tags.add(tag);
        return this;
    }

    /**
     * 使用给定 tags 覆盖当前 tag 列表.
     *
     * @param tags tag 列表
     * @return 当前对象, 便于链式调用
     */
    public AnnounceSpec withTags(List<String> tags) {
        this.tags.clear();
        this.tags.addAll(tags);
        return this;
    }

    /**
     * 插入一个 metadata 键值对.
     *
     * @param key metadata key
     * @param value metadata value
     * @return 当前对象, 便于链式调用
     */
    public AnnounceSpec insertMetadata(String key, String value) {
        metadata.put(key, value);
        return this;
    }

    /**
     * 使用给定 metadata 覆盖当前 metadata.
     *
     * @param metadata metadata 映射
     * @return 当前对象, 便于链式调用
     */
    public AnnounceSpec withMetadata(Map<String, String> metadata) {
        this.metadata.clear();
        this.metadata.putAll(metadata);
        return this;
    }

    /**
     * 设置 TTL.
     *
     * @param ttlSecs TTL, 单位为秒
     * @return 当前对象, 便于链式调用
     */
    public AnnounceSpec withTtlSecs(long ttlSecs) {
        this.ttlSecs = ttlSecs;
        return this;
    }

    /**
     * 获取逻辑发现域.
     *
     * @return `network_id`, 未设置时为 `null`
     */
    public String getNetworkId() {
        return networkId;
    }

    /**
     * 获取 `node_id`.
     *
     * @return 持久节点标识
     */
    public String getNodeId() {
        return nodeId;
    }

    /**
     * 获取服务名.
     *
     * @return 服务名
     */
    public String getService() {
        return service;
    }

    /**
     * 获取显示名.
     *
     * @return 显示名
     */
    public String getDisplayName() {
        return displayName;
    }

    /**
     * 获取服务端口.
     *
     * @return 端口
     */
    public int getPort() {
        return port;
    }

    /**
     * 返回显式 LAN 地址列表.
     *
     * @return 不可变列表
     */
    public List<String> getLanAddrs() {
        return Collections.unmodifiableList(lanAddrs);
    }

    /**
     * 是否启用自动 LAN 地址收集.
     *
     * @return 当前值
     */
    public boolean isAutoLanAddrs() {
        return autoLanAddrs;
    }

    /**
     * 返回单条 announce 的地址选择规则.
     *
     * @return 地址选择规则, 未设置时为 `null`
     */
    public AddressSelection getAddressSelection() {
        return addressSelection == null ? null : addressSelection.copy();
    }

    /**
     * 返回显式可达域列表.
     *
     * @return 不可变列表
     */
    public List<String> getReachabilityScopes() {
        return Collections.unmodifiableList(reachabilityScopes);
    }

    /**
     * 是否启用自动可达域推导.
     *
     * @return 当前值
     */
    public boolean isAutoReachabilityScopes() {
        return autoReachabilityScopes;
    }

    /**
     * 返回 tag 列表.
     *
     * @return 不可变列表
     */
    public List<String> getTags() {
        return Collections.unmodifiableList(tags);
    }

    /**
     * 返回 metadata 映射.
     *
     * @return 不可变映射
     */
    public Map<String, String> getMetadata() {
        return Collections.unmodifiableMap(metadata);
    }

    /**
     * 获取 TTL.
     *
     * @return TTL, 单位为秒
     */
    public long getTtlSecs() {
        return ttlSecs;
    }

    /**
     * 复制当前规格.
     *
     * @return 独立副本
     */
    public AnnounceSpec copy() {
        return new AnnounceSpec(nodeId, service, displayName, port)
            .withNetworkId(networkId)
            .withLanAddrs(lanAddrs)
            .withAutoLanAddrs(autoLanAddrs)
            .withAddressSelection(addressSelection)
            .withReachabilityScopes(reachabilityScopes)
            .withAutoReachabilityScopes(autoReachabilityScopes)
            .withTags(tags)
            .withMetadata(metadata)
            .withTtlSecs(ttlSecs);
    }
}
