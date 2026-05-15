import com.sun.jna.Library;
import com.sun.jna.Native;
import com.sun.jna.Pointer;

public final class Discover {
    public interface LndLibrary extends Library {
        LndLibrary INSTANCE = Native.load("lnd", LndLibrary.class);

        Pointer lnd_client_new(String serverUrl, String bearerToken);
        void lnd_client_free(Pointer handle);
        Pointer lnd_discovery_filter_new(String networkId);
        boolean lnd_discovery_filter_set_service(Pointer handle, String service);
        void lnd_discovery_filter_free(Pointer handle);
        Pointer lnd_discover(Pointer client, Pointer filter);
        void lnd_string_free(Pointer value);
        String lnd_last_error();
    }

    public static void main(String[] args) {
        LndLibrary lib = LndLibrary.INSTANCE;
        Pointer client = lib.lnd_client_new("http://127.0.0.1:8765", "dev-token");
        if (client == null) {
            throw new IllegalStateException(lib.lnd_last_error());
        }

        Pointer filter = lib.lnd_discovery_filter_new("office-a");
        if (filter == null) {
            lib.lnd_client_free(client);
            throw new IllegalStateException(lib.lnd_last_error());
        }

        if (!lib.lnd_discovery_filter_set_service(filter, "_demo._tcp")) {
            lib.lnd_discovery_filter_free(filter);
            lib.lnd_client_free(client);
            throw new IllegalStateException(lib.lnd_last_error());
        }

        Pointer json = lib.lnd_discover(client, filter);
        if (json == null) {
            lib.lnd_discovery_filter_free(filter);
            lib.lnd_client_free(client);
            throw new IllegalStateException(lib.lnd_last_error());
        }

        System.out.println(json.getString(0));
        lib.lnd_string_free(json);
        lib.lnd_discovery_filter_free(filter);
        lib.lnd_client_free(client);
    }
}
