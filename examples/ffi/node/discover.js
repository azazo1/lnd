import path from "node:path";
import { fileURLToPath } from "node:url";
import koffi from "koffi";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..", "..", "..");
const releaseDir = path.join(root, "target", "release");

function libraryName() {
  switch (process.platform) {
    case "darwin":
      return "liblnd.dylib";
    case "win32":
      return "lnd.dll";
    default:
      return "liblnd.so";
  }
}

const lib = koffi.load(path.join(releaseDir, libraryName()));

const lnd_client_new = lib.func("lnd_client_new", "void *", ["str", "str"]);
const lnd_client_free = lib.func("lnd_client_free", "void", ["void *"]);
const lnd_discovery_filter_new = lib.func("lnd_discovery_filter_new", "void *", ["str"]);
const lnd_discovery_filter_set_service = lib.func(
  "lnd_discovery_filter_set_service",
  "bool",
  ["void *", "str"],
);
const lnd_discovery_filter_free = lib.func("lnd_discovery_filter_free", "void", ["void *"]);
const lnd_discover = lib.func("lnd_discover", "void *", ["void *", "void *"]);
const lnd_string_free = lib.func("lnd_string_free", "void", ["void *"]);
const lnd_last_error = lib.func("lnd_last_error", "str", []);

const client = lnd_client_new("http://127.0.0.1:8765", "dev-token");
if (!client) {
  throw new Error(lnd_last_error());
}

const filter = lnd_discovery_filter_new("office-a");
if (!filter) {
  lnd_client_free(client);
  throw new Error(lnd_last_error());
}

if (!lnd_discovery_filter_set_service(filter, "_demo._tcp")) {
  lnd_discovery_filter_free(filter);
  lnd_client_free(client);
  throw new Error(lnd_last_error());
}

const jsonPtr = lnd_discover(client, filter);
if (!jsonPtr) {
  lnd_discovery_filter_free(filter);
  lnd_client_free(client);
  throw new Error(lnd_last_error());
}

console.log(koffi.decode(jsonPtr, "char", -1));

lnd_string_free(jsonPtr);
lnd_discovery_filter_free(filter);
lnd_client_free(client);
