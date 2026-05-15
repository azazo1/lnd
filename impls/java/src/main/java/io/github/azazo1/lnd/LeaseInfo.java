package io.github.azazo1.lnd;

/**
 * 节点租约信息.
 *
 * <p>字段说明:
 *
 * <ul>
 *   <li>`revision`: 服务端递增修订号, 同时也是 watch cursor 的基础
 *   <li>`ttlSecs`: 当前租约 TTL
 *   <li>`expiresAtUnixMs`: 如果不再续租, 服务端将在该时间后摘除节点
 *   <li>`lastSeenUnixMs`: 服务端最近一次接收到续租的时间
 * </ul>
 */
public final class LeaseInfo {
    private final long revision;
    private final long ttlSecs;
    private final long expiresAtUnixMs;
    private final long lastSeenUnixMs;

    /**
     * 创建租约信息.
     *
     * @param revision 修订号
     * @param ttlSecs TTL, 单位为秒
     * @param expiresAtUnixMs 过期时间, Unix epoch milliseconds
     * @param lastSeenUnixMs 最近上报时间, Unix epoch milliseconds
     */
    public LeaseInfo(long revision, long ttlSecs, long expiresAtUnixMs, long lastSeenUnixMs) {
        this.revision = revision;
        this.ttlSecs = ttlSecs;
        this.expiresAtUnixMs = expiresAtUnixMs;
        this.lastSeenUnixMs = lastSeenUnixMs;
    }

    /**
     * 返回修订号.
     *
     * @return 修订号
     */
    public long getRevision() {
        return revision;
    }

    /**
     * 返回 TTL.
     *
     * @return TTL, 单位为秒
     */
    public long getTtlSecs() {
        return ttlSecs;
    }

    /**
     * 返回过期时间.
     *
     * @return Unix epoch milliseconds
     */
    public long getExpiresAtUnixMs() {
        return expiresAtUnixMs;
    }

    /**
     * 返回最近一次上报时间.
     *
     * @return Unix epoch milliseconds
     */
    public long getLastSeenUnixMs() {
        return lastSeenUnixMs;
    }

    @Override
    public String toString() {
        return "LeaseInfo{"
            + "revision=" + revision
            + ", ttlSecs=" + ttlSecs
            + ", expiresAtUnixMs=" + expiresAtUnixMs
            + ", lastSeenUnixMs=" + lastSeenUnixMs
            + '}';
    }
}
