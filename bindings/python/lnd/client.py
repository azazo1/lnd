import ctypes
import ctypes.util
import json
import os
import platform
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable


WatchCallback = Callable[[dict], None]


def _project_root() -> Path:
    return Path(__file__).resolve().parents[3]


def _platform_library_filename() -> str:
    system = platform.system()
    if system == "Darwin":
        return "liblnd.dylib"
    if system == "Windows":
        return "lnd.dll"
    return "liblnd.so"


def _default_library_path() -> Path:
    release_dir = _project_root() / "target" / "release"
    return release_dir / _platform_library_filename()


def _resolve_library_path(library_path: str | None = None) -> str:
    if library_path:
        return library_path
    env_path = os.environ.get("LND_LIBRARY_PATH")
    if env_path:
        return env_path
    found = ctypes.util.find_library("lnd")
    if found:
        return found
    return os.fspath(_default_library_path())


class LndError(RuntimeError):
    pass


class _Bindings:
    def __init__(self, library_path: str | None = None) -> None:
        self.lib = ctypes.CDLL(_resolve_library_path(library_path))
        self._configure()

    def _configure(self) -> None:
        self.lib.lnd_client_new.argtypes = [ctypes.c_char_p, ctypes.c_char_p]
        self.lib.lnd_client_new.restype = ctypes.c_void_p
        self.lib.lnd_client_free.argtypes = [ctypes.c_void_p]

        self.lib.lnd_client_set_server_url.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
        self.lib.lnd_client_set_server_url.restype = ctypes.c_bool
        self.lib.lnd_client_set_bearer_token.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
        self.lib.lnd_client_set_bearer_token.restype = ctypes.c_bool
        self.lib.lnd_client_set_timeout_ms.argtypes = [ctypes.c_void_p, ctypes.c_uint64]
        self.lib.lnd_client_set_timeout_ms.restype = ctypes.c_bool
        self.lib.lnd_client_set_reconnect_backoff_ms.argtypes = [
            ctypes.c_void_p,
            ctypes.c_uint64,
            ctypes.c_uint64,
        ]
        self.lib.lnd_client_set_reconnect_backoff_ms.restype = ctypes.c_bool
        self.lib.lnd_client_set_include_loopback.argtypes = [ctypes.c_void_p, ctypes.c_bool]
        self.lib.lnd_client_set_include_loopback.restype = ctypes.c_bool
        self.lib.lnd_client_set_include_ipv6.argtypes = [ctypes.c_void_p, ctypes.c_bool]
        self.lib.lnd_client_set_include_ipv6.restype = ctypes.c_bool
        self.lib.lnd_client_set_include_private_ipv4.argtypes = [ctypes.c_void_p, ctypes.c_bool]
        self.lib.lnd_client_set_include_private_ipv4.restype = ctypes.c_bool
        self.lib.lnd_client_set_include_link_local_ipv4.argtypes = [
            ctypes.c_void_p,
            ctypes.c_bool,
        ]
        self.lib.lnd_client_set_include_link_local_ipv4.restype = ctypes.c_bool
        self.lib.lnd_client_enable_interface.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
        self.lib.lnd_client_enable_interface.restype = ctypes.c_bool
        self.lib.lnd_client_disable_interface.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
        self.lib.lnd_client_disable_interface.restype = ctypes.c_bool
        self.lib.lnd_client_clear_interface_filters.argtypes = [ctypes.c_void_p]
        self.lib.lnd_client_clear_interface_filters.restype = ctypes.c_bool

        self.lib.lnd_discovery_filter_new.argtypes = [ctypes.c_char_p]
        self.lib.lnd_discovery_filter_new.restype = ctypes.c_void_p
        self.lib.lnd_discovery_filter_free.argtypes = [ctypes.c_void_p]
        self.lib.lnd_discovery_filter_set_service.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
        self.lib.lnd_discovery_filter_set_service.restype = ctypes.c_bool
        self.lib.lnd_discovery_filter_clear_service.argtypes = [ctypes.c_void_p]
        self.lib.lnd_discovery_filter_clear_service.restype = ctypes.c_bool
        self.lib.lnd_discovery_filter_add_tag.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
        self.lib.lnd_discovery_filter_add_tag.restype = ctypes.c_bool
        self.lib.lnd_discovery_filter_clear_tags.argtypes = [ctypes.c_void_p]
        self.lib.lnd_discovery_filter_clear_tags.restype = ctypes.c_bool
        self.lib.lnd_discover.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
        self.lib.lnd_discover.restype = ctypes.c_void_p

        self.lib.lnd_announce_spec_new.argtypes = [
            ctypes.c_char_p,
            ctypes.c_char_p,
            ctypes.c_char_p,
            ctypes.c_char_p,
            ctypes.c_uint16,
        ]
        self.lib.lnd_announce_spec_new.restype = ctypes.c_void_p
        self.lib.lnd_announce_spec_free.argtypes = [ctypes.c_void_p]
        self.lib.lnd_announce_spec_set_auto_lan_addrs.argtypes = [ctypes.c_void_p, ctypes.c_bool]
        self.lib.lnd_announce_spec_set_auto_lan_addrs.restype = ctypes.c_bool
        self.lib.lnd_announce_spec_add_lan_addr.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
        self.lib.lnd_announce_spec_add_lan_addr.restype = ctypes.c_bool
        self.lib.lnd_announce_spec_set_include_loopback.argtypes = [ctypes.c_void_p, ctypes.c_bool]
        self.lib.lnd_announce_spec_set_include_loopback.restype = ctypes.c_bool
        self.lib.lnd_announce_spec_set_include_ipv6.argtypes = [ctypes.c_void_p, ctypes.c_bool]
        self.lib.lnd_announce_spec_set_include_ipv6.restype = ctypes.c_bool
        self.lib.lnd_announce_spec_set_include_private_ipv4.argtypes = [
            ctypes.c_void_p,
            ctypes.c_bool,
        ]
        self.lib.lnd_announce_spec_set_include_private_ipv4.restype = ctypes.c_bool
        self.lib.lnd_announce_spec_set_include_link_local_ipv4.argtypes = [
            ctypes.c_void_p,
            ctypes.c_bool,
        ]
        self.lib.lnd_announce_spec_set_include_link_local_ipv4.restype = ctypes.c_bool
        self.lib.lnd_announce_spec_enable_interface.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
        self.lib.lnd_announce_spec_enable_interface.restype = ctypes.c_bool
        self.lib.lnd_announce_spec_disable_interface.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
        self.lib.lnd_announce_spec_disable_interface.restype = ctypes.c_bool
        self.lib.lnd_announce_spec_add_tag.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
        self.lib.lnd_announce_spec_add_tag.restype = ctypes.c_bool
        self.lib.lnd_announce_spec_insert_metadata.argtypes = [
            ctypes.c_void_p,
            ctypes.c_char_p,
            ctypes.c_char_p,
        ]
        self.lib.lnd_announce_spec_insert_metadata.restype = ctypes.c_bool
        self.lib.lnd_announce_spec_set_ttl_secs.argtypes = [ctypes.c_void_p, ctypes.c_uint64]
        self.lib.lnd_announce_spec_set_ttl_secs.restype = ctypes.c_bool
        self.lib.lnd_resolve_announce_addrs_json.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
        self.lib.lnd_resolve_announce_addrs_json.restype = ctypes.c_void_p
        self.lib.lnd_announce_once.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
        self.lib.lnd_announce_once.restype = ctypes.c_void_p
        self.lib.lnd_announce_start_with_spec.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
        self.lib.lnd_announce_start_with_spec.restype = ctypes.c_void_p
        self.lib.lnd_announce_stop.argtypes = [ctypes.c_void_p]

        self._watch_callback_type = ctypes.CFUNCTYPE(None, ctypes.c_char_p, ctypes.c_void_p)
        self.lib.lnd_watch_start_with_filter.argtypes = [
            ctypes.c_void_p,
            ctypes.c_void_p,
            self._watch_callback_type,
            ctypes.c_void_p,
        ]
        self.lib.lnd_watch_start_with_filter.restype = ctypes.c_void_p
        self.lib.lnd_watch_stop.argtypes = [ctypes.c_void_p]

        self.lib.lnd_string_free.argtypes = [ctypes.c_void_p]
        self.lib.lnd_last_error.restype = ctypes.c_char_p

    def last_error(self) -> str:
        value = self.lib.lnd_last_error()
        if not value:
            return "unknown lnd error"
        return value.decode("utf-8")

    def check_bool(self, ok: bool) -> None:
        if not ok:
            raise LndError(self.last_error())

    def check_ptr(self, ptr: int | None) -> int:
        if not ptr:
            raise LndError(self.last_error())
        return ptr

    def read_json_string(self, ptr: int) -> object:
        try:
            return json.loads(ctypes.string_at(ptr).decode("utf-8"))
        finally:
            self.lib.lnd_string_free(ptr)


_DEFAULT_BINDINGS: _Bindings | None = None


def _default_bindings() -> _Bindings:
    global _DEFAULT_BINDINGS
    if _DEFAULT_BINDINGS is None:
        _DEFAULT_BINDINGS = _Bindings()
    return _DEFAULT_BINDINGS


def _encode(value: str | None) -> bytes | None:
    if value is None:
        return None
    return value.encode("utf-8")


@dataclass
class DiscoveryFilter:
    network_id: str
    service: str | None = None
    tags: list[str] = field(default_factory=list)

    def with_service(self, service: str) -> "DiscoveryFilter":
        self.service = service
        return self

    def add_tag(self, tag: str) -> "DiscoveryFilter":
        self.tags.append(tag)
        return self

    def _into_handle(self, bindings: _Bindings) -> int:
        handle = bindings.check_ptr(
            bindings.lib.lnd_discovery_filter_new(_encode(self.network_id))
        )
        try:
            if self.service is not None:
                bindings.check_bool(
                    bindings.lib.lnd_discovery_filter_set_service(handle, _encode(self.service))
                )
            for tag in self.tags:
                bindings.check_bool(bindings.lib.lnd_discovery_filter_add_tag(handle, _encode(tag)))
            return handle
        except Exception:
            bindings.lib.lnd_discovery_filter_free(handle)
            raise


@dataclass
class AnnounceSpec:
    network_id: str
    node_id: str
    service: str
    display_name: str
    port: int
    auto_lan_addrs: bool = True
    ttl_secs: int = 30
    lan_addrs: list[str] = field(default_factory=list)
    tags: list[str] = field(default_factory=list)
    metadata: dict[str, str] = field(default_factory=dict)
    include_loopback: bool = False
    include_ipv6: bool = False
    include_private_ipv4: bool = True
    include_link_local_ipv4: bool = False
    interface_allowlist: list[str] = field(default_factory=list)
    interface_denylist: list[str] = field(default_factory=list)

    def add_lan_addr(self, addr: str) -> "AnnounceSpec":
        self.lan_addrs.append(addr)
        return self

    def add_tag(self, tag: str) -> "AnnounceSpec":
        self.tags.append(tag)
        return self

    def insert_metadata(self, key: str, value: str) -> "AnnounceSpec":
        self.metadata[key] = value
        return self

    def enable_interface(self, name: str) -> "AnnounceSpec":
        self.interface_allowlist.append(name)
        return self

    def disable_interface(self, name: str) -> "AnnounceSpec":
        self.interface_denylist.append(name)
        return self

    def _into_handle(self, bindings: _Bindings) -> int:
        handle = bindings.check_ptr(
            bindings.lib.lnd_announce_spec_new(
                _encode(self.network_id),
                _encode(self.node_id),
                _encode(self.service),
                _encode(self.display_name),
                self.port,
            )
        )
        try:
            bindings.check_bool(
                bindings.lib.lnd_announce_spec_set_auto_lan_addrs(handle, self.auto_lan_addrs)
            )
            bindings.check_bool(bindings.lib.lnd_announce_spec_set_ttl_secs(handle, self.ttl_secs))
            bindings.check_bool(
                bindings.lib.lnd_announce_spec_set_include_loopback(handle, self.include_loopback)
            )
            bindings.check_bool(
                bindings.lib.lnd_announce_spec_set_include_ipv6(handle, self.include_ipv6)
            )
            bindings.check_bool(
                bindings.lib.lnd_announce_spec_set_include_private_ipv4(
                    handle, self.include_private_ipv4
                )
            )
            bindings.check_bool(
                bindings.lib.lnd_announce_spec_set_include_link_local_ipv4(
                    handle, self.include_link_local_ipv4
                )
            )
            for addr in self.lan_addrs:
                bindings.check_bool(bindings.lib.lnd_announce_spec_add_lan_addr(handle, _encode(addr)))
            for tag in self.tags:
                bindings.check_bool(bindings.lib.lnd_announce_spec_add_tag(handle, _encode(tag)))
            for key, value in self.metadata.items():
                bindings.check_bool(
                    bindings.lib.lnd_announce_spec_insert_metadata(
                        handle,
                        _encode(key),
                        _encode(value),
                    )
                )
            for interface_name in self.interface_allowlist:
                bindings.check_bool(
                    bindings.lib.lnd_announce_spec_enable_interface(
                        handle, _encode(interface_name)
                    )
                )
            for interface_name in self.interface_denylist:
                bindings.check_bool(
                    bindings.lib.lnd_announce_spec_disable_interface(
                        handle, _encode(interface_name)
                    )
                )
            return handle
        except Exception:
            bindings.lib.lnd_announce_spec_free(handle)
            raise


class AnnounceHandle:
    def __init__(self, bindings: _Bindings, handle: int) -> None:
        self._bindings = bindings
        self._handle = handle

    def close(self) -> None:
        handle = getattr(self, "_handle", 0)
        bindings = getattr(self, "_bindings", None)
        if handle and bindings is not None:
            bindings.lib.lnd_announce_stop(handle)
            self._handle = 0

    def __enter__(self) -> "AnnounceHandle":
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        self.close()

    def __del__(self) -> None:
        self.close()


class WatchHandle:
    def __init__(self, bindings: _Bindings, handle: int, callback_ref) -> None:
        self._bindings = bindings
        self._handle = handle
        self._callback_ref = callback_ref

    def close(self) -> None:
        handle = getattr(self, "_handle", 0)
        bindings = getattr(self, "_bindings", None)
        if handle and bindings is not None:
            bindings.lib.lnd_watch_stop(handle)
            self._handle = 0
            self._callback_ref = None

    def __enter__(self) -> "WatchHandle":
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        self.close()

    def __del__(self) -> None:
        self.close()


class Client:
    def __init__(
        self,
        server_url: str,
        bearer_token: str = "",
        *,
        timeout_ms: int | None = None,
        reconnect_backoff_ms: tuple[int, int] | None = None,
        include_loopback: bool = False,
        include_ipv6: bool = False,
        include_private_ipv4: bool = True,
        include_link_local_ipv4: bool = False,
        library_path: str | None = None,
    ) -> None:
        self._bindings = _default_bindings() if library_path is None else _Bindings(library_path)
        self._handle = self._bindings.check_ptr(
            self._bindings.lib.lnd_client_new(_encode(server_url), _encode(bearer_token))
        )
        self.set_include_loopback(include_loopback)
        self.set_include_ipv6(include_ipv6)
        self.set_include_private_ipv4(include_private_ipv4)
        self.set_include_link_local_ipv4(include_link_local_ipv4)
        if timeout_ms is not None:
            self.set_timeout_ms(timeout_ms)
        if reconnect_backoff_ms is not None:
            self.set_reconnect_backoff_ms(*reconnect_backoff_ms)

    def close(self) -> None:
        handle = getattr(self, "_handle", 0)
        bindings = getattr(self, "_bindings", None)
        if handle and bindings is not None:
            bindings.lib.lnd_client_free(handle)
            self._handle = 0

    def __enter__(self) -> "Client":
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        self.close()

    def __del__(self) -> None:
        self.close()

    def set_server_url(self, server_url: str) -> "Client":
        self._bindings.check_bool(
            self._bindings.lib.lnd_client_set_server_url(self._handle, _encode(server_url))
        )
        return self

    def set_bearer_token(self, bearer_token: str) -> "Client":
        self._bindings.check_bool(
            self._bindings.lib.lnd_client_set_bearer_token(self._handle, _encode(bearer_token))
        )
        return self

    def set_timeout_ms(self, timeout_ms: int) -> "Client":
        self._bindings.check_bool(
            self._bindings.lib.lnd_client_set_timeout_ms(self._handle, timeout_ms)
        )
        return self

    def set_reconnect_backoff_ms(self, min_ms: int, max_ms: int) -> "Client":
        self._bindings.check_bool(
            self._bindings.lib.lnd_client_set_reconnect_backoff_ms(
                self._handle,
                min_ms,
                max_ms,
            )
        )
        return self

    def set_include_loopback(self, on: bool) -> "Client":
        self._bindings.check_bool(
            self._bindings.lib.lnd_client_set_include_loopback(self._handle, on)
        )
        return self

    def set_include_ipv6(self, on: bool) -> "Client":
        self._bindings.check_bool(
            self._bindings.lib.lnd_client_set_include_ipv6(self._handle, on)
        )
        return self

    def set_include_private_ipv4(self, on: bool) -> "Client":
        self._bindings.check_bool(
            self._bindings.lib.lnd_client_set_include_private_ipv4(self._handle, on)
        )
        return self

    def set_include_link_local_ipv4(self, on: bool) -> "Client":
        self._bindings.check_bool(
            self._bindings.lib.lnd_client_set_include_link_local_ipv4(self._handle, on)
        )
        return self

    def enable_interface(self, interface_name: str) -> "Client":
        self._bindings.check_bool(
            self._bindings.lib.lnd_client_enable_interface(self._handle, _encode(interface_name))
        )
        return self

    def disable_interface(self, interface_name: str) -> "Client":
        self._bindings.check_bool(
            self._bindings.lib.lnd_client_disable_interface(self._handle, _encode(interface_name))
        )
        return self

    def clear_interface_filters(self) -> "Client":
        self._bindings.check_bool(
            self._bindings.lib.lnd_client_clear_interface_filters(self._handle)
        )
        return self

    def discover(self, filter_spec: DiscoveryFilter) -> list[dict]:
        filter_handle = filter_spec._into_handle(self._bindings)
        try:
            response = self._bindings.check_ptr(
                self._bindings.lib.lnd_discover(self._handle, filter_handle)
            )
            result = self._bindings.read_json_string(response)
            assert isinstance(result, list)
            return result
        finally:
            self._bindings.lib.lnd_discovery_filter_free(filter_handle)

    def resolve_announce_addrs(self, spec: AnnounceSpec) -> list[str]:
        spec_handle = spec._into_handle(self._bindings)
        try:
            response = self._bindings.check_ptr(
                self._bindings.lib.lnd_resolve_announce_addrs_json(self._handle, spec_handle)
            )
            result = self._bindings.read_json_string(response)
            assert isinstance(result, list)
            return result
        finally:
            self._bindings.lib.lnd_announce_spec_free(spec_handle)

    def announce_once(self, spec: AnnounceSpec) -> dict:
        spec_handle = spec._into_handle(self._bindings)
        try:
            response = self._bindings.check_ptr(
                self._bindings.lib.lnd_announce_once(self._handle, spec_handle)
            )
            result = self._bindings.read_json_string(response)
            assert isinstance(result, dict)
            return result
        finally:
            self._bindings.lib.lnd_announce_spec_free(spec_handle)

    def announce(self, spec: AnnounceSpec) -> AnnounceHandle:
        spec_handle = spec._into_handle(self._bindings)
        try:
            handle = self._bindings.check_ptr(
                self._bindings.lib.lnd_announce_start_with_spec(self._handle, spec_handle)
            )
            return AnnounceHandle(self._bindings, handle)
        finally:
            self._bindings.lib.lnd_announce_spec_free(spec_handle)

    def watch(self, filter_spec: DiscoveryFilter, callback: WatchCallback) -> WatchHandle:
        filter_handle = filter_spec._into_handle(self._bindings)
        try:
            def on_event(payload, _user_data) -> None:
                callback(json.loads(payload.decode("utf-8")))

            callback_ref = self._bindings._watch_callback_type(on_event)
            handle = self._bindings.check_ptr(
                self._bindings.lib.lnd_watch_start_with_filter(
                    self._handle,
                    filter_handle,
                    callback_ref,
                    None,
                )
            )
            return WatchHandle(self._bindings, handle, callback_ref)
        finally:
            self._bindings.lib.lnd_discovery_filter_free(filter_handle)
