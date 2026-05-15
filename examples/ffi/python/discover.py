import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "bindings" / "python"))

from lnd import Client, DiscoveryFilter


with Client("http://127.0.0.1:8765", "dev-token") as client:
    nodes = client.discover(
        DiscoveryFilter("office-a").with_service("_demo._tcp").add_tag("stable")
    )
    print(nodes)
