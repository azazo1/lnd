package io.github.azazo1.lnd;

/**
 * 自动推导出的 `network_id` 候选项.
 *
 * <p>字段说明:
 *
 * <ul>
 *   <li>`networkId`: 稳定指纹化后的逻辑发现域标识, 形如 `lan-ec3a7b1765ff30c6`
 *   <li>`scope`: 该候选项对应的本机子网前缀, 例如 `192.168.1.0/24`
 * </ul>
 */
public final class DerivedNetworkId {
    private final String networkId;
    private final String scope;

    /**
     * 创建候选项.
     *
     * @param networkId 推导出的逻辑发现域
     * @param scope 对应的本机子网前缀
     */
    public DerivedNetworkId(String networkId, String scope) {
        this.networkId = networkId;
        this.scope = scope;
    }

    /**
     * 返回推导出的逻辑发现域.
     *
     * @return `network_id`
     */
    public String getNetworkId() {
        return networkId;
    }

    /**
     * 返回对应的本机子网前缀.
     *
     * @return 子网前缀
     */
    public String getScope() {
        return scope;
    }

    @Override
    public String toString() {
        return "DerivedNetworkId{"
            + "networkId='" + networkId + '\''
            + ", scope='" + scope + '\''
            + '}';
    }
}
