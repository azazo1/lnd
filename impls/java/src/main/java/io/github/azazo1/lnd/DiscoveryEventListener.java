package io.github.azazo1.lnd;

/**
 * watch 事件监听器.
 *
 * <p>功能简介:
 *
 * <ul>
 *   <li>供 {@link Client#watch(DiscoveryFilter, DiscoveryEventListener)} 和
 *       {@link Client#watchWithAutoScopeOverlap(DiscoveryFilter, DiscoveryEventListener)} 使用
 *   <li>回调会在后台 watch 线程中执行
 * </ul>
 *
 * <p>注意事项:
 *
 * <ul>
 *   <li>回调内部应避免长时间阻塞
 *   <li>如果要切回 Android 主线程, 应在应用层自行切换线程
 * </ul>
 */
public interface DiscoveryEventListener {
    /**
     * 接收一个 watch 事件.
     *
     * @param envelope 事件封装
     */
    void onEvent(DiscoveryEventEnvelope envelope);
}
