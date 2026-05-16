package io.github.azazo1.lnd;

import java.util.Collections;
import java.util.List;
import java.util.Map;

/**
 * discover 和 watch 返回的标准节点模型.
 *
 * <p>功能简介:
 *
 * <ul>
 *   <li>对应服务端标准化后的节点记录
 *   <li>包含地址, tags, metadata, reachability scopes 与 lease 信息
 * </ul>
 */
public final class DiscoveredNode {
    private final String discoveryDomain;
    private final String nodeId;
    private final String service;
    private final String displayName;
    private final int port;
    private final List<String> lanAddrs;
    private final List<String> reachabilityScopes;
    private final List<String> tags;
    private final Map<String, String> metadata;
    private final LeaseInfo lease;

    /**
     * 创建 discovered node.
     *
     * @param discoveryDomain 逻辑发现域, 未设置时可为 `null`
     * @param nodeId 节点标识
     * @param service 服务名
     * @param displayName 显示名
     * @param port 端口
     * @param lanAddrs 地址列表
     * @param reachabilityScopes 可达域列表
     * @param tags tag 列表
     * @param metadata metadata 映射
     * @param lease 服务端租约信息
     */
    public DiscoveredNode(
        String discoveryDomain,
        String nodeId,
        String service,
        String displayName,
        int port,
        List<String> lanAddrs,
        List<String> reachabilityScopes,
        List<String> tags,
        Map<String, String> metadata,
        LeaseInfo lease
    ) {
        this.discoveryDomain = discoveryDomain;
        this.nodeId = nodeId;
        this.service = service;
        this.displayName = displayName;
        this.port = port;
        this.lanAddrs = Collections.unmodifiableList(lanAddrs);
        this.reachabilityScopes = Collections.unmodifiableList(reachabilityScopes);
        this.tags = Collections.unmodifiableList(tags);
        this.metadata = Collections.unmodifiableMap(metadata);
        this.lease = lease;
    }

    /**
     * 返回逻辑发现域.
     *
     * @return `discovery_domain`, 未设置时为 `null`
     */
    public String getDiscoveryDomain() {
        return discoveryDomain;
    }

    /**
     * 返回节点标识.
     *
     * @return `node_id`
     */
    public String getNodeId() {
        return nodeId;
    }

    /**
     * 返回服务名.
     *
     * @return 服务名
     */
    public String getService() {
        return service;
    }

    /**
     * 返回显示名.
     *
     * @return 显示名
     */
    public String getDisplayName() {
        return displayName;
    }

    /**
     * 返回端口.
     *
     * @return 服务端口
     */
    public int getPort() {
        return port;
    }

    /**
     * 返回 LAN 地址列表.
     *
     * @return 不可变列表
     */
    public List<String> getLanAddrs() {
        return lanAddrs;
    }

    /**
     * 返回可达域列表.
     *
     * @return 不可变列表
     */
    public List<String> getReachabilityScopes() {
        return reachabilityScopes;
    }

    /**
     * 返回 tag 列表.
     *
     * @return 不可变列表
     */
    public List<String> getTags() {
        return tags;
    }

    /**
     * 返回 metadata 映射.
     *
     * @return 不可变映射
     */
    public Map<String, String> getMetadata() {
        return metadata;
    }

    /**
     * 返回租约信息.
     *
     * @return 租约信息
     */
    public LeaseInfo getLease() {
        return lease;
    }

    @Override
    public String toString() {
        return "DiscoveredNode{"
            + "discoveryDomain='" + discoveryDomain + '\''
            + ", nodeId='" + nodeId + '\''
            + ", service='" + service + '\''
            + ", displayName='" + displayName + '\''
            + ", port=" + port
            + ", lanAddrs=" + lanAddrs
            + ", reachabilityScopes=" + reachabilityScopes
            + ", tags=" + tags
            + ", metadata=" + metadata
            + ", lease=" + lease
            + '}';
    }
}
