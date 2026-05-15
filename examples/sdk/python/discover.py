from lnd import Client, DiscoveryFilter


with Client("http://127.0.0.1:8765", "dev-token") as client:
    nodes = client.discover(
        DiscoveryFilter("office-a").with_service("_demo._tcp").add_tag("stable")
    )
    print(nodes)
