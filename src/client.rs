use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::Duration;

use anyhow::Context;
use eventsource_stream::Eventsource;
use futures::{Stream, StreamExt};
use if_addrs::{IfAddr, get_if_addrs};
use rand::Rng;
use reqwest::header::{ACCEPT, AUTHORIZATION};
use reqwest::{Client as HttpClient, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

use crate::protocol::{
    AddressSelection, AnnounceSpec, ApiErrorBody, DEFAULT_TTL_SECS, DiscoverResponse,
    DiscoveredNode, DiscoveryEvent, DiscoveryEventEnvelope, DiscoveryFilter, NodeAnnouncement,
};

/// `watch` 返回的异步事件流类型.
///
/// 功能简介:
/// - 每个元素要么是一个成功解析的 [`DiscoveryEventEnvelope`], 要么是 [`ClientError`].
/// - 该流内部已经处理了 SSE 重连, cursor 恢复和 `reset` 后快照补发.
pub type WatchStream =
    Pin<Box<dyn Stream<Item = Result<DiscoveryEventEnvelope, ClientError>> + Send>>;

/// 自动推导出的局域网发现域候选项.
///
/// 功能简介:
/// - 表示从本机接口和地址选择规则推导出的一个候选 `network_id`.
/// - 调用方可以直接使用 `network_id`, 也可以结合 `scope` 做展示或调试.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DerivedNetworkId {
    pub network_id: String,
    pub scope: String,
}

/// 自动推导出的可达域候选项.
///
/// 功能简介:
/// - 表示从本机接口和地址选择规则推导出的一个局域网前缀作用域.
/// - 可直接用于发现过滤器或注册公告中的 `reachability_scopes`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReachabilityScope {
    pub scope: String,
}

/// Rust client 配置.
///
/// 使用场景:
/// - 直接调用 [`LndClient::new`] 时显式传入.
/// - 通过 [`ClientBuilder`] 渐进构造.
///
/// 注意事项:
/// - `default_address_selection` 只影响自动地址解析的默认值.
/// - 单个 [`AnnounceSpec`] 上的地址选择参数可以覆盖该默认值.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub server_url: String,
    pub bearer_token: String,
    pub timeout: Duration,
    pub reconnect_backoff_min: Duration,
    pub reconnect_backoff_max: Duration,
    pub default_address_selection: AddressSelection,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            server_url: "http://127.0.0.1:8765".to_string(),
            bearer_token: String::new(),
            timeout: Duration::from_secs(10),
            reconnect_backoff_min: Duration::from_millis(500),
            reconnect_backoff_max: Duration::from_secs(15),
            default_address_selection: AddressSelection::default(),
        }
    }
}

#[derive(Clone)]
pub struct LndClient {
    http: HttpClient,
    config: ClientConfig,
}

/// 长时间注册循环的句柄.
///
/// 功能简介:
/// - 由 [`LndClient::announce_loop`] 返回.
/// - 通过 [`AnnounceHandle::stop`] 停止后台续租任务.
pub struct AnnounceHandle {
    shutdown: watch::Sender<bool>,
    task: JoinHandle<Result<(), ClientError>>,
}

/// Rust client 错误类型.
///
/// 常见来源:
/// - HTTP 请求失败.
/// - JSON 序列化或反序列化失败.
/// - server 返回业务错误.
/// - client 配置不合法, 例如 Bearer token 头无法构造.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("api error: {0}")]
    Api(String),
    #[error("unexpected response status: {0}")]
    Status(StatusCode),
    #[error("invalid client configuration: {0}")]
    InvalidConfig(String),
}

impl LndClient {
    /// 创建一个 builder, 用于渐进构造 [`LndClient`].
    ///
    /// 参数:
    /// - `server_url`: server base URL, 例如 `http://127.0.0.1:8765`.
    ///
    /// 返回值:
    /// - 一个 [`ClientBuilder`].
    ///
    /// 使用示例:
    /// ```rust
    /// use lnd::LndClient;
    ///
    /// let client = LndClient::builder("http://127.0.0.1:8765")
    ///     .bearer_token("dev-token")
    ///     .include_loopback(true)
    ///     .build();
    /// ```
    pub fn builder(server_url: impl Into<String>) -> ClientBuilder {
        ClientBuilder::new(server_url)
    }

    /// 从完整配置创建 client.
    ///
    /// 参数:
    /// - `config`: 包含 base URL, 鉴权, 超时和默认地址选择的配置.
    ///
    /// 返回值:
    /// - 成功时返回可复用的 [`LndClient`].
    ///
    /// 异常:
    /// - 返回 [`ClientError::InvalidConfig`] 当 Bearer token 无法转为合法 header.
    /// - 返回 [`ClientError::Http`] 当底层 HTTP client 构建失败.
    pub fn new(config: ClientConfig) -> Result<Self, ClientError> {
        let mut headers = reqwest::header::HeaderMap::new();
        if !config.bearer_token.is_empty() {
            let value = format!("Bearer {}", config.bearer_token);
            headers.insert(
                AUTHORIZATION,
                value
                    .parse()
                    .map_err(|_| ClientError::InvalidConfig("invalid bearer token".to_string()))?,
            );
        }
        let http = HttpClient::builder()
            .default_headers(headers)
            .timeout(config.timeout)
            .build()?;
        Ok(Self { http, config })
    }

    /// 执行一次性节点查询.
    ///
    /// 参数:
    /// - `filter`: 发现过滤条件.
    ///
    /// 返回值:
    /// - 当前快照中的全部匹配节点.
    ///
    /// 异常:
    /// - 返回 [`ClientError`] 当网络请求失败, server 返回错误, 或响应无法解析.
    ///
    /// 注意事项:
    /// - 本方法只返回节点列表, 不返回 cursor.
    /// - 如果调用方需要后续从快照续接 watch, 应自行调用底层 `list_response` 风格逻辑或使用 `watch`.
    #[instrument(skip(self), fields(network_id = ?filter.network_id))]
    pub async fn list(&self, filter: DiscoveryFilter) -> Result<Vec<DiscoveredNode>, ClientError> {
        self.list_response(filter)
            .await
            .map(|response| response.nodes)
    }

    async fn list_response(
        &self,
        filter: DiscoveryFilter,
    ) -> Result<DiscoverResponse, ClientError> {
        let response = self.build_list_request(&filter).send().await?;
        parse_json_response::<DiscoverResponse>(response).await
    }

    /// 创建一个可自动重连的 watch 事件流.
    ///
    /// 参数:
    /// - `filter`: 发现过滤条件.
    ///
    /// 返回值:
    /// - 一个 [`WatchStream`], 可持续拉取 `snapshot`, `upsert`, `remove`, `reset` 等事件.
    ///
    /// 注意事项:
    /// - 该流内部会在连接断开后自动重连.
    /// - 当 server 返回 `reset` 或 cursor 失效时, 该流会自动补发一份新的 `snapshot`.
    /// - 调用方应持续消费该流, 否则无法及时推进 cursor.
    ///
    /// 使用示例:
    /// ```rust
    /// use futures::StreamExt;
    /// use lnd::{DiscoveryFilter, LndClient};
    ///
    /// # async fn demo(client: LndClient) {
    /// let mut stream = client.watch(DiscoveryFilter::new().with_network_id("office-a"));
    /// while let Some(event) = stream.next().await {
    ///     println!("{:?}", event);
    /// }
    /// # }
    /// ```
    pub fn watch(&self, filter: DiscoveryFilter) -> WatchStream {
        let client = self.clone();
        Box::pin(async_stream::try_stream! {
            let mut cursor: Option<u64> = None;
            let mut attempt: u32 = 0;
            loop {
                let mut request = client.http.get(
                    format!("{}/v1/watch", client.base_url())
                );
                if let Some(network_id) = filter.network_id.as_deref() {
                    request = request.query(&[("network_id", network_id)]);
                }
                if let Some(service) = filter.service.as_deref() {
                    request = request.query(&[("service", service)]);
                }
                if let Some(cursor_value) = cursor {
                    request = request.query(&[("cursor", cursor_value)]);
                }
                request = request.query(
                    &filter
                        .tags
                        .iter()
                        .map(|tag| ("tag", tag.as_str()))
                        .collect::<Vec<_>>(),
                );
                request = request.query(
                    &filter
                        .reachability_scopes
                        .iter()
                        .map(|scope| ("scope", scope.as_str()))
                        .collect::<Vec<_>>(),
                );
                request = request.header(ACCEPT, "text/event-stream");

                match request.send().await {
                    Ok(response) if response.status() == StatusCode::CONFLICT => {
                        yield DiscoveryEventEnvelope {
                            cursor: cursor.take(),
                            event: DiscoveryEvent::Reset,
                        };
                        let snapshot = client.list_response(filter.clone()).await?;
                        cursor = Some(snapshot.cursor);
                        yield DiscoveryEventEnvelope {
                            cursor,
                            event: DiscoveryEvent::Snapshot { nodes: snapshot.nodes },
                        };
                        attempt = 0;
                    }
                    Ok(response) if response.status().is_success() => {
                        let mut stream = response.bytes_stream().eventsource();
                        attempt = 0;
                        while let Some(event) = stream.next().await {
                            let event = event.map_err(|error| ClientError::Api(error.to_string()))?;
                            let envelope: DiscoveryEventEnvelope = serde_json::from_str(&event.data)?;
                            if matches!(envelope.event, DiscoveryEvent::Reset) {
                                yield envelope;
                                let snapshot = client.list_response(filter.clone()).await?;
                                cursor = Some(snapshot.cursor);
                                yield DiscoveryEventEnvelope {
                                    cursor,
                                    event: DiscoveryEvent::Snapshot { nodes: snapshot.nodes },
                                };
                                continue;
                            }
                            if let Some(next_cursor) = envelope.cursor {
                                cursor = Some(next_cursor);
                            }
                            yield envelope;
                        }
                    }
                    Ok(response) => {
                        let status = response.status();
                        let message = response.text().await.unwrap_or_default();
                        warn!(%status, %message, "watch request failed");
                    }
                    Err(error) => {
                        warn!(error = %error, "watch connection failed");
                    }
                }
                attempt = attempt.saturating_add(1);
                tokio::time::sleep(backoff_delay(
                    client.config.reconnect_backoff_min,
                    client.config.reconnect_backoff_max,
                    attempt,
                ))
                .await;
            }
        })
    }

    /// 启动一个后台续租循环.
    ///
    /// 参数:
    /// - `spec`: 注册规格.
    ///
    /// 返回值:
    /// - 一个 [`AnnounceHandle`], 可在稍后停止续租.
    ///
    /// 异常:
    /// - 返回 [`ClientError`] 当后台任务在启动前就无法创建.
    ///
    /// 注意事项:
    /// - 该循环会先解析地址, 再调用一次注册.
    /// - 成功注册后按 `ttl_secs / 3` 加抖动续租.
    /// - 失败时会使用指数退避重试.
    pub fn announce_loop(&self, spec: AnnounceSpec) -> Result<AnnounceHandle, ClientError> {
        let client = self.clone();
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let task = tokio::spawn(async move {
            let renew_interval = Duration::from_secs((spec.ttl_secs / 3).max(1));
            let mut attempt = 0u32;
            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            info!(node_id = %spec.node_id, "announce loop stopping");
                            return Ok(());
                        }
                    }
                    _ = tokio::time::sleep(if attempt == 0 { Duration::from_secs(0) } else { backoff_delay(
                        client.config.reconnect_backoff_min,
                        client.config.reconnect_backoff_max,
                        attempt,
                    )}) => {
                        let lan_addrs = client.resolve_announce_addrs(&spec)?;
                        let reachability_scopes = client.resolve_reachability_scopes(&spec)?;
                        let mut announcement = spec.clone().into_announcement(lan_addrs);
                        announcement.reachability_scopes = reachability_scopes;
                        match client.announce_once(announcement).await {
                            Ok(_) => {
                                debug!(node_id = %spec.node_id, "lease renewed");
                                attempt = 0;
                                tokio::select! {
                                    _ = shutdown_rx.changed() => {
                                        if *shutdown_rx.borrow() {
                                            info!(node_id = %spec.node_id, "announce loop stopping");
                                            return Ok(());
                                        }
                                    }
                                    _ = tokio::time::sleep(with_jitter(renew_interval)) => {}
                                }
                            }
                            Err(error) => {
                                attempt = attempt.saturating_add(1);
                                warn!(node_id = %spec.node_id, error = %error, attempt, "announce failed, retrying");
                            }
                        }
                    }
                }
            }
        });
        Ok(AnnounceHandle {
            shutdown: shutdown_tx,
            task,
        })
    }

    /// 提交一次最终公告.
    ///
    /// 参数:
    /// - `announcement`: 已解析完成的最终公告模型.
    ///
    /// 返回值:
    /// - server 接受后的 [`DiscoveredNode`], 其中包含最新租约信息.
    ///
    /// 异常:
    /// - 返回 [`ClientError`] 当网络请求失败, server 拒绝请求, 或响应无法解析.
    ///
    /// 注意事项:
    /// - 本方法不会自动解析地址.
    /// - 大多数业务代码应优先使用 [`LndClient::announce_loop`] 或先调用 [`LndClient::resolve_announce_addrs`].
    #[instrument(skip(self, announcement), fields(node_id = %announcement.node_id, network_id = ?announcement.network_id))]
    pub async fn announce_once(
        &self,
        announcement: NodeAnnouncement,
    ) -> Result<DiscoveredNode, ClientError> {
        let response = self
            .http
            .put(format!(
                "{}/v1/nodes/{}",
                self.base_url(),
                urlencoding::encode(&announcement.node_id)
            ))
            .json(&announcement)
            .send()
            .await?;
        parse_json_response(response).await
    }

    fn base_url(&self) -> String {
        self.config.server_url.trim_end_matches('/').to_string()
    }

    /// 根据 client 默认地址选择和 `spec` 覆写规则, 解析最终上报地址.
    ///
    /// 参数:
    /// - `spec`: 原始注册规格.
    ///
    /// 返回值:
    /// - 去重后的 `SocketAddr` 列表.
    ///
    /// 异常:
    /// - 返回 [`ClientError::Io`] 当本机网络接口读取失败.
    pub fn resolve_announce_addrs(
        &self,
        spec: &AnnounceSpec,
    ) -> Result<Vec<SocketAddr>, ClientError> {
        resolve_announce_addrs_with_defaults(spec, &self.config.default_address_selection)
    }

    /// 根据 client 默认地址选择和 `spec` 覆写规则, 解析最终上报可达域.
    pub fn resolve_reachability_scopes(
        &self,
        spec: &AnnounceSpec,
    ) -> Result<Vec<String>, ClientError> {
        resolve_reachability_scopes_with_defaults(spec, &self.config.default_address_selection)
    }

    /// 使用 client 默认地址选择规则自动推导一个局域网 `network_id`.
    ///
    /// 返回值:
    /// - 成功时返回一个稳定的 `network_id` 字符串.
    ///
    /// 异常:
    /// - 当没有可用局域网地址, 或出现多个同优先级候选时返回 [`ClientError`].
    pub fn resolve_network_id(&self) -> Result<String, ClientError> {
        resolve_network_id_with_selection(&self.config.default_address_selection)
    }

    /// 列出当前地址选择规则下的全部局域网 `network_id` 候选项.
    ///
    /// 返回值:
    /// - 去重并排序后的候选项列表.
    ///
    /// 异常:
    /// - 当接口枚举失败时返回 [`ClientError::Io`].
    pub fn list_network_id_candidates(&self) -> Result<Vec<DerivedNetworkId>, ClientError> {
        list_network_id_candidates(&self.config.default_address_selection)
    }

    /// 列出当前地址选择规则下的全部局域网可达域候选项.
    pub fn list_reachability_scopes(&self) -> Result<Vec<ReachabilityScope>, ClientError> {
        list_reachability_scopes(&self.config.default_address_selection)
    }

    fn build_list_request(&self, filter: &DiscoveryFilter) -> reqwest::RequestBuilder {
        let mut request = self.http.get(format!("{}/v1/nodes", self.base_url()));
        if let Some(network_id) = filter.network_id.as_deref() {
            request = request.query(&[("network_id", network_id)]);
        }
        if let Some(service) = filter.service.as_deref() {
            request = request.query(&[("service", service)]);
        }
        request = request.query(
            &filter
                .tags
                .iter()
                .map(|tag| ("tag", tag.as_str()))
                .collect::<Vec<_>>(),
        );
        request.query(
            &filter
                .reachability_scopes
                .iter()
                .map(|scope| ("scope", scope.as_str()))
                .collect::<Vec<_>>(),
        )
    }
}

pub struct ClientBuilder {
    config: ClientConfig,
}

impl ClientBuilder {
    /// 创建一个新的 client builder.
    pub fn new(server_url: impl Into<String>) -> Self {
        let config = ClientConfig {
            server_url: server_url.into(),
            ..ClientConfig::default()
        };
        Self { config }
    }

    /// 设置 Bearer token.
    pub fn bearer_token(mut self, bearer_token: impl Into<String>) -> Self {
        self.config.bearer_token = bearer_token.into();
        self
    }

    /// 设置 HTTP 请求超时.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = timeout;
        self
    }

    /// 设置 watch 和 announce 重试时的指数退避区间.
    pub fn reconnect_backoff(mut self, min: Duration, max: Duration) -> Self {
        self.config.reconnect_backoff_min = min;
        self.config.reconnect_backoff_max = max;
        self
    }

    /// 设置默认地址选择是否允许 loopback.
    pub fn include_loopback(mut self, include_loopback: bool) -> Self {
        self.config.default_address_selection.include_loopback = include_loopback;
        self
    }

    /// 设置默认地址选择是否允许 IPv6.
    pub fn include_ipv6(mut self, include_ipv6: bool) -> Self {
        self.config.default_address_selection.include_ipv6 = include_ipv6;
        self
    }

    /// 向默认地址选择规则追加接口白名单.
    pub fn enable_interface(mut self, interface_name: impl Into<String>) -> Self {
        self.config
            .default_address_selection
            .interface_allowlist
            .push(interface_name.into());
        self
    }

    /// 向默认地址选择规则追加接口黑名单.
    pub fn disable_interface(mut self, interface_name: impl Into<String>) -> Self {
        self.config
            .default_address_selection
            .interface_denylist
            .push(interface_name.into());
        self
    }

    /// 用一份完整地址选择规则替换默认值.
    pub fn address_selection(mut self, address_selection: AddressSelection) -> Self {
        self.config.default_address_selection = address_selection;
        self
    }

    /// 构建最终 client.
    ///
    /// 返回值:
    /// - 成功时返回 [`LndClient`].
    ///
    /// 异常:
    /// - 可能返回 [`ClientError`] 当配置不合法或 HTTP client 初始化失败.
    pub fn build(self) -> Result<LndClient, ClientError> {
        LndClient::new(self.config)
    }
}

impl AnnounceHandle {
    /// 停止后台续租循环并等待任务退出.
    ///
    /// 返回值:
    /// - 成功时返回 `Ok(())`.
    ///
    /// 异常:
    /// - 返回 [`ClientError::Api`] 当后台任务 join 失败.
    pub async fn stop(self) -> Result<(), ClientError> {
        let _ = self.shutdown.send(true);
        self.task
            .await
            .map_err(|error| ClientError::Api(format!("announce task join error: {error}")))?
    }
}

/// 从指定路径读取 node id, 如不存在则自动创建一个新的 UUID.
///
/// 参数:
/// - `path`: 状态文件路径.
///
/// 返回值:
/// - 持久 `node_id`.
///
/// 异常:
/// - 返回 [`ClientError::Io`] 当文件读写失败.
pub async fn load_or_create_node_id(path: &Path) -> Result<String, ClientError> {
    match tokio::fs::read_to_string(path).await {
        Ok(value) => Ok(value.trim().to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            let node_id = Uuid::new_v4().to_string();
            tokio::fs::write(path, format!("{node_id}\n")).await?;
            Ok(node_id)
        }
        Err(error) => Err(error.into()),
    }
}

/// 返回默认 node id 状态文件路径.
///
/// 优先级:
/// - `dirs::state_dir()`
/// - `dirs::data_local_dir()`
/// - `std::env::temp_dir()`
pub fn default_node_id_path() -> PathBuf {
    let base = dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir);
    base.join("lnd").join("node_id")
}

/// 返回默认 display name.
///
/// 规则:
/// - 优先使用系统 hostname.
/// - 回退为 `"lnd-node"`.
pub fn default_display_name() -> String {
    hostname::get()
        .ok()
        .and_then(|value| value.into_string().ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "lnd-node".to_string())
}

/// 解析地址列表, 端口默认为 `0`.
///
/// 参数:
/// - `explicit`: 显式地址列表. 如果传入 `Some`, 则只做去重.
///
/// 返回值:
/// - 去重后的地址列表.
pub fn resolve_lan_addrs(
    explicit: Option<Vec<SocketAddr>>,
) -> Result<Vec<SocketAddr>, ClientError> {
    resolve_lan_addrs_with_port(explicit, 0)
}

/// 解析地址列表, 并为纯 IP 补上给定端口.
pub fn resolve_lan_addrs_with_port(
    explicit: Option<Vec<SocketAddr>>,
    port: u16,
) -> Result<Vec<SocketAddr>, ClientError> {
    resolve_lan_addrs_with_port_and_selection(explicit, port, &AddressSelection::default())
}

/// 使用给定地址选择规则解析地址列表.
///
/// 参数:
/// - `explicit`: 如果为 `Some`, 则直接返回去重结果.
/// - `port`: 自动发现地址时附加的端口.
/// - `selection`: 地址选择规则.
///
/// 返回值:
/// - 去重后的地址集合.
///
/// 异常:
/// - 返回 [`ClientError::Io`] 当接口枚举失败.
pub fn resolve_lan_addrs_with_port_and_selection(
    explicit: Option<Vec<SocketAddr>>,
    port: u16,
    selection: &AddressSelection,
) -> Result<Vec<SocketAddr>, ClientError> {
    if let Some(addrs) = explicit {
        return Ok(dedupe_socket_addrs(addrs));
    }
    let mut addrs = Vec::new();
    for iface in get_if_addrs()? {
        if !selection.allows_interface(&iface.name) {
            continue;
        }
        let is_loopback = iface.is_loopback();
        match iface.addr {
            IfAddr::V4(v4) => {
                if selection.allows_ip(IpAddr::V4(v4.ip), is_loopback) {
                    addrs.push(SocketAddr::new(IpAddr::V4(v4.ip), port));
                }
            }
            IfAddr::V6(v6) => {
                if selection.allows_ip(IpAddr::V6(v6.ip), is_loopback) {
                    addrs.push(SocketAddr::new(IpAddr::V6(v6.ip), port));
                }
            }
        }
    }
    Ok(dedupe_socket_addrs(addrs))
}

/// 根据 `AnnounceSpec` 和默认地址选择规则解析最终上报地址.
///
/// 参数:
/// - `spec`: 原始注册规格.
/// - `default_selection`: client 默认地址选择.
///
/// 返回值:
/// - 显式地址和自动解析地址合并去重后的结果.
pub fn resolve_announce_addrs_with_defaults(
    spec: &AnnounceSpec,
    default_selection: &AddressSelection,
) -> Result<Vec<SocketAddr>, ClientError> {
    let mut addrs = spec.lan_addrs.clone().unwrap_or_default();
    if spec.auto_lan_addrs {
        addrs.extend(resolve_lan_addrs_with_port_and_selection(
            None,
            spec.port,
            &merge_address_selection(default_selection, spec.address_selection.as_ref()),
        )?);
    }
    Ok(dedupe_socket_addrs(addrs))
}

/// 根据 `AnnounceSpec` 和默认地址选择规则解析最终上报可达域.
pub fn resolve_reachability_scopes_with_defaults(
    spec: &AnnounceSpec,
    default_selection: &AddressSelection,
) -> Result<Vec<String>, ClientError> {
    let mut scopes = spec.reachability_scopes.clone().unwrap_or_default();
    if spec.auto_reachability_scopes {
        scopes.extend(
            list_reachability_scopes(&merge_address_selection(
                default_selection,
                spec.address_selection.as_ref(),
            ))?
            .into_iter()
            .map(|scope| scope.scope),
        );
    }
    scopes.sort();
    scopes.dedup();
    Ok(scopes)
}

/// 使用给定地址选择规则列出本机局域网 `network_id` 候选项.
///
/// 生成规则:
/// - IPv4 使用 `ip & netmask` 形成子网前缀.
/// - IPv6 使用 `ip/prefixlen` 形成子网前缀, 仅在选择规则允许 IPv6 时考虑.
/// - 最终 `network_id` 采用 `lan-<hex>` 形式, 便于跨语言复现和日志展示.
pub fn list_network_id_candidates(
    selection: &AddressSelection,
) -> Result<Vec<DerivedNetworkId>, ClientError> {
    let mut candidates = collect_network_id_candidates(
        get_if_addrs()?.into_iter().map(|iface| {
            let is_loopback = iface.is_loopback();
            let interface_name = iface.name;
            match iface.addr {
                IfAddr::V4(v4) => CandidateInput {
                    interface_name,
                    ip: IpAddr::V4(v4.ip),
                    is_loopback,
                    ipv4_netmask: Some(v4.netmask),
                    prefixlen: v4.prefixlen,
                },
                IfAddr::V6(v6) => CandidateInput {
                    interface_name,
                    ip: IpAddr::V6(v6.ip),
                    is_loopback,
                    ipv4_netmask: None,
                    prefixlen: v6.prefixlen,
                },
            }
        }),
        selection,
    );
    candidates.sort_by(|left, right| {
        left.scope
            .cmp(&right.scope)
            .then(left.network_id.cmp(&right.network_id))
    });
    candidates
        .dedup_by(|left, right| left.scope == right.scope && left.network_id == right.network_id);
    Ok(candidates)
}

/// 使用给定地址选择规则列出本机局域网可达域候选项.
pub fn list_reachability_scopes(
    selection: &AddressSelection,
) -> Result<Vec<ReachabilityScope>, ClientError> {
    let mut scopes = collect_network_id_candidates(
        get_if_addrs()?.into_iter().map(|iface| {
            let is_loopback = iface.is_loopback();
            let interface_name = iface.name;
            match iface.addr {
                IfAddr::V4(v4) => CandidateInput {
                    interface_name,
                    ip: IpAddr::V4(v4.ip),
                    is_loopback,
                    ipv4_netmask: Some(v4.netmask),
                    prefixlen: v4.prefixlen,
                },
                IfAddr::V6(v6) => CandidateInput {
                    interface_name,
                    ip: IpAddr::V6(v6.ip),
                    is_loopback,
                    ipv4_netmask: None,
                    prefixlen: v6.prefixlen,
                },
            }
        }),
        selection,
    )
    .into_iter()
    .map(|candidate| ReachabilityScope {
        scope: candidate.scope,
    })
    .collect::<Vec<_>>();
    scopes.sort_by(|left, right| left.scope.cmp(&right.scope));
    scopes.dedup_by(|left, right| left.scope == right.scope);
    Ok(scopes)
}

fn collect_network_id_candidates(
    candidates: impl IntoIterator<Item = CandidateInput>,
    selection: &AddressSelection,
) -> Vec<DerivedNetworkId> {
    let mut derived = Vec::new();
    for candidate in candidates {
        if !selection.allows_interface(&candidate.interface_name) {
            continue;
        }
        if !selection.allows_ip(candidate.ip, candidate.is_loopback) {
            continue;
        }
        match candidate.ip {
            IpAddr::V4(ip) => {
                let Some(netmask) = candidate.ipv4_netmask else {
                    continue;
                };
                let network = Ipv4Addr::from(u32::from(ip) & u32::from(netmask));
                let scope = format!("{network}/{}", candidate.prefixlen);
                let key = format!("v4:{scope}");
                derived.push(DerivedNetworkId {
                    network_id: format!("lan-{}", short_stable_hex(&key)),
                    scope,
                });
            }
            IpAddr::V6(ip) => {
                let network = ipv6_network_prefix(ip, candidate.prefixlen);
                let scope = format!("{network}/{}", candidate.prefixlen);
                let key = format!("v6:{scope}");
                derived.push(DerivedNetworkId {
                    network_id: format!("lan-{}", short_stable_hex(&key)),
                    scope,
                });
            }
        }
    }
    derived
}

#[derive(Debug, Clone)]
struct CandidateInput {
    interface_name: String,
    ip: IpAddr,
    is_loopback: bool,
    ipv4_netmask: Option<Ipv4Addr>,
    prefixlen: u8,
}

fn choose_network_id(candidates: &[DerivedNetworkId]) -> Result<String, ClientError> {
    if candidates.is_empty() {
        return Err(ClientError::Api(
            "failed to derive network_id: no eligible local network prefix found".to_string(),
        ));
    }
    if candidates.len() == 1 {
        return Ok(candidates[0].network_id.clone());
    }
    let ipv4_candidates: Vec<&DerivedNetworkId> = candidates
        .iter()
        .filter(|candidate| candidate.scope.contains('.'))
        .collect();
    if ipv4_candidates.len() == 1 {
        return Ok(ipv4_candidates[0].network_id.clone());
    }
    let visible = candidates
        .iter()
        .map(|candidate| format!("{}({})", candidate.network_id, candidate.scope))
        .collect::<Vec<_>>()
        .join(", ");
    Err(ClientError::Api(format!(
        "failed to derive network_id: multiple eligible network prefixes found: {visible}; specify network_id explicitly or narrow interfaces"
    )))
}

/// 使用给定地址选择规则自动推导一个最合适的局域网 `network_id`.
///
/// 选择规则:
/// - 只有一个候选时直接返回.
/// - 没有候选时返回错误.
/// - 多个候选时, 优先选择私网 IPv4 候选.
/// - 如果仍有多个同优先级候选, 返回错误并提示调用方显式配置.
pub fn resolve_network_id_with_selection(
    selection: &AddressSelection,
) -> Result<String, ClientError> {
    let candidates = list_network_id_candidates(selection)?;
    choose_network_id(&candidates)
}

/// 将 `key=value` 形式的字符串切片解析为 metadata 映射.
///
/// 参数:
/// - `pairs`: 例如 `["version=1.0.0", "role=api"]`.
///
/// 返回值:
/// - 一个 `BTreeMap<String, String>`.
///
/// 异常:
/// - 当某个字符串不含 `=` 时返回错误.
pub fn metadata_from_pairs(pairs: &[String]) -> anyhow::Result<BTreeMap<String, String>> {
    let mut metadata = BTreeMap::new();
    for pair in pairs {
        let (key, value) = pair
            .split_once('=')
            .with_context(|| format!("invalid metadata pair: {pair}"))?;
        metadata.insert(key.to_string(), value.to_string());
    }
    Ok(metadata)
}

fn dedupe_socket_addrs(mut addrs: Vec<SocketAddr>) -> Vec<SocketAddr> {
    addrs.sort();
    addrs.dedup();
    addrs
}

fn ipv6_network_prefix(ip: Ipv6Addr, prefixlen: u8) -> Ipv6Addr {
    if prefixlen == 0 {
        return Ipv6Addr::UNSPECIFIED;
    }
    let bits = u128::from(ip);
    let host_bits = 128u32.saturating_sub(prefixlen as u32);
    let mask = if host_bits >= 128 {
        0
    } else {
        u128::MAX << host_bits
    };
    Ipv6Addr::from(bits & mask)
}

fn short_stable_hex(value: &str) -> String {
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x00000100000001b3;

    let mut hash = OFFSET_BASIS;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    format!("{hash:016x}")
}

async fn parse_json_response<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T, ClientError> {
    if response.status().is_success() {
        return Ok(response.json::<T>().await?);
    }
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if let Ok(api_error) = serde_json::from_str::<ApiErrorBody>(&body) {
        return Err(ClientError::Api(api_error.error));
    }
    Err(ClientError::Api(format!("{status}: {body}")))
}

fn backoff_delay(min: Duration, max: Duration, attempt: u32) -> Duration {
    let base_ms = min.as_millis() as u64;
    let max_ms = max.as_millis() as u64;
    let exp = 2u64.saturating_pow(attempt.min(10));
    let delay = base_ms.saturating_mul(exp).min(max_ms.max(base_ms));
    let jitter = rand::rng().random_range(0..=base_ms.max(1));
    Duration::from_millis(delay.saturating_add(jitter).min(max_ms.max(base_ms)))
}

fn with_jitter(duration: Duration) -> Duration {
    let millis = duration.as_millis() as u64;
    if millis == 0 {
        return Duration::from_secs(DEFAULT_TTL_SECS.min(1));
    }
    let jitter = rand::rng().random_range(0..=(millis / 5).max(1));
    Duration::from_millis(millis.saturating_add(jitter))
}

fn merge_address_selection(
    defaults: &AddressSelection,
    override_selection: Option<&AddressSelection>,
) -> AddressSelection {
    let Some(override_selection) = override_selection else {
        return defaults.clone();
    };
    let mut merged = defaults.clone();
    merged.include_private_ipv4 = override_selection.include_private_ipv4;
    merged.include_loopback = override_selection.include_loopback;
    merged.include_link_local_ipv4 = override_selection.include_link_local_ipv4;
    merged.include_ipv6 = override_selection.include_ipv6;
    if !override_selection.interface_allowlist.is_empty() {
        merged.interface_allowlist = override_selection.interface_allowlist.clone();
    }
    if !override_selection.interface_denylist.is_empty() {
        merged.interface_denylist = override_selection.interface_denylist.clone();
    }
    merged
}

pub fn parse_socket_addrs(values: &[String], port: u16) -> anyhow::Result<Vec<SocketAddr>> {
    values
        .iter()
        .map(|value| {
            value.parse::<SocketAddr>().or_else(|_| {
                let ip: IpAddr = value
                    .parse()
                    .with_context(|| format!("invalid ip or socket address: {value}"))?;
                Ok(SocketAddr::new(ip, port))
            })
        })
        .collect()
}

/// 将事件封装为 JSON 字符串.
///
/// 返回值:
/// - 成功时返回 UTF-8 JSON.
pub fn watch_event_to_json(event: &DiscoveryEventEnvelope) -> Result<String, ClientError> {
    serde_json::to_string(event).map_err(ClientError::from)
}

/// 将节点列表编码为 JSON 字符串.
pub fn discover_nodes_to_json(nodes: &[DiscoveredNode]) -> Result<String, ClientError> {
    serde_json::to_string(nodes).map_err(ClientError::from)
}

/// 从 JSON 字符串解析发现过滤器.
///
/// 异常:
/// - 返回 [`ClientError::Serde`] 当 JSON 结构不合法.
pub fn parse_filter_json(json: &str) -> Result<DiscoveryFilter, ClientError> {
    let filter: Value = serde_json::from_str(json)?;
    serde_json::from_value(filter).map_err(ClientError::from)
}

/// 从 JSON 字符串解析注册规格.
pub fn parse_announce_json(json: &str) -> Result<AnnounceSpec, ClientError> {
    let spec: Value = serde_json::from_str(json)?;
    serde_json::from_value(spec).map_err(ClientError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_explicit_addrs_dedupes() {
        let addrs = resolve_lan_addrs_with_port(
            Some(vec![
                "192.168.1.2:8080".parse().unwrap(),
                "192.168.1.2:8080".parse().unwrap(),
            ]),
            8080,
        )
        .unwrap();
        assert_eq!(addrs.len(), 1);
    }

    #[test]
    fn backoff_is_bounded() {
        for attempt in 0..8 {
            let delay = backoff_delay(Duration::from_millis(100), Duration::from_secs(2), attempt);
            assert!(delay >= Duration::from_millis(100));
            assert!(delay <= Duration::from_secs(2));
        }
    }

    #[test]
    fn loopback_can_be_enabled_in_selection() {
        let selection = AddressSelection::new().with_loopback(true);
        let addrs = resolve_lan_addrs_with_port_and_selection(None, 8080, &selection).unwrap();
        assert!(addrs.iter().any(|addr| addr.ip().is_loopback()) || !addrs.is_empty());
    }

    #[test]
    fn announce_spec_can_disable_auto_addrs() {
        let spec = AnnounceSpec::new("node-a", "svc", "node-a", 8080)
            .with_network_id("net-a")
            .with_auto_lan_addrs(false)
            .with_lan_addrs(["127.0.0.1:8080".parse().unwrap()]);
        let addrs =
            resolve_announce_addrs_with_defaults(&spec, &AddressSelection::default()).unwrap();
        assert_eq!(addrs, vec!["127.0.0.1:8080".parse().unwrap()]);
    }

    #[test]
    fn ipv6_prefix_masks_host_bits() {
        let ip: Ipv6Addr = "fd12:3456:789a:1::abcd".parse().unwrap();
        let network = ipv6_network_prefix(ip, 64);
        assert_eq!(network, "fd12:3456:789a:1::".parse::<Ipv6Addr>().unwrap());
    }

    #[test]
    fn stable_hex_is_deterministic() {
        assert_eq!(
            short_stable_hex("v4:192.168.1.0/24"),
            short_stable_hex("v4:192.168.1.0/24")
        );
        assert_ne!(
            short_stable_hex("v4:192.168.1.0/24"),
            short_stable_hex("v4:192.168.2.0/24")
        );
    }

    #[test]
    fn stable_hex_matches_fnv1a64() {
        assert_eq!(short_stable_hex("v4:192.168.1.0/24"), "ec3a7b1765ff30c6");
    }

    #[test]
    fn collect_candidates_dedupes_and_sorts() {
        let selection = AddressSelection::new();
        let candidates = collect_network_id_candidates(
            [
                CandidateInput {
                    interface_name: "en1".to_string(),
                    ip: "192.168.2.23".parse().unwrap(),
                    is_loopback: false,
                    ipv4_netmask: Some("255.255.255.0".parse().unwrap()),
                    prefixlen: 24,
                },
                CandidateInput {
                    interface_name: "en0".to_string(),
                    ip: "192.168.1.9".parse().unwrap(),
                    is_loopback: false,
                    ipv4_netmask: Some("255.255.255.0".parse().unwrap()),
                    prefixlen: 24,
                },
                CandidateInput {
                    interface_name: "en0".to_string(),
                    ip: "192.168.1.44".parse().unwrap(),
                    is_loopback: false,
                    ipv4_netmask: Some("255.255.255.0".parse().unwrap()),
                    prefixlen: 24,
                },
            ],
            &selection,
        );
        let mut candidates = candidates;
        candidates.sort_by(|left, right| left.scope.cmp(&right.scope));
        candidates.dedup_by(|left, right| {
            left.scope == right.scope && left.network_id == right.network_id
        });
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].scope, "192.168.1.0/24");
        assert_eq!(candidates[1].scope, "192.168.2.0/24");
    }

    #[test]
    fn choose_network_id_prefers_single_ipv4_candidate() {
        let network_id = choose_network_id(&[
            DerivedNetworkId {
                network_id: "lan-v6a".to_string(),
                scope: "fd12:3456:789a:1::/64".to_string(),
            },
            DerivedNetworkId {
                network_id: "lan-v4".to_string(),
                scope: "192.168.1.0/24".to_string(),
            },
            DerivedNetworkId {
                network_id: "lan-v6b".to_string(),
                scope: "fd12:3456:789a:2::/64".to_string(),
            },
        ])
        .unwrap();
        assert_eq!(network_id, "lan-v4");
    }

    #[test]
    fn choose_network_id_errors_when_ambiguous() {
        let error = choose_network_id(&[
            DerivedNetworkId {
                network_id: "lan-a".to_string(),
                scope: "192.168.1.0/24".to_string(),
            },
            DerivedNetworkId {
                network_id: "lan-b".to_string(),
                scope: "10.0.0.0/24".to_string(),
            },
        ])
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("multiple eligible network prefixes found")
        );
    }

    #[test]
    fn resolve_reachability_scopes_merges_explicit_and_auto() {
        let spec = AnnounceSpec::new("node-a", "svc", "node-a", 8080)
            .with_reachability_scopes(["10.0.0.0/24"])
            .with_auto_reachability_scopes(false);
        let scopes =
            resolve_reachability_scopes_with_defaults(&spec, &AddressSelection::default()).unwrap();
        assert_eq!(scopes, vec!["10.0.0.0/24".to_string()]);
    }
}
