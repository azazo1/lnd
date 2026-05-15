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
    AnnounceSpec, ApiErrorBody, DEFAULT_TTL_SECS, DiscoverResponse, DiscoveryEvent,
    DiscoveryEventEnvelope, DiscoveryFilter, DiscoveredNode, NodeAnnouncement,
};

pub type WatchStream = Pin<Box<dyn Stream<Item = Result<DiscoveryEventEnvelope, ClientError>> + Send>>;

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub server_url: String,
    pub bearer_token: String,
    pub timeout: Duration,
    pub reconnect_backoff_min: Duration,
    pub reconnect_backoff_max: Duration,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            server_url: "http://127.0.0.1:8765".to_string(),
            bearer_token: String::new(),
            timeout: Duration::from_secs(10),
            reconnect_backoff_min: Duration::from_millis(500),
            reconnect_backoff_max: Duration::from_secs(15),
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
        self.list_response(filter).await.map(|response| response.nodes)
    }

    async fn list_response(&self, filter: DiscoveryFilter) -> Result<DiscoverResponse, ClientError> {
        let response = self
            .build_list_request(&filter)
            .send()
            .await?;
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
                        let lan_addrs = resolve_lan_addrs_with_port(spec.lan_addrs.clone(), spec.port)?;
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
    pub async fn announce_once(&self, announcement: NodeAnnouncement) -> Result<DiscoveredNode, ClientError> {
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

pub fn resolve_lan_addrs(explicit: Option<Vec<SocketAddr>>) -> Result<Vec<SocketAddr>, ClientError> {
    resolve_lan_addrs_with_port(explicit, 0)
}

pub fn resolve_lan_addrs_with_port(
    explicit: Option<Vec<SocketAddr>>,
    port: u16,
) -> Result<Vec<SocketAddr>, ClientError> {
    if let Some(addrs) = explicit {
        return Ok(dedupe_socket_addrs(addrs));
    }
    let mut addrs = Vec::new();
    for iface in get_if_addrs()? {
        if iface.is_loopback() {
            continue;
        }
        match iface.addr {
            IfAddr::V4(v4) => {
                if is_private_ip(IpAddr::V4(v4.ip)) {
                    addrs.push(SocketAddr::new(IpAddr::V4(v4.ip), port));
                }
            }
            IfAddr::V6(_) => {}
        }
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

fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => ipv4.is_private(),
        IpAddr::V6(_) => false,
    }
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
        let addrs = resolve_lan_addrs_with_port(Some(vec![
            "192.168.1.2:8080".parse().unwrap(),
            "192.168.1.2:8080".parse().unwrap(),
        ]), 8080)
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
}
