package io.github.azazo1.lnd;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * 自动地址选择规则.
 *
 * <p>功能简介:
 *
 * <ul>
 *   <li>控制自动 LAN 地址收集和自动 reachability scope 推导时允许哪些接口和地址
 *   <li>既可作为 {@link Client} 的默认策略使用, 也可挂在单个 {@link AnnounceSpec} 上覆盖默认值
 * </ul>
 *
 * <p>默认行为:
 *
 * <ul>
 *   <li>只包含私网 IPv4
 *   <li>不包含 loopback
 *   <li>不包含 link local IPv4
 *   <li>不包含 IPv6
 * </ul>
 */
public final class AddressSelection {
    private boolean includePrivateIpv4 = true;
    private boolean includeLoopback = false;
    private boolean includeLinkLocalIpv4 = false;
    private boolean includeIpv6 = false;
    private final List<String> interfaceAllowlist = new ArrayList<String>();
    private final List<String> interfaceDenylist = new ArrayList<String>();

    /**
     * 创建默认地址选择规则.
     *
     * @return 默认规则
     */
    public static AddressSelection defaults() {
        return new AddressSelection();
    }

    /**
     * 是否允许私网 IPv4.
     *
     * @return 当前值
     */
    public boolean isIncludePrivateIpv4() {
        return includePrivateIpv4;
    }

    /**
     * 设置是否允许私网 IPv4.
     *
     * @param includePrivateIpv4 为 true 时允许自动收集私网 IPv4
     * @return 当前对象, 便于链式调用
     */
    public AddressSelection withPrivateIpv4(boolean includePrivateIpv4) {
        this.includePrivateIpv4 = includePrivateIpv4;
        return this;
    }

    /**
     * 是否允许 loopback.
     *
     * @return 当前值
     */
    public boolean isIncludeLoopback() {
        return includeLoopback;
    }

    /**
     * 设置是否允许 loopback.
     *
     * @param includeLoopback 为 true 时允许自动收集 loopback 地址
     * @return 当前对象, 便于链式调用
     */
    public AddressSelection withLoopback(boolean includeLoopback) {
        this.includeLoopback = includeLoopback;
        return this;
    }

    /**
     * 是否允许 link local IPv4.
     *
     * @return 当前值
     */
    public boolean isIncludeLinkLocalIpv4() {
        return includeLinkLocalIpv4;
    }

    /**
     * 设置是否允许 link local IPv4.
     *
     * @param includeLinkLocalIpv4 为 true 时允许自动收集 `169.254.0.0/16`
     * @return 当前对象, 便于链式调用
     */
    public AddressSelection withLinkLocalIpv4(boolean includeLinkLocalIpv4) {
        this.includeLinkLocalIpv4 = includeLinkLocalIpv4;
        return this;
    }

    /**
     * 是否允许 IPv6.
     *
     * @return 当前值
     */
    public boolean isIncludeIpv6() {
        return includeIpv6;
    }

    /**
     * 设置是否允许 IPv6.
     *
     * @param includeIpv6 为 true 时允许自动收集 IPv6 地址
     * @return 当前对象, 便于链式调用
     */
    public AddressSelection withIpv6(boolean includeIpv6) {
        this.includeIpv6 = includeIpv6;
        return this;
    }

    /**
     * 追加一个接口白名单.
     *
     * <p>注意事项:
     *
     * <ul>
     *   <li>当白名单非空时, 只有白名单接口会被考虑
     *   <li>如果同时也出现在黑名单中, 黑名单优先
     * </ul>
     *
     * @param interfaceName 接口名, 例如 `en0`, `eth0`, `wlan0`
     * @return 当前对象, 便于链式调用
     */
    public AddressSelection enableInterface(String interfaceName) {
        interfaceAllowlist.add(interfaceName);
        return this;
    }

    /**
     * 追加一个接口黑名单.
     *
     * @param interfaceName 接口名, 例如 `docker0`, `lo`, `utun0`
     * @return 当前对象, 便于链式调用
     */
    public AddressSelection disableInterface(String interfaceName) {
        interfaceDenylist.add(interfaceName);
        return this;
    }

    /**
     * 清空接口白名单和黑名单.
     *
     * @return 当前对象, 便于链式调用
     */
    public AddressSelection clearInterfaceFilters() {
        interfaceAllowlist.clear();
        interfaceDenylist.clear();
        return this;
    }

    /**
     * 返回接口白名单快照.
     *
     * @return 不可变列表
     */
    public List<String> getInterfaceAllowlist() {
        return Collections.unmodifiableList(interfaceAllowlist);
    }

    /**
     * 返回接口黑名单快照.
     *
     * @return 不可变列表
     */
    public List<String> getInterfaceDenylist() {
        return Collections.unmodifiableList(interfaceDenylist);
    }

    /**
     * 复制当前规则.
     *
     * @return 独立副本
     */
    public AddressSelection copy() {
        AddressSelection copy = new AddressSelection()
            .withPrivateIpv4(includePrivateIpv4)
            .withLoopback(includeLoopback)
            .withLinkLocalIpv4(includeLinkLocalIpv4)
            .withIpv6(includeIpv6);
        copy.interfaceAllowlist.addAll(interfaceAllowlist);
        copy.interfaceDenylist.addAll(interfaceDenylist);
        return copy;
    }

    boolean allowsInterface(String interfaceName) {
        boolean allowed = interfaceAllowlist.isEmpty() || interfaceAllowlist.contains(interfaceName);
        boolean denied = interfaceDenylist.contains(interfaceName);
        return allowed && !denied;
    }
}
