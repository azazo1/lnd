import { Client, DiscoveryFilter } from "../../../bindings/node/index.js";

const client = new Client("http://127.0.0.1:8765", "dev-token");

try {
  const nodes = client.discover(
    new DiscoveryFilter("office-a").withService("_demo._tcp").addTag("stable"),
  );
  console.log(nodes);
} finally {
  client.close();
}
