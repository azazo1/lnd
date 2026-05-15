package io.github.azazo1.lnd;

/**
 * 带 cursor 的 watch 事件封装.
 *
 * <p>功能简介:
 *
 * <ul>
 *   <li>保留服务端返回的最新 resume cursor
 *   <li>供调用方在需要时自行记录事件推进位置
 * </ul>
 *
 * <p>注意事项:
 *
 * <ul>
 *   <li>`cursor = null` 常见于 `reset`
 *   <li>`watch()` 已经内建了自动 cursor 恢复, 常规调用方不需要自己处理重连逻辑
 * </ul>
 */
public final class DiscoveryEventEnvelope {
    private final Long cursor;
    private final DiscoveryEvent event;

    /**
     * 创建事件封装.
     *
     * @param cursor 最新 cursor, 没有时为 `null`
     * @param event 事件体
     */
    public DiscoveryEventEnvelope(Long cursor, DiscoveryEvent event) {
        this.cursor = cursor;
        this.event = event;
    }

    /**
     * 返回最新 cursor.
     *
     * @return cursor, 没有时为 `null`
     */
    public Long getCursor() {
        return cursor;
    }

    /**
     * 返回事件体.
     *
     * @return 事件体
     */
    public DiscoveryEvent getEvent() {
        return event;
    }

    @Override
    public String toString() {
        return "DiscoveryEventEnvelope{"
            + "cursor=" + cursor
            + ", event=" + event
            + '}';
    }
}
