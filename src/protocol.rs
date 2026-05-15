use std::collections::BTreeMap;
use std::net::{IpAddr, SocketAddr};

use serde::{Deserialize, Serialize};

/// 默认租约 TTL, 单位为秒.
///
/// v1 中 client 和 server 的默认值都会使用这个常量.
pub const DEFAULT_TTL_SECS: u64 = 30;
/// 默认续租间隔, 单位为秒.
///
/// 该值对应默认 TTL `30s` 的 `ttl / 3`.
pub const DEFAULT_RENEW_INTERVAL_SECS: u64 = 10;
/// 默认 SSE keepalive 间隔, 单位为秒.
pub const DEFAULT_SSE_KEEPALIVE_SECS: u64 = 15;
/// 默认事件缓冲区容量.
pub const DEFAULT_EVENT_BUFFER_CAPACITY: usize = 4096;

/// 已经完成地址解析, 可直接提交到 server 的节点公告模型.
///
/// 功能简介:
/// - 表示 `PUT /v1/nodes/{node_id}` 的请求体.
/// - 与 [`AnnounceSpec`] 的区别在于这里的 `lan_addrs` 已经是最终地址集合,
///   不再包含自动选址参数.
///
/// 注意事项:
/// - `node_id` 必须与 URL path 中的 `node_id` 一致.
/// - `lan_addrs` 应当已经去重.
/// - `ttl_secs = 0` 会被 server 视为无效输入或重置为默认值, 调用方不应依赖该行为.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeAnnouncement {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_id: Option<String>,
    pub node_id: String,
    pub service: String,
    pub display_name: String,
    pub port: u16,
    pub lan_addrs: Vec<SocketAddr>,
    #[serde(default)]
    pub reachability_scopes: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
    #[serde(default = "default_ttl_secs")]
    pub ttl_secs: u64,
}

/// 发现结果中的节点信息.
///
/// 功能简介:
/// - 由 `GET /v1/nodes` 返回.
/// - 也用于 `watch` 的 `snapshot`, `upsert`, `remove` 事件.
///
/// 与 [`NodeAnnouncement`] 的差别:
/// - 增加了 [`LeaseInfo`] 字段, 用于表达 revision 和过期时间.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveredNode {
    pub network_id: Option<String>,
    pub node_id: String,
    pub service: String,
    pub display_name: String,
    pub port: u16,
    pub lan_addrs: Vec<SocketAddr>,
    pub reachability_scopes: Vec<String>,
    pub tags: Vec<String>,
    pub metadata: BTreeMap<String, String>,
    pub lease: LeaseInfo,
}

/// 节点租约元数据.
///
/// 字段说明:
/// - `revision`: server 分配的递增事件游标.
/// - `ttl_secs`: 当前租约 TTL.
/// - `expires_at_unix_ms`: 该节点若不续租, 将在该时刻后被自动摘除.
/// - `last_seen_unix_ms`: 最近一次上报时间.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeaseInfo {
    pub revision: u64,
    pub ttl_secs: u64,
    pub expires_at_unix_ms: u64,
    pub last_seen_unix_ms: u64,
}

/// 节点发现过滤器.
///
/// 功能简介:
/// - 用于一次性查询和持续 watch.
/// - `network_id` 可选, 用于逻辑发现域隔离.
/// - `service` 为可选单值过滤条件.
/// - `tags` 为 "全部满足" 语义, 即返回节点必须包含所有给定 tag.
/// - `reachability_scopes` 为 "至少有一个重叠" 语义.
///
/// 使用示例:
/// ```rust
/// use lnd::DiscoveryFilter;
///
/// let filter = DiscoveryFilter::new()
///     .with_network_id("office-a")
///     .with_service("_demo._tcp")
///     .add_tag("stable");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveryFilter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_id: Option<String>,
    #[serde(default)]
    pub service: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub reachability_scopes: Vec<String>,
}

impl DiscoveryFilter {
    /// 创建一个最小发现过滤器.
    ///
    /// 返回值:
    /// - 一个没有 `network_id`, `service`, `tags` 和 `reachability_scopes` 过滤条件的 [`DiscoveryFilter`].
    pub fn new() -> Self {
        Self {
            network_id: None,
            service: None,
            tags: Vec::new(),
            reachability_scopes: Vec::new(),
        }
    }

    /// 设置逻辑发现域过滤条件.
    pub fn with_network_id(mut self, network_id: impl Into<String>) -> Self {
        self.network_id = Some(network_id.into());
        self
    }

    /// 清空逻辑发现域过滤条件.
    pub fn without_network_id(mut self) -> Self {
        self.network_id = None;
        self
    }

    /// 设置服务名过滤条件.
    ///
    /// 参数:
    /// - `service`: 例如 `_demo._tcp`.
    ///
    /// 返回值:
    /// - 更新后的过滤器, 便于链式调用.
    pub fn with_service(mut self, service: impl Into<String>) -> Self {
        self.service = Some(service.into());
        self
    }

    /// 使用一组 tag 替换当前 tag 过滤条件.
    ///
    /// 参数:
    /// - `tags`: 需要全部满足的 tag 集合.
    ///
    /// 返回值:
    /// - 更新后的过滤器.
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    /// 追加一个 tag 过滤条件.
    ///
    /// 参数:
    /// - `tag`: 需要匹配的 tag.
    ///
    /// 返回值:
    /// - 更新后的过滤器.
    pub fn add_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// 用一组可达域替换当前 `reachability_scopes`.
    pub fn with_reachability_scopes(
        mut self,
        scopes: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.reachability_scopes = scopes.into_iter().map(Into::into).collect();
        self
    }

    /// 追加一个可达域过滤条件.
    pub fn add_reachability_scope(mut self, scope: impl Into<String>) -> Self {
        self.reachability_scopes.push(scope.into());
        self
    }
}

impl Default for DiscoveryFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// 一次性发现接口的 JSON 响应体.
///
/// 字段说明:
/// - `nodes`: 当前匹配的节点快照.
/// - `cursor`: 对应这份快照的游标, 可用于后续 `watch` 恢复.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoverResponse {
    pub nodes: Vec<DiscoveredNode>,
    pub cursor: u64,
}

/// SSE watch 响应体的别名结构.
///
/// v1 中它与 [`DiscoveryEventEnvelope`] 字段相同, 保留该类型是为了让接口表达更直接.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WatchResponse {
    pub cursor: Option<u64>,
    pub event: DiscoveryEvent,
}

/// 发现事件类型.
///
/// 事件说明:
/// - `Snapshot`: 首次快照, 或 `reset` 后的全量重同步结果.
/// - `Upsert`: 某节点上线或续租后的最新状态.
/// - `Remove`: 某节点过期或被删除.
/// - `Reset`: 当前 cursor 无法继续恢复, 调用方应重新拉取全量快照.
/// - `Keepalive`: 保留事件类型, 便于未来扩展或统一事件处理逻辑.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DiscoveryEvent {
    Snapshot { nodes: Vec<DiscoveredNode> },
    Upsert { node: DiscoveredNode },
    Remove { node: DiscoveredNode },
    Reset,
    Keepalive,
}

/// 带 cursor 的发现事件封装.
///
/// 注意事项:
/// - `cursor = None` 通常意味着这是无法恢复 replay 时发出的 `reset`.
/// - 对 `snapshot`, `upsert`, `remove` 来说, 如果 `cursor` 存在, 调用方可以保存它用于断线恢复.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveryEventEnvelope {
    pub cursor: Option<u64>,
    pub event: DiscoveryEvent,
}

/// API 错误响应体.
///
/// 功能简介:
/// - 用于 REST 接口非成功响应时的 JSON 结构.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApiErrorBody {
    pub error: String,
}

/// 面向 client 的注册规格.
///
/// 功能简介:
/// - 这是 Rust API 和高层绑定中最常见的 "声明式注册参数".
/// - 调用方可在这里同时配置显式 `lan_addrs`, 自动地址收集和地址选择规则.
/// - 实际发往 server 前通常会先转换成 [`NodeAnnouncement`].
///
/// 注意事项:
/// - `lan_addrs` 为 `None` 或空时, 是否自动收集地址取决于 `auto_lan_addrs`.
/// - `address_selection` 为空时, client 会使用默认地址选择策略.
///
/// 使用示例:
/// ```rust
/// use lnd::AnnounceSpec;
///
/// let spec = AnnounceSpec::new("node-a", "_demo._tcp", "devbox-a", 8080)
///     .with_network_id("office-a")
///     .add_tag("stable")
///     .insert_metadata("version", "1.0.0")
///     .include_loopback(true);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnnounceSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_id: Option<String>,
    pub node_id: String,
    pub service: String,
    pub display_name: String,
    pub port: u16,
    #[serde(default)]
    pub lan_addrs: Option<Vec<SocketAddr>>,
    #[serde(default = "default_true")]
    pub auto_lan_addrs: bool,
    #[serde(default)]
    pub address_selection: Option<AddressSelection>,
    #[serde(default)]
    pub reachability_scopes: Option<Vec<String>>,
    #[serde(default = "default_true")]
    pub auto_reachability_scopes: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
    #[serde(default = "default_ttl_secs")]
    pub ttl_secs: u64,
}

impl AnnounceSpec {
    /// 创建一个最小可用的注册规格.
    ///
    /// 参数:
    /// - `node_id`: 节点持久标识.
    /// - `service`: 服务名, 例如 `_demo._tcp`.
    /// - `display_name`: 面向人类展示的名称.
    /// - `port`: 服务监听端口.
    ///
    /// 返回值:
    /// - 默认启用 `auto_lan_addrs`, 默认 TTL 为 [`DEFAULT_TTL_SECS`] 的 [`AnnounceSpec`].
    pub fn new(
        node_id: impl Into<String>,
        service: impl Into<String>,
        display_name: impl Into<String>,
        port: u16,
    ) -> Self {
        Self {
            network_id: None,
            node_id: node_id.into(),
            service: service.into(),
            display_name: display_name.into(),
            port,
            lan_addrs: None,
            auto_lan_addrs: true,
            address_selection: None,
            reachability_scopes: None,
            auto_reachability_scopes: true,
            tags: Vec::new(),
            metadata: BTreeMap::new(),
            ttl_secs: default_ttl_secs(),
        }
    }

    /// 将当前规格转换为最终公告模型.
    ///
    /// 参数:
    /// - `lan_addrs`: 已经完成解析和去重的最终地址集合.
    ///
    /// 返回值:
    /// - 可直接提交给 server 的 [`NodeAnnouncement`].
    ///
    /// 注意事项:
    /// - 本方法不会自行解析地址.
    /// - 调用方通常应先调用 `resolve_announce_addrs*` 相关函数.
    pub fn into_announcement(self, lan_addrs: Vec<SocketAddr>) -> NodeAnnouncement {
        NodeAnnouncement {
            network_id: self.network_id,
            node_id: self.node_id,
            service: self.service,
            display_name: self.display_name,
            port: self.port,
            lan_addrs,
            reachability_scopes: self.reachability_scopes.unwrap_or_default(),
            tags: self.tags,
            metadata: self.metadata,
            ttl_secs: self.ttl_secs,
        }
    }

    /// 设置逻辑发现域标识.
    pub fn with_network_id(mut self, network_id: impl Into<String>) -> Self {
        self.network_id = Some(network_id.into());
        self
    }

    /// 清空逻辑发现域标识.
    pub fn without_network_id(mut self) -> Self {
        self.network_id = None;
        self
    }

    /// 用一组显式地址替换当前 `lan_addrs`.
    pub fn with_lan_addrs(mut self, lan_addrs: impl IntoIterator<Item = SocketAddr>) -> Self {
        self.lan_addrs = Some(lan_addrs.into_iter().collect());
        self
    }

    /// 追加一个显式地址.
    pub fn add_lan_addr(mut self, lan_addr: SocketAddr) -> Self {
        self.lan_addrs.get_or_insert_with(Vec::new).push(lan_addr);
        self
    }

    /// 设置是否启用自动 LAN 地址收集.
    ///
    /// 注意事项:
    /// - 设为 `false` 后, 如果没有显式 `lan_addrs`, 最终可能上报空地址列表.
    pub fn with_auto_lan_addrs(mut self, auto_lan_addrs: bool) -> Self {
        self.auto_lan_addrs = auto_lan_addrs;
        self
    }

    /// 覆盖地址选择策略.
    pub fn with_address_selection(mut self, address_selection: AddressSelection) -> Self {
        self.address_selection = Some(address_selection);
        self
    }

    /// 用一组显式可达域替换当前 `reachability_scopes`.
    pub fn with_reachability_scopes(
        mut self,
        scopes: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.reachability_scopes = Some(scopes.into_iter().map(Into::into).collect());
        self
    }

    /// 追加一个显式可达域.
    pub fn add_reachability_scope(mut self, scope: impl Into<String>) -> Self {
        self.reachability_scopes
            .get_or_insert_with(Vec::new)
            .push(scope.into());
        self
    }

    /// 设置是否启用自动可达域收集.
    pub fn with_auto_reachability_scopes(mut self, on: bool) -> Self {
        self.auto_reachability_scopes = on;
        self
    }

    /// 用一组 tag 替换当前 tag 列表.
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    /// 追加一个 tag.
    pub fn add_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// 用一组键值对替换 metadata.
    pub fn with_metadata(
        mut self,
        metadata: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        self.metadata = metadata
            .into_iter()
            .map(|(key, value)| (key.into(), value.into()))
            .collect();
        self
    }

    /// 插入一个 metadata 键值对.
    pub fn insert_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// 设置租约 TTL, 单位为秒.
    pub fn with_ttl_secs(mut self, ttl_secs: u64) -> Self {
        self.ttl_secs = ttl_secs;
        self
    }

    /// 设置是否允许自动地址选择包含 loopback.
    pub fn include_loopback(mut self, include_loopback: bool) -> Self {
        self.address_selection
            .get_or_insert_with(AddressSelection::default)
            .include_loopback = include_loopback;
        self
    }

    /// 设置是否允许自动地址选择包含 IPv6.
    pub fn include_ipv6(mut self, include_ipv6: bool) -> Self {
        self.address_selection
            .get_or_insert_with(AddressSelection::default)
            .include_ipv6 = include_ipv6;
        self
    }

    /// 向自动地址选择规则中追加接口白名单.
    pub fn with_interface(mut self, interface_name: impl Into<String>) -> Self {
        self.address_selection
            .get_or_insert_with(AddressSelection::default)
            .interface_allowlist
            .push(interface_name.into());
        self
    }

    /// 向自动地址选择规则中追加接口黑名单.
    pub fn without_interface(mut self, interface_name: impl Into<String>) -> Self {
        self.address_selection
            .get_or_insert_with(AddressSelection::default)
            .interface_denylist
            .push(interface_name.into());
        self
    }
}

/// 自动地址选择规则.
///
/// 功能简介:
/// - 控制 client 自动收集哪些本机地址.
/// - 既可作为 client 默认值使用, 也可在单个 [`AnnounceSpec`] 上覆写.
///
/// 注意事项:
/// - 默认只包含私网 IPv4.
/// - `interface_allowlist` 非空时, 只有白名单接口会被考虑.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AddressSelection {
    #[serde(default = "default_true")]
    pub include_private_ipv4: bool,
    #[serde(default)]
    pub include_loopback: bool,
    #[serde(default)]
    pub include_link_local_ipv4: bool,
    #[serde(default)]
    pub include_ipv6: bool,
    #[serde(default)]
    pub interface_allowlist: Vec<String>,
    #[serde(default)]
    pub interface_denylist: Vec<String>,
}

impl AddressSelection {
    /// 创建默认地址选择规则.
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置是否允许 loopback.
    pub fn with_loopback(mut self, include_loopback: bool) -> Self {
        self.include_loopback = include_loopback;
        self
    }

    /// 设置是否允许私网 IPv4.
    pub fn with_private_ipv4(mut self, include_private_ipv4: bool) -> Self {
        self.include_private_ipv4 = include_private_ipv4;
        self
    }

    /// 设置是否允许链路本地 IPv4.
    pub fn with_link_local_ipv4(mut self, include_link_local_ipv4: bool) -> Self {
        self.include_link_local_ipv4 = include_link_local_ipv4;
        self
    }

    /// 设置是否允许 IPv6.
    pub fn with_ipv6(mut self, include_ipv6: bool) -> Self {
        self.include_ipv6 = include_ipv6;
        self
    }

    /// 追加一个接口白名单项.
    pub fn with_interface(mut self, interface_name: impl Into<String>) -> Self {
        self.interface_allowlist.push(interface_name.into());
        self
    }

    /// 追加一个接口黑名单项.
    pub fn without_interface(mut self, interface_name: impl Into<String>) -> Self {
        self.interface_denylist.push(interface_name.into());
        self
    }

    /// 判断某个接口名是否允许被自动地址选择使用.
    ///
    /// 参数:
    /// - `interface_name`: 例如 `en0`, `eth0`.
    ///
    /// 返回值:
    /// - `true` 表示通过接口层面的白名单/黑名单校验.
    pub fn allows_interface(&self, interface_name: &str) -> bool {
        let allowed = self.interface_allowlist.is_empty()
            || self
                .interface_allowlist
                .iter()
                .any(|value| value == interface_name);
        let denied = self
            .interface_denylist
            .iter()
            .any(|value| value == interface_name);
        allowed && !denied
    }

    /// 判断某个 IP 是否允许被自动地址选择使用.
    ///
    /// 参数:
    /// - `ip`: 待判断 IP.
    /// - `is_loopback`: 该地址所属接口是否为 loopback.
    ///
    /// 返回值:
    /// - `true` 表示该地址可被收集.
    pub fn allows_ip(&self, ip: IpAddr, is_loopback: bool) -> bool {
        match ip {
            IpAddr::V4(ipv4) => {
                if is_loopback {
                    return self.include_loopback;
                }
                ipv4.is_private() && self.include_private_ipv4
                    || ipv4.is_link_local() && self.include_link_local_ipv4
            }
            IpAddr::V6(ipv6) => {
                if ipv6.is_loopback() {
                    return self.include_loopback;
                }
                self.include_ipv6 && !ipv6.is_unspecified()
            }
        }
    }
}

impl Default for AddressSelection {
    fn default() -> Self {
        Self {
            include_private_ipv4: true,
            include_loopback: false,
            include_link_local_ipv4: false,
            include_ipv6: false,
            interface_allowlist: Vec::new(),
            interface_denylist: Vec::new(),
        }
    }
}

pub fn default_ttl_secs() -> u64 {
    DEFAULT_TTL_SECS
}

/// serde 默认值辅助函数.
pub fn default_true() -> bool {
    true
}
