package io.github.azazo1.lnd;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * 发现过滤器.
 *
 * <p>功能简介:
 *
 * <ul>
 *   <li>用于一次性 discover 和持续 watch
 *   <li>`network_id` 为可选逻辑发现域
 *   <li>`service` 为单值过滤条件, 推荐使用 mDNS / DNS-SD 风格的 service type
 *   <li>`tags` 为全部满足语义
 *   <li>`reachability_scopes` 为至少一个重叠语义
 * </ul>
 *
 * <p>使用示例:
 *
 * <pre>{@code
 * DiscoveryFilter filter = new DiscoveryFilter()
 *     .withNetworkId("office-a")
 *     .withService("_http._tcp")
 *     .addTag("stable");
 * }</pre>
 */
public final class DiscoveryFilter {
    private String networkId;
    private String service;
    private final List<String> tags = new ArrayList<String>();
    private final List<String> reachabilityScopes = new ArrayList<String>();

    /**
     * 设置逻辑发现域.
     *
     * @param networkId 逻辑发现域, 例如 `office-a`
     * @return 当前对象, 便于链式调用
     */
    public DiscoveryFilter withNetworkId(String networkId) {
        this.networkId = networkId;
        return this;
    }

    /**
     * 清空逻辑发现域.
     *
     * @return 当前对象, 便于链式调用
     */
    public DiscoveryFilter withoutNetworkId() {
        this.networkId = null;
        return this;
    }

    /**
     * 设置服务名过滤条件.
     *
     * @param service 服务名, 例如 `_http._tcp`, 建议使用 mDNS / DNS-SD 常见的 service type
     * @return 当前对象, 便于链式调用
     */
    public DiscoveryFilter withService(String service) {
        this.service = service;
        return this;
    }

    /**
     * 追加一个必须匹配的 tag.
     *
     * @param tag 过滤 tag
     * @return 当前对象, 便于链式调用
     */
    public DiscoveryFilter addTag(String tag) {
        tags.add(tag);
        return this;
    }

    /**
     * 使用给定 tags 覆盖当前 tag 列表.
     *
     * @param tags 新的 tag 集合
     * @return 当前对象, 便于链式调用
     */
    public DiscoveryFilter withTags(List<String> tags) {
        this.tags.clear();
        this.tags.addAll(tags);
        return this;
    }

    /**
     * 追加一个可达域过滤条件.
     *
     * @param scope 可达域, 例如 `192.168.1.0/24`
     * @return 当前对象, 便于链式调用
     */
    public DiscoveryFilter addReachabilityScope(String scope) {
        reachabilityScopes.add(scope);
        return this;
    }

    /**
     * 使用给定 scopes 覆盖当前可达域过滤条件.
     *
     * @param scopes 新的 scope 集合
     * @return 当前对象, 便于链式调用
     */
    public DiscoveryFilter withReachabilityScopes(List<String> scopes) {
        reachabilityScopes.clear();
        reachabilityScopes.addAll(scopes);
        return this;
    }

    /**
     * 返回当前逻辑发现域.
     *
     * @return `network_id`, 如果未设置则为 `null`
     */
    public String getNetworkId() {
        return networkId;
    }

    /**
     * 返回当前服务名过滤条件.
     *
     * @return 服务名, 如果未设置则为 `null`
     */
    public String getService() {
        return service;
    }

    /**
     * 返回 tag 过滤列表.
     *
     * @return 不可变列表
     */
    public List<String> getTags() {
        return Collections.unmodifiableList(tags);
    }

    /**
     * 返回可达域过滤列表.
     *
     * @return 不可变列表
     */
    public List<String> getReachabilityScopes() {
        return Collections.unmodifiableList(reachabilityScopes);
    }

    /**
     * 复制当前过滤器.
     *
     * @return 独立副本
     */
    public DiscoveryFilter copy() {
        return new DiscoveryFilter()
            .withNetworkId(networkId)
            .withService(service)
            .withTags(tags)
            .withReachabilityScopes(reachabilityScopes);
    }
}
