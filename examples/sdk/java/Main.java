import io.github.azazo1.lnd.Client;
import io.github.azazo1.lnd.DiscoveryFilter;

import java.util.List;

public final class Main {
    public static void main(String[] args) throws Exception {
        Client client = new Client("http://127.0.0.1:8765", "dev-token");
        String networkId = client.resolveNetworkId();

        DiscoveryFilter filter = new DiscoveryFilter()
            .withNetworkId(networkId)
            .withService("_demo._tcp")
            .addTag("stable");

        List<String> scopes = client.listReachabilityScopes();
        for (String scope : scopes) {
            filter.addReachabilityScope(scope);
        }

        System.out.println(client.discover(filter));
    }
}
