from lnd import Client, DiscoveryFilter


with Client("http://127.0.0.1:8765", "dev-token") as client:
    nodes = client.discover_with_auto_scope_overlap(
        DiscoveryFilter().with_discovery_domain("office-a").with_service("_http._tcp").add_tag("stable")
    )
    print(nodes)
