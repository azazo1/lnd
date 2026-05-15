public final class Discover {
    public static void main(String[] args) {
        try (Lnd.Client client = new Lnd.Client("http://127.0.0.1:8765", "dev-token")) {
            String nodes = client.discoverJson(
                new Lnd.DiscoveryFilter("office-a")
                    .withService("_demo._tcp")
                    .addTag("stable")
            );
            System.out.println(nodes);
        }
    }
}
