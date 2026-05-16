import json
from collections.abc import Callable
from dataclasses import asdict, dataclass, field
from typing import Any

from . import _native


WatchCallback = Callable[[dict], None]


class LndError(RuntimeError):
    """`lnd` Python SDK 的统一异常类型.

    功能简介:
    - 表示 `lnd._native` 扩展或远端 server 返回失败.

    常见触发场景:
    - 请求参数 JSON 无法解析.
    - server 返回业务错误.
    - watch 或 announce 后台任务启动失败.
    """


def _filter_json(filter_spec: "DiscoveryFilter") -> str:
    return json.dumps(asdict(filter_spec))


def _spec_json(spec: "AnnounceSpec") -> str:
    return json.dumps(asdict(spec))


def _call_native(name: str, func: Callable[..., Any], *args: Any) -> Any:
    try:
        return func(*args)
    except Exception as error:
        raise LndError(f"{name} failed: {error}") from error


@dataclass
class DiscoveryFilter:
    """一次性发现和持续 watch 使用的过滤器.

    参数:
    - `network_id`: 可选逻辑发现域.
    - `service`: 可选服务名过滤器, 推荐使用 mDNS / DNS-SD 风格的 service type.
    - `tags`: 需要全部满足的 tag 列表.
    - `reachability_scopes`: 至少重叠一个即可匹配.

    返回值:
    - 该类的方法都会返回 `self`, 便于链式调用.

    使用示例:
    ```python
    filter_spec = DiscoveryFilter().with_network_id("office-a").with_service("_http._tcp")
    ```
    """

    network_id: str | None = None
    service: str | None = None
    tags: list[str] = field(default_factory=list)
    reachability_scopes: list[str] = field(default_factory=list)

    def with_network_id(self, network_id: str | None) -> "DiscoveryFilter":
        """设置逻辑发现域过滤条件."""

        self.network_id = network_id
        return self

    def with_service(self, service: str) -> "DiscoveryFilter":
        """设置服务名过滤条件.

        参数:
        - `service`: 例如 `_http._tcp`, 形式上接近 mDNS / DNS-SD 常见的 service type.
        """

        self.service = service
        return self

    def add_tag(self, tag: str) -> "DiscoveryFilter":
        """追加一个 tag 过滤条件."""

        self.tags.append(tag)
        return self

    def add_reachability_scope(self, scope: str) -> "DiscoveryFilter":
        """追加一个可达域过滤条件."""

        self.reachability_scopes.append(scope)
        return self


@dataclass
class AnnounceSpec:
    """面向 Python 调用方的注册规格.

    功能简介:
    - 描述一个节点如何向 `lnd-server` 注册自己.
    - 同时支持显式 `lan_addrs` 和自动地址解析参数.

    参数:
    - `network_id`: 可选逻辑发现域.
    - `node_id`: 持久节点标识.
    - `service`: 服务名, 建议使用 mDNS / DNS-SD 常见的 service type.
    - `display_name`: 展示名称.
    - `port`: 服务端口.

    注意事项:
    - `auto_lan_addrs=False` 且 `lan_addrs` 为空时, 最终可能上报空地址列表.
    - `include_*` 和接口过滤只影响自动地址解析.
    """

    network_id: str | None = None
    node_id: str
    service: str
    display_name: str
    port: int
    auto_lan_addrs: bool = True
    auto_reachability_scopes: bool = True
    ttl_secs: int = 30
    lan_addrs: list[str] = field(default_factory=list)
    reachability_scopes: list[str] = field(default_factory=list)
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

    def add_reachability_scope(self, scope: str) -> "AnnounceSpec":
        """追加一个显式可达域."""

        self.reachability_scopes.append(scope)
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


class AnnounceHandle:
    """长期注册循环的句柄.

    功能简介:
    - 由 `Client.announce()` 返回.
    - `close()` 后停止后台续租任务.

    注意事项:
    - 可作为 context manager 使用.
    """

    def __init__(self, handle: object) -> None:
        self._handle = handle

    def close(self) -> None:
        """停止长期注册循环.

        返回值:
        - 无显式返回值.

        注意事项:
        - 重复调用是安全的.
        """

        handle = getattr(self, "_handle", None)
        if handle is None:
            return
        _call_native("announce_stop", handle.close)
        self._handle = None

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

    def __init__(self, handle: object, callback_ref: WatchCallback) -> None:
        self._handle = handle
        self._callback_ref = callback_ref

    def close(self) -> None:
        """停止 watch.

        返回值:
        - 无显式返回值.
        """

        handle = getattr(self, "_handle", None)
        if handle is None:
            return
        _call_native("watch_stop", handle.close)
        self._handle = None
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
    - 内部通过原生扩展 `lnd._native` 调用 Rust client.

    参数:
    - `server_url`: server base URL.
    - `bearer_token`: 可选 Bearer token.
    - `timeout_ms`: 可选请求超时.
    - `reconnect_backoff_ms`: `(min_ms, max_ms)` 重连退避区间.
    - `include_loopback` / `include_ipv6` / `include_private_ipv4` / `include_link_local_ipv4`:
      client 默认自动地址选择策略.

    异常:
    - 可能抛出 [`LndError`] 当原生扩展调用失败或 server 返回错误.

    注意事项:
    - wheel 必须包含 `lnd._native`.
    - 可作为 context manager 使用.

    使用示例:
    ```python
    with Client("http://127.0.0.1:8765", "dev-token") as client:
        nodes = client.discover(DiscoveryFilter().with_network_id("office-a"))
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
    ) -> None:
        self._server_url = server_url
        self._bearer_token = bearer_token
        self._timeout_ms = timeout_ms if timeout_ms is not None else 10_000
        self._reconnect_backoff_ms = reconnect_backoff_ms if reconnect_backoff_ms is not None else (
            500,
            15_000,
        )
        self._include_loopback = include_loopback
        self._include_ipv6 = include_ipv6
        self._include_private_ipv4 = include_private_ipv4
        self._include_link_local_ipv4 = include_link_local_ipv4
        self._interface_allowlist: list[str] = []
        self._interface_denylist: list[str] = []

    def close(self) -> None:
        """关闭 client.

        返回值:
        - 无显式返回值.

        注意事项:
        - 当前 Python client 不持有独立的底层连接句柄.
        - 重复调用是安全的.
        """

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

        self._server_url = server_url
        return self

    def set_bearer_token(self, bearer_token: str) -> "Client":
        """更新 Bearer token."""

        self._bearer_token = bearer_token
        return self

    def set_timeout_ms(self, timeout_ms: int) -> "Client":
        """设置请求超时, 单位为毫秒."""

        self._timeout_ms = timeout_ms
        return self

    def set_reconnect_backoff_ms(self, min_ms: int, max_ms: int) -> "Client":
        """设置重连退避区间.

        参数:
        - `min_ms`: 最小退避毫秒数.
        - `max_ms`: 最大退避毫秒数.
        """

        self._reconnect_backoff_ms = (min_ms, max_ms)
        return self

    def set_include_loopback(self, on: bool) -> "Client":
        """设置默认自动地址选择是否允许 loopback."""

        self._include_loopback = on
        return self

    def set_include_ipv6(self, on: bool) -> "Client":
        """设置默认自动地址选择是否允许 IPv6."""

        self._include_ipv6 = on
        return self

    def set_include_private_ipv4(self, on: bool) -> "Client":
        """设置默认自动地址选择是否允许私网 IPv4."""

        self._include_private_ipv4 = on
        return self

    def set_include_link_local_ipv4(self, on: bool) -> "Client":
        """设置默认自动地址选择是否允许链路本地 IPv4."""

        self._include_link_local_ipv4 = on
        return self

    def enable_interface(self, interface_name: str) -> "Client":
        """向默认自动地址选择追加接口白名单."""

        self._interface_allowlist.append(interface_name)
        return self

    def disable_interface(self, interface_name: str) -> "Client":
        """向默认自动地址选择追加接口黑名单."""

        self._interface_denylist.append(interface_name)
        return self

    def clear_interface_filters(self) -> "Client":
        """清空默认自动地址选择中的接口白名单和黑名单."""

        self._interface_allowlist.clear()
        self._interface_denylist.clear()
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

        result = _call_native(
            "discover",
            _native.discover_json,
            self._server_url,
            self._bearer_token,
            _filter_json(filter_spec),
            self._timeout_ms,
            self._reconnect_backoff_ms,
            self._address_defaults_json(),
        )
        assert isinstance(result, list)
        return result

    def discover_with_auto_scope_overlap(self, filter_spec: DiscoveryFilter) -> list[dict]:
        """使用本机自动可达域重叠策略执行一次发现."""

        filter_spec = DiscoveryFilter(
            network_id=filter_spec.network_id,
            service=filter_spec.service,
            tags=list(filter_spec.tags),
            reachability_scopes=list(filter_spec.reachability_scopes),
        )
        for scope in self.list_reachability_scopes():
            filter_spec.add_reachability_scope(scope["scope"])
        return self.discover(filter_spec)

    def resolve_network_id(self) -> str:
        """自动推导一个局域网发现域标识.

        返回值:
        - 一个稳定的 `network_id` 字符串.

        异常:
        - 抛出 [`LndError`] 当没有可用局域网前缀, 或候选过多无法自动选定.
        """

        result = _call_native(
            "resolve_network_id",
            _native.resolve_network_id,
            self._server_url,
            self._bearer_token,
            self._timeout_ms,
            self._reconnect_backoff_ms,
            self._address_defaults_json(),
        )
        assert isinstance(result, str)
        return result

    def list_network_id_candidates(self) -> list[dict]:
        """列出当前默认地址选择规则下的全部 `network_id` 候选项.

        返回值:
        - 每项都包含 `network_id` 和 `scope`.
        """

        result = _call_native(
            "list_network_id_candidates",
            _native.list_network_id_candidates_json,
            self._server_url,
            self._bearer_token,
            self._timeout_ms,
            self._reconnect_backoff_ms,
            self._address_defaults_json(),
        )
        assert isinstance(result, list)
        return result

    def list_reachability_scopes(self) -> list[dict]:
        """列出当前默认地址选择规则下的全部可达域候选项."""

        result = _call_native(
            "list_reachability_scopes",
            _native.list_reachability_scopes_json,
            self._server_url,
            self._bearer_token,
            self._timeout_ms,
            self._reconnect_backoff_ms,
            self._address_defaults_json(),
        )
        assert isinstance(result, list)
        return result

    def resolve_announce_addrs(self, spec: AnnounceSpec) -> list[str]:
        """解析最终上报地址列表.

        参数:
        - `spec`: 注册规格.

        返回值:
        - 去重后的地址字符串列表.
        """

        result = _call_native(
            "resolve_announce_addrs",
            _native.resolve_announce_addrs_json,
            self._server_url,
            self._bearer_token,
            _spec_json(spec),
            self._timeout_ms,
            self._reconnect_backoff_ms,
            self._address_defaults_json(),
        )
        assert isinstance(result, list)
        return result

    def announce_once(self, spec: AnnounceSpec) -> dict:
        """执行一次注册.

        参数:
        - `spec`: 注册规格.

        返回值:
        - 单个节点 JSON 对象, 与 Rust `DiscoveredNode` 序列化结果一致.
        """

        result = _call_native(
            "announce_once",
            _native.announce_once_json,
            self._server_url,
            self._bearer_token,
            _spec_json(spec),
            self._timeout_ms,
            self._reconnect_backoff_ms,
            self._address_defaults_json(),
        )
        assert isinstance(result, dict)
        return result

    def announce(self, spec: AnnounceSpec) -> AnnounceHandle:
        """启动长期注册循环.

        参数:
        - `spec`: 注册规格.

        返回值:
        - 一个 [`AnnounceHandle`].

        注意事项:
        - 底层会持续续租, 直到 `handle.close()`.
        """

        handle = _call_native(
            "announce_start",
            _native.announce_start,
            self._server_url,
            self._bearer_token,
            _spec_json(spec),
            self._timeout_ms,
            self._reconnect_backoff_ms,
            self._address_defaults_json(),
        )
        return AnnounceHandle(handle)

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

        handle = _call_native(
            "watch_start",
            _native.watch_start,
            self._server_url,
            self._bearer_token,
            _filter_json(filter_spec),
            callback,
            self._timeout_ms,
            self._reconnect_backoff_ms,
            self._address_defaults_json(),
        )
        return WatchHandle(handle, callback)

    def watch_with_auto_scope_overlap(
        self,
        filter_spec: DiscoveryFilter,
        callback: WatchCallback,
    ) -> WatchHandle:
        """使用本机自动可达域重叠策略启动持续 watch."""

        filter_spec = DiscoveryFilter(
            network_id=filter_spec.network_id,
            service=filter_spec.service,
            tags=list(filter_spec.tags),
            reachability_scopes=list(filter_spec.reachability_scopes),
        )
        for scope in self.list_reachability_scopes():
            filter_spec.add_reachability_scope(scope["scope"])
        return self.watch(filter_spec, callback)

    def _address_defaults(self) -> dict[str, object]:
        return {
            "include_loopback": self._include_loopback,
            "include_ipv6": self._include_ipv6,
            "include_private_ipv4": self._include_private_ipv4,
            "include_link_local_ipv4": self._include_link_local_ipv4,
            "interface_allowlist": list(self._interface_allowlist),
            "interface_denylist": list(self._interface_denylist),
        }

    def _address_defaults_json(self) -> str:
        return json.dumps(self._address_defaults())
