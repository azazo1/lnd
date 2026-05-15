package io.github.azazo1.lnd;

/**
 * Java SDK 的统一异常类型.
 *
 * <p>功能简介:
 *
 * <ul>
 *   <li>封装 HTTP, JSON, SSE, 自动地址推导等失败
 *   <li>供同步 API 直接抛出
 *   <li>供后台 announce 和 watch 句柄记录最终错误
 * </ul>
 *
 * <p>注意事项:
 *
 * <ul>
 *   <li>后台循环启动本身是异步的, 启动后的错误会保存在 handle 中
 *   <li>使用 {@link AnnounceHandle#getLastError()} 或 {@link WatchHandle#getLastError()} 获取后台错误
 * </ul>
 */
public final class LndException extends Exception {

    /**
     * 创建一个只包含错误消息的异常.
     *
     * @param message 错误说明
     */
    public LndException(String message) {
        super(message);
    }

    /**
     * 创建一个带底层原因的异常.
     *
     * @param message 错误说明
     * @param cause 底层异常
     */
    public LndException(String message, Throwable cause) {
        super(message, cause);
    }
}
