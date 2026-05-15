import ctypes
import os
import platform


def load_library():
    root = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
    release_dir = os.path.join(root, "target", "release")
    system = platform.system()
    if system == "Darwin":
        filename = "liblnd.dylib"
    elif system == "Windows":
        filename = "lnd.dll"
    else:
        filename = "liblnd.so"
    return ctypes.CDLL(os.path.join(release_dir, filename))


lib = load_library()

lib.lnd_client_new.argtypes = [ctypes.c_char_p, ctypes.c_char_p]
lib.lnd_client_new.restype = ctypes.c_void_p
lib.lnd_client_free.argtypes = [ctypes.c_void_p]
lib.lnd_discovery_filter_new.argtypes = [ctypes.c_char_p]
lib.lnd_discovery_filter_new.restype = ctypes.c_void_p
lib.lnd_discovery_filter_set_service.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
lib.lnd_discovery_filter_set_service.restype = ctypes.c_bool
lib.lnd_discovery_filter_free.argtypes = [ctypes.c_void_p]
lib.lnd_discover.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
lib.lnd_discover.restype = ctypes.c_void_p
lib.lnd_string_free.argtypes = [ctypes.c_void_p]
lib.lnd_last_error.restype = ctypes.c_char_p

client = lib.lnd_client_new(b"http://127.0.0.1:8765", b"dev-token")
if not client:
    raise RuntimeError(lib.lnd_last_error().decode())

filter_handle = lib.lnd_discovery_filter_new(b"office-a")
if not filter_handle:
    lib.lnd_client_free(client)
    raise RuntimeError(lib.lnd_last_error().decode())

if not lib.lnd_discovery_filter_set_service(filter_handle, b"_demo._tcp"):
    lib.lnd_discovery_filter_free(filter_handle)
    lib.lnd_client_free(client)
    raise RuntimeError(lib.lnd_last_error().decode())

json_ptr = lib.lnd_discover(client, filter_handle)
if not json_ptr:
    lib.lnd_discovery_filter_free(filter_handle)
    lib.lnd_client_free(client)
    raise RuntimeError(lib.lnd_last_error().decode())

print(ctypes.string_at(json_ptr).decode())

lib.lnd_string_free(json_ptr)
lib.lnd_discovery_filter_free(filter_handle)
lib.lnd_client_free(client)
