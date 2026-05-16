import io.github.azazo1.lnd.Client;
import io.github.azazo1.lnd.DiscoveryFilter;
import io.github.azazo1.lnd.WatchHandle;

public final class WatchCloseSmoke {
    public static void main(String[] args) throws Exception {
        String serverUrl = args.length > 0 ? args[0] : "http://127.0.0.1:8765";
        String bearerToken = args.length > 1 ? args[1] : "dev-token";

        Client client = new Client(serverUrl, bearerToken);
        WatchHandle handle = client.watch(new DiscoveryFilter(), envelope -> {
        });

        Thread.sleep(300L);
        long startedAt = System.nanoTime();
        handle.close();
        boolean stopped = handle.awaitStopped(2_000L);
        long elapsedMillis = (System.nanoTime() - startedAt) / 1_000_000L;

        if (!stopped) {
            throw new IllegalStateException("watch did not stop within 2000 ms");
        }

        System.out.println("watch_close_elapsed_ms=" + elapsedMillis);
    }
}
