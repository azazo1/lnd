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
    """`lnd` Python SDK 的统一异常类型.

    功能简介:
    - 表示底层 `liblnd` 返回失败, 或绑定层无法完成请求.

    常见触发场景:
    - 动态库加载失败.
    - server 返回错误.
    - FFI 句柄创建失败.
    """
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
    """一次性发现和持续 watch 使用的过滤器.

    参数:
    - `network_id`: 逻辑发现域, 必填.
    - `service`: 可选服务名过滤器.
    - `tags`: 需要全部满足的 tag 列表.

    返回值:
    - 该类的方法都会返回 `self`, 便于链式调用.

    使用示例:
    ```python
    filter_spec = DiscoveryFilter("office-a").with_service("_demo._tcp").add_tag("stable")
    ```
    """
    network_id: str
    service: str | None = None
    tags: list[str] = field(default_factory=list)

    def with_service(self, service: str) -> "DiscoveryFilter":
        """设置服务名过滤条件.

        参数:
        - `service`: 例如 `_demo._tcp`.
        """
        self.service = service
        return self

    def add_tag(self, tag: str) -> "DiscoveryFilter":
        """追加一个 tag 过滤条件."""
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
    """面向 Python 调用方的注册规格.

    功能简介:
    - 描述一个节点如何向 `lnd-server` 注册自己.
    - 同时支持显式 `lan_addrs` 和自动地址解析参数.

    参数:
    - `network_id`: 发现域.
    - `node_id`: 持久节点标识.
    - `service`: 服务名.
    - `display_name`: 展示名称.
    - `port`: 服务端口.

    注意事项:
    - `auto_lan_addrs=False` 且 `lan_addrs` 为空时, 最终可能上报空地址列表.
    - `include_*` 和接口过滤只影响自动地址解析.
    """
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
        """追加一个显式地址.

        参数:
        - `addr`: `ip:port` 或纯 IP 字符串, 由底层库解析.
        """
        self.lan_addrs.append(addr)
        return self

    def add_tag(self, tag: str) -> "AnnounceSpec":
        """追加一个 tag."""
        self.tags.append(tag)
        return self

    def insert_metadata(self, key: str, value: str) -> "AnnounceSpec":
        """插入一个 metadata 键值对."""
        self.metadata[key] = value
        return self

    def enable_interface(self, name: str) -> "AnnounceSpec":
        """向自动地址解析规则追加接口白名单."""
        self.interface_allowlist.append(name)
        return self

    def disable_interface(self, name: str) -> "AnnounceSpec":
        """向自动地址解析规则追加接口黑名单."""
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
    """长期注册循环的句柄.

    功能简介:
    - 由 `Client.announce()` 返回.
    - `close()` 后停止后台续租任务.

    注意事项:
    - 可作为 context manager 使用.
    """
    def __init__(self, bindings: _Bindings, handle: int) -> None:
        self._bindings = bindings
        self._handle = handle

    def close(self) -> None:
        """停止长期注册循环.

        返回值:
        - 无显式返回值.

        注意事项:
        - 重复调用是安全的.
        """
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
    """持续 watch 的句柄.

    功能简介:
    - 由 `Client.watch()` 返回.
    - `close()` 后停止后台 SSE 监听和回调分发.
    """
    def __init__(self, bindings: _Bindings, handle: int, callback_ref) -> None:
        self._bindings = bindings
        self._handle = handle
        self._callback_ref = callback_ref

    def close(self) -> None:
        """停止 watch.

        返回值:
        - 无显式返回值.
        """
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
    """Python 高层 client.

    功能简介:
    - 提供 discover, announce, watch 三类核心能力.
    - 内部通过 `ctypes` 调用 `liblnd`.

    参数:
    - `server_url`: server base URL.
    - `bearer_token`: 可选 Bearer token.
    - `timeout_ms`: 可选请求超时.
    - `reconnect_backoff_ms`: `(min_ms, max_ms)` 重连退避区间.
    - `include_loopback` / `include_ipv6` / `include_private_ipv4` / `include_link_local_ipv4`:
      client 默认自动地址选择策略.
    - `library_path`: 可选动态库显式路径.

    异常:
    - 可能抛出 [`LndError`] 当动态库加载失败或底层调用失败.

    注意事项:
    - 如果不传 `library_path`, 绑定会依次尝试 `LND_LIBRARY_PATH`, 系统库查找, 仓库默认构建输出路径.
    - 可作为 context manager 使用.

    使用示例:
    ```python
    with Client("http://127.0.0.1:8765", "dev-token") as client:
        nodes = client.discover(DiscoveryFilter("office-a"))
        print(nodes)
    ```
    """
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
        """关闭 client 并释放底层句柄.

        返回值:
        - 无显式返回值.

        注意事项:
        - 重复调用是安全的.
        """
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
        """更新 server base URL.

        参数:
        - `server_url`: 新的 server base URL.
        """
        self._bindings.check_bool(
            self._bindings.lib.lnd_client_set_server_url(self._handle, _encode(server_url))
        )
        return self

    def set_bearer_token(self, bearer_token: str) -> "Client":
        """更新 Bearer token."""
        self._bindings.check_bool(
            self._bindings.lib.lnd_client_set_bearer_token(self._handle, _encode(bearer_token))
        )
        return self

    def set_timeout_ms(self, timeout_ms: int) -> "Client":
        """设置请求超时, 单位为毫秒."""
        self._bindings.check_bool(
            self._bindings.lib.lnd_client_set_timeout_ms(self._handle, timeout_ms)
        )
        return self

    def set_reconnect_backoff_ms(self, min_ms: int, max_ms: int) -> "Client":
        """设置重连退避区间.

        参数:
        - `min_ms`: 最小退避毫秒数.
        - `max_ms`: 最大退避毫秒数.
        """
        self._bindings.check_bool(
            self._bindings.lib.lnd_client_set_reconnect_backoff_ms(
                self._handle,
                min_ms,
                max_ms,
            )
        )
        return self

    def set_include_loopback(self, on: bool) -> "Client":
        """设置默认自动地址选择是否允许 loopback."""
        self._bindings.check_bool(
            self._bindings.lib.lnd_client_set_include_loopback(self._handle, on)
        )
        return self

    def set_include_ipv6(self, on: bool) -> "Client":
        """设置默认自动地址选择是否允许 IPv6."""
        self._bindings.check_bool(
            self._bindings.lib.lnd_client_set_include_ipv6(self._handle, on)
        )
        return self

    def set_include_private_ipv4(self, on: bool) -> "Client":
        """设置默认自动地址选择是否允许私网 IPv4."""
        self._bindings.check_bool(
            self._bindings.lib.lnd_client_set_include_private_ipv4(self._handle, on)
        )
        return self

    def set_include_link_local_ipv4(self, on: bool) -> "Client":
        """设置默认自动地址选择是否允许链路本地 IPv4."""
        self._bindings.check_bool(
            self._bindings.lib.lnd_client_set_include_link_local_ipv4(self._handle, on)
        )
        return self

    def enable_interface(self, interface_name: str) -> "Client":
        """向默认自动地址选择追加接口白名单."""
        self._bindings.check_bool(
            self._bindings.lib.lnd_client_enable_interface(self._handle, _encode(interface_name))
        )
        return self

    def disable_interface(self, interface_name: str) -> "Client":
        """向默认自动地址选择追加接口黑名单."""
        self._bindings.check_bool(
            self._bindings.lib.lnd_client_disable_interface(self._handle, _encode(interface_name))
        )
        return self

    def clear_interface_filters(self) -> "Client":
        """清空默认自动地址选择中的接口白名单和黑名单."""
        self._bindings.check_bool(
            self._bindings.lib.lnd_client_clear_interface_filters(self._handle)
        )
        return self

    def discover(self, filter_spec: DiscoveryFilter) -> list[dict]:
        """执行一次性发现.

        参数:
        - `filter_spec`: 查询过滤器.

        返回值:
        - 节点 JSON 对象列表, 每项形态与 Rust `DiscoveredNode` 序列化结果一致.

        异常:
        - 抛出 [`LndError`] 当请求失败或响应解析失败.
        """
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
        """解析最终上报地址列表.

        参数:
        - `spec`: 注册规格.

        返回值:
        - 去重后的地址字符串列表.
        """
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
        """执行一次注册.

        参数:
        - `spec`: 注册规格.

        返回值:
        - 单个节点 JSON 对象, 与 Rust `DiscoveredNode` 序列化结果一致.
        """
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
        """启动长期注册循环.

        参数:
        - `spec`: 注册规格.

        返回值:
        - 一个 [`AnnounceHandle`].

        注意事项:
        - 底层会持续续租, 直到 `handle.close()`.
        """
        spec_handle = spec._into_handle(self._bindings)
        try:
            handle = self._bindings.check_ptr(
                self._bindings.lib.lnd_announce_start_with_spec(self._handle, spec_handle)
            )
            return AnnounceHandle(self._bindings, handle)
        finally:
            self._bindings.lib.lnd_announce_spec_free(spec_handle)

    def watch(self, filter_spec: DiscoveryFilter, callback: WatchCallback) -> WatchHandle:
        """启动持续 watch.

        参数:
        - `filter_spec`: 事件过滤器.
        - `callback`: 每次收到事件时调用, 参数为解析后的 JSON `dict`.

        返回值:
        - 一个 [`WatchHandle`].

        异常:
        - 抛出 [`LndError`] 当底层 watch 无法启动.

        注意事项:
        - 回调在线程中被触发, 应避免阻塞过久.
        - 事件对象形态与 Rust `DiscoveryEventEnvelope` 序列化结果一致.
        """
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
