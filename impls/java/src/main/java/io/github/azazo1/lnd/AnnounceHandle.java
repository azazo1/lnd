package io.github.azazo1.lnd;

import java.net.HttpURLConnection;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicReference;

/**
 * 后台 announce 循环句柄.
 *
 * <p>功能简介:
 *
 * <ul>
 *   <li>由 {@link Client#announce(AnnounceSpec)} 返回
 *   <li>负责控制长时间续租任务的停止和等待
 * </ul>
 *
 * <p>注意事项:
 *
 * <ul>
 *   <li>创建 handle 时并不保证首次注册已经成功
 *   <li>后台错误会记录到 handle 中
 * </ul>
 */
public final class AnnounceHandle implements AutoCloseable {
    private final AtomicBoolean stopRequested = new AtomicBoolean(false);
    private final CountDownLatch stopped = new CountDownLatch(1);
    private final AtomicReference<HttpURLConnection> activeConnection = new AtomicReference<HttpURLConnection>(null);
    private volatile LndException lastError;
    private volatile Thread thread;

    AnnounceHandle() {
    }

    void bindThread(Thread thread) {
        this.thread = thread;
    }

    void fail(LndException error) {
        this.lastError = error;
    }

    void bindConnection(HttpURLConnection connection) {
        activeConnection.set(connection);
        if (isStopRequested() && connection != null) {
            connection.disconnect();
            activeConnection.compareAndSet(connection, null);
        }
    }

    void clearConnection(HttpURLConnection connection) {
        if (connection != null) {
            activeConnection.compareAndSet(connection, null);
        }
    }

    void finish() {
        stopped.countDown();
    }

    boolean isStopRequested() {
        return stopRequested.get();
    }

    /**
     * 请求停止后台 announce 循环.
     *
     * <p>返回值说明:
     *
     * <ul>
     *   <li>本方法无返回值
     *   <li>可继续调用 {@link #awaitStopped()} 等待线程退出
     * </ul>
     */
    @Override
    public void close() {
        stopRequested.set(true);
        HttpURLConnection connection = activeConnection.getAndSet(null);
        if (connection != null) {
            connection.disconnect();
        }
        Thread current = thread;
        if (current != null) {
            current.interrupt();
        }
    }

    /**
     * 判断后台线程是否仍在运行.
     *
     * @return true 表示尚未退出
     */
    public boolean isRunning() {
        return stopped.getCount() > 0L;
    }

    /**
     * 返回后台循环最终错误.
     *
     * <p>注意事项:
     *
     * <ul>
     *   <li>如果后台尚未结束, 此值可能仍然为 `null`
     *   <li>`null` 表示正常停止或尚无错误
     * </ul>
     *
     * @return 最终错误或 `null`
     */
    public LndException getLastError() {
        return lastError;
    }

    /**
     * 阻塞等待后台线程退出.
     *
     * @return 最终错误, 正常停止时为 `null`
     * @throws InterruptedException 当前线程被中断
     */
    public LndException awaitStopped() throws InterruptedException {
        stopped.await();
        return lastError;
    }

    /**
     * 在给定超时内等待后台线程退出.
     *
     * @param timeoutMillis 超时毫秒数
     * @return true 表示已退出, false 表示超时
     * @throws InterruptedException 当前线程被中断
     */
    public boolean awaitStopped(long timeoutMillis) throws InterruptedException {
        return stopped.await(timeoutMillis, TimeUnit.MILLISECONDS);
    }
}
