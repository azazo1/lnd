from lnd import Client, DiscoveryFilter


with Client("http://127.0.0.1:8765", "dev-token") as client:
    network_id = client.resolve_network_id()
    nodes = client.discover(
        DiscoveryFilter(network_id).with_service("_demo._tcp").add_tag("stable")
    )
    print(nodes)
