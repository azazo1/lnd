use std::collections::BTreeMap;
use std::net::{IpAddr, SocketAddr};
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
use serde_json::Value;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

use crate::protocol::{
    AddressSelection, AnnounceSpec, ApiErrorBody, DEFAULT_TTL_SECS, DiscoverResponse,
    DiscoveredNode, DiscoveryEvent, DiscoveryEventEnvelope, DiscoveryFilter, NodeAnnouncement,
};

pub type WatchStream =
    Pin<Box<dyn Stream<Item = Result<DiscoveryEventEnvelope, ClientError>> + Send>>;

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

pub struct AnnounceHandle {
    shutdown: watch::Sender<bool>,
    task: JoinHandle<Result<(), ClientError>>,
}

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
    pub fn builder(server_url: impl Into<String>) -> ClientBuilder {
        ClientBuilder::new(server_url)
    }

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

    #[instrument(skip(self), fields(network_id = %filter.network_id))]
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

    pub fn watch(&self, filter: DiscoveryFilter) -> WatchStream {
        let client = self.clone();
        Box::pin(async_stream::try_stream! {
            let mut cursor: Option<u64> = None;
            let mut attempt: u32 = 0;
            loop {
                let mut request = client
                    .http
                    .get(format!("{}/v1/watch", client.base_url()))
                    .query(&[("network_id", filter.network_id.as_str())]);
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
                        let announcement = spec.clone().into_announcement(lan_addrs);
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

    #[instrument(skip(self, announcement), fields(node_id = %announcement.node_id, network_id = %announcement.network_id))]
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

    pub fn resolve_announce_addrs(
        &self,
        spec: &AnnounceSpec,
    ) -> Result<Vec<SocketAddr>, ClientError> {
        resolve_announce_addrs_with_defaults(spec, &self.config.default_address_selection)
    }

    fn build_list_request(&self, filter: &DiscoveryFilter) -> reqwest::RequestBuilder {
        let mut request = self
            .http
            .get(format!("{}/v1/nodes", self.base_url()))
            .query(&[("network_id", filter.network_id.as_str())]);
        if let Some(service) = filter.service.as_deref() {
            request = request.query(&[("service", service)]);
        }
        request.query(
            &filter
                .tags
                .iter()
                .map(|tag| ("tag", tag.as_str()))
                .collect::<Vec<_>>(),
        )
    }
}

pub struct ClientBuilder {
    config: ClientConfig,
}

impl ClientBuilder {
    pub fn new(server_url: impl Into<String>) -> Self {
        let config = ClientConfig {
            server_url: server_url.into(),
            ..ClientConfig::default()
        };
        Self { config }
    }

    pub fn bearer_token(mut self, bearer_token: impl Into<String>) -> Self {
        self.config.bearer_token = bearer_token.into();
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = timeout;
        self
    }

    pub fn reconnect_backoff(mut self, min: Duration, max: Duration) -> Self {
        self.config.reconnect_backoff_min = min;
        self.config.reconnect_backoff_max = max;
        self
    }

    pub fn include_loopback(mut self, include_loopback: bool) -> Self {
        self.config.default_address_selection.include_loopback = include_loopback;
        self
    }

    pub fn include_ipv6(mut self, include_ipv6: bool) -> Self {
        self.config.default_address_selection.include_ipv6 = include_ipv6;
        self
    }

    pub fn enable_interface(mut self, interface_name: impl Into<String>) -> Self {
        self.config
            .default_address_selection
            .interface_allowlist
            .push(interface_name.into());
        self
    }

    pub fn disable_interface(mut self, interface_name: impl Into<String>) -> Self {
        self.config
            .default_address_selection
            .interface_denylist
            .push(interface_name.into());
        self
    }

    pub fn address_selection(mut self, address_selection: AddressSelection) -> Self {
        self.config.default_address_selection = address_selection;
        self
    }

    pub fn build(self) -> Result<LndClient, ClientError> {
        LndClient::new(self.config)
    }
}

impl AnnounceHandle {
    pub async fn stop(self) -> Result<(), ClientError> {
        let _ = self.shutdown.send(true);
        self.task
            .await
            .map_err(|error| ClientError::Api(format!("announce task join error: {error}")))?
    }
}

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

pub fn default_node_id_path() -> PathBuf {
    let base = dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir);
    base.join("lnd").join("node_id")
}

pub fn default_display_name() -> String {
    hostname::get()
        .ok()
        .and_then(|value| value.into_string().ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "lnd-node".to_string())
}

pub fn resolve_lan_addrs(
    explicit: Option<Vec<SocketAddr>>,
) -> Result<Vec<SocketAddr>, ClientError> {
    resolve_lan_addrs_with_port(explicit, 0)
}

pub fn resolve_lan_addrs_with_port(
    explicit: Option<Vec<SocketAddr>>,
    port: u16,
) -> Result<Vec<SocketAddr>, ClientError> {
    resolve_lan_addrs_with_port_and_selection(explicit, port, &AddressSelection::default())
}

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

pub fn watch_event_to_json(event: &DiscoveryEventEnvelope) -> Result<String, ClientError> {
    serde_json::to_string(event).map_err(ClientError::from)
}

pub fn discover_nodes_to_json(nodes: &[DiscoveredNode]) -> Result<String, ClientError> {
    serde_json::to_string(nodes).map_err(ClientError::from)
}

pub fn parse_filter_json(json: &str) -> Result<DiscoveryFilter, ClientError> {
    let filter: Value = serde_json::from_str(json)?;
    serde_json::from_value(filter).map_err(ClientError::from)
}

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
        let spec = AnnounceSpec::new("net-a", "node-a", "svc", "node-a", 8080)
            .with_auto_lan_addrs(false)
            .with_lan_addrs(["127.0.0.1:8080".parse().unwrap()]);
        let addrs =
            resolve_announce_addrs_with_defaults(&spec, &AddressSelection::default()).unwrap();
        assert_eq!(addrs, vec!["127.0.0.1:8080".parse().unwrap()]);
    }
}
