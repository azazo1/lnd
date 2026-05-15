use std::collections::{HashMap, VecDeque};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Context;
use async_stream::stream;
use axum::extract::{Path, Query, RawQuery, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, put};
use axum::{Json, Router};
use parking_lot::RwLock;
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio::time::MissedTickBehavior;
use tower_http::trace::TraceLayer;
use tracing::{debug, info, instrument, warn};

use crate::protocol::{
    ApiErrorBody, DEFAULT_EVENT_BUFFER_CAPACITY, DEFAULT_SSE_KEEPALIVE_SECS, DEFAULT_TTL_SECS,
    DiscoverResponse, DiscoveredNode, DiscoveryEvent, DiscoveryEventEnvelope, DiscoveryFilter, LeaseInfo,
    NodeAnnouncement,
};

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub listen_addr: SocketAddr,
    pub bearer_token: String,
    pub sse_keepalive_secs: u64,
    pub event_buffer_capacity: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: SocketAddr::from(([0, 0, 0, 0], 8765)),
            bearer_token: String::new(),
            sse_keepalive_secs: DEFAULT_SSE_KEEPALIVE_SECS,
            event_buffer_capacity: DEFAULT_EVENT_BUFFER_CAPACITY,
        }
    }
}

#[derive(Debug, Clone)]
pub struct InMemoryRegistry {
    inner: Arc<RegistryInner>,
}

#[derive(Debug)]
struct RegistryInner {
    state: RwLock<RegistryState>,
    tx: broadcast::Sender<DiscoveryEventEnvelope>,
    event_capacity: usize,
}

#[derive(Debug)]
struct RegistryState {
    nodes: HashMap<String, NodeEntry>,
    events: VecDeque<DiscoveryEventEnvelope>,
    next_revision: u64,
}

#[derive(Debug, Clone)]
struct NodeEntry {
    announcement: NodeAnnouncement,
    last_seen_unix_ms: u64,
    expires_at_unix_ms: u64,
    revision: u64,
}

#[derive(Debug, Clone)]
pub struct RegistrySnapshot {
    pub nodes: Vec<DiscoveredNode>,
    pub cursor: u64,
}

#[derive(Debug, Deserialize, Default)]
struct FilterQuery {
    network_id: Option<String>,
    service: Option<String>,
    cursor: Option<u64>,
}

#[derive(Debug, Clone)]
struct AppState {
    config: ServerConfig,
    registry: InMemoryRegistry,
}

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("invalid ttl")]
    InvalidTtl,
    #[error("cursor too old")]
    CursorTooOld,
}

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error(transparent)]
    Registry(#[from] RegistryError),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error) = match self {
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized".to_string()),
            Self::BadRequest(error) => (StatusCode::BAD_REQUEST, error),
            Self::Registry(RegistryError::InvalidTtl) => (StatusCode::BAD_REQUEST, "invalid ttl".to_string()),
            Self::Registry(RegistryError::CursorTooOld) => {
                (StatusCode::CONFLICT, "cursor too old".to_string())
            }
        };
        (status, Json(ApiErrorBody { error })).into_response()
    }
}

impl InMemoryRegistry {
    pub fn new(event_capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(event_capacity.max(16));
        Self {
            inner: Arc::new(RegistryInner {
                state: RwLock::new(RegistryState {
                    nodes: HashMap::new(),
                    events: VecDeque::with_capacity(event_capacity),
                    next_revision: 1,
                }),
                tx,
                event_capacity,
            }),
        }
    }

    #[instrument(skip(self, announcement), fields(node_id = %announcement.node_id, network_id = %announcement.network_id))]
    pub fn upsert(&self, announcement: NodeAnnouncement) -> Result<DiscoveredNode, RegistryError> {
        if announcement.ttl_secs == 0 {
            return Err(RegistryError::InvalidTtl);
        }

        let now = now_unix_ms();
        let mut state = self.inner.state.write();
        let revision = state.next_revision;
        state.next_revision += 1;
        let expires_at = now.saturating_add(announcement.ttl_secs.saturating_mul(1000));
        let entry = NodeEntry {
            announcement: dedupe_announcement(announcement),
            last_seen_unix_ms: now,
            expires_at_unix_ms: expires_at,
            revision,
        };
        let node = entry.to_discovered_node();
        state.nodes.insert(entry.announcement.node_id.clone(), entry);
        self.push_event_locked(
            &mut state,
            DiscoveryEventEnvelope {
                cursor: Some(revision),
                event: DiscoveryEvent::Upsert { node: node.clone() },
            },
        );
        Ok(node)
    }

    pub fn remove_expired(&self) -> usize {
        let now = now_unix_ms();
        let mut removed = Vec::new();
        {
            let mut state = self.inner.state.write();
            let expired_nodes: Vec<DiscoveredNode> = state
                .nodes
                .iter()
                .filter(|(_, entry)| entry.expires_at_unix_ms <= now)
                .map(|(_, entry)| entry.to_discovered_node())
                .collect();
            for node in expired_nodes {
                state.nodes.remove(&node.node_id);
                let revision = state.next_revision;
                state.next_revision += 1;
                removed.push((node, revision));
            }
            for (node, revision) in &removed {
                self.push_event_locked(
                    &mut state,
                    DiscoveryEventEnvelope {
                        cursor: Some(*revision),
                        event: DiscoveryEvent::Remove { node: node.clone() },
                    },
                );
            }
        }
        removed.len()
    }

    pub fn list(&self, filter: &DiscoveryFilter) -> RegistrySnapshot {
        let state = self.inner.state.read();
        let nodes = state
            .nodes
            .values()
            .filter(|entry| filter_matches(filter, &entry.announcement))
            .map(NodeEntry::to_discovered_node)
            .collect();
        RegistrySnapshot {
            nodes,
            cursor: state.next_revision.saturating_sub(1),
        }
    }

    pub fn replay_since(
        &self,
        cursor: u64,
        filter: &DiscoveryFilter,
    ) -> Result<Vec<DiscoveryEventEnvelope>, RegistryError> {
        let state = self.inner.state.read();
        if cursor == 0 {
            return Ok(state
                .events
                .iter()
                .filter(|event| event_matches_filter(event, filter))
                .cloned()
                .collect());
        }

        if let Some(oldest) = state.events.front().and_then(|event| event.cursor)
            && cursor < oldest.saturating_sub(1)
        {
            return Err(RegistryError::CursorTooOld);
        }
        Ok(state
            .events
            .iter()
            .filter(|event| event.cursor.is_some_and(|event_cursor| event_cursor > cursor))
            .filter(|event| event_matches_filter(event, filter))
            .cloned()
            .collect())
    }

    pub fn subscribe(&self) -> broadcast::Receiver<DiscoveryEventEnvelope> {
        self.inner.tx.subscribe()
    }

    fn push_event_locked(&self, state: &mut RegistryState, event: DiscoveryEventEnvelope) {
        if state.events.len() >= self.inner.event_capacity {
            state.events.pop_front();
        }
        state.events.push_back(event.clone());
        let _ = self.inner.tx.send(event);
    }
}

impl Default for InMemoryRegistry {
    fn default() -> Self {
        Self::new(DEFAULT_EVENT_BUFFER_CAPACITY)
    }
}

impl NodeEntry {
    fn to_discovered_node(&self) -> DiscoveredNode {
        DiscoveredNode {
            network_id: self.announcement.network_id.clone(),
            node_id: self.announcement.node_id.clone(),
            service: self.announcement.service.clone(),
            display_name: self.announcement.display_name.clone(),
            port: self.announcement.port,
            lan_addrs: self.announcement.lan_addrs.clone(),
            tags: self.announcement.tags.clone(),
            metadata: self.announcement.metadata.clone(),
            lease: LeaseInfo {
                revision: self.revision,
                ttl_secs: self.announcement.ttl_secs,
                expires_at_unix_ms: self.expires_at_unix_ms,
                last_seen_unix_ms: self.last_seen_unix_ms,
            },
        }
    }
}

pub fn build_router(config: ServerConfig, registry: InMemoryRegistry) -> Router {
    let state = AppState { config, registry };
    Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/nodes", get(list_nodes))
        .route("/v1/nodes/{node_id}", put(upsert_node))
        .route("/v1/watch", get(watch_nodes))
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

#[instrument(skip(config, registry))]
pub async fn run_server(config: ServerConfig, registry: InMemoryRegistry) -> anyhow::Result<()> {
    let app = build_router(config.clone(), registry.clone());
    let listener = TcpListener::bind(config.listen_addr)
        .await
        .with_context(|| format!("failed to bind {}", config.listen_addr))?;
    info!(addr = %config.listen_addr, "lnd server listening");

    let cleanup_registry = registry.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            let removed = cleanup_registry.remove_expired();
            if removed > 0 {
                debug!(removed, "expired leases removed");
            }
        }
    });

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server failed")
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}

async fn healthz() -> &'static str {
    "ok"
}

async fn list_nodes(
    State(state): State<AppState>,
    headers: HeaderMap,
    RawQuery(raw_query): RawQuery,
    Query(query): Query<FilterQuery>,
) -> Result<Json<DiscoverResponse>, AppError> {
    authorize(&state.config, &headers)?;
    let filter = parse_filter(raw_query.as_deref(), &query)?;
    let snapshot = state.registry.list(&filter);
    Ok(Json(DiscoverResponse {
        nodes: snapshot.nodes,
        cursor: snapshot.cursor,
    }))
}

async fn upsert_node(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(node_id): Path<String>,
    Json(mut body): Json<NodeAnnouncement>,
) -> Result<Json<DiscoveredNode>, AppError> {
    authorize(&state.config, &headers)?;
    if body.node_id != node_id {
        return Err(AppError::BadRequest("path node_id does not match body".to_string()));
    }
    if body.network_id.trim().is_empty()
        || body.node_id.trim().is_empty()
        || body.service.trim().is_empty()
        || body.display_name.trim().is_empty()
    {
        return Err(AppError::BadRequest("missing required fields".to_string()));
    }
    if body.ttl_secs == 0 {
        body.ttl_secs = DEFAULT_TTL_SECS;
    }
    let node = state.registry.upsert(body)?;
    Ok(Json(node))
}

async fn watch_nodes(
    State(state): State<AppState>,
    headers: HeaderMap,
    RawQuery(raw_query): RawQuery,
    Query(query): Query<FilterQuery>,
) -> Result<Response, AppError> {
    authorize(&state.config, &headers)?;
    let filter = parse_filter(raw_query.as_deref(), &query)?;
    let snapshot = state.registry.list(&filter);
    let replay = match query.cursor {
        Some(cursor) => match state.registry.replay_since(cursor, &filter) {
            Ok(events) => Some(events),
            Err(RegistryError::CursorTooOld) => None,
            Err(error) => return Err(error.into()),
        },
        None => Some(Vec::new()),
    };

    let registry = state.registry.clone();
    let keepalive_secs = state.config.sse_keepalive_secs;
    let filter_for_stream = filter.clone();
    let response_stream = stream! {
        let snapshot_event = if replay.is_some() {
            if query.cursor.is_some() {
                None
            } else {
                Some(DiscoveryEventEnvelope {
                    cursor: Some(snapshot.cursor),
                    event: DiscoveryEvent::Snapshot {
                        nodes: snapshot.nodes.clone(),
                    },
                })
            }
        } else {
            Some(DiscoveryEventEnvelope {
                cursor: Some(snapshot.cursor),
                event: DiscoveryEvent::Reset,
            })
        };

        if let Some(event) = snapshot_event {
            yield Ok::<Event, Infallible>(serialize_event(event));
        }
        if let Some(events) = replay {
            for event in events {
                yield Ok::<Event, Infallible>(serialize_event(event));
            }
        }

        let mut rx = registry.subscribe();
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if event_matches_filter(&event, &filter_for_stream) {
                        yield Ok::<Event, Infallible>(serialize_event(event));
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    yield Ok::<Event, Infallible>(serialize_event(DiscoveryEventEnvelope {
                        cursor: None,
                        event: DiscoveryEvent::Reset,
                    }));
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    warn!("watch channel closed");
                    break;
                }
            }
        }
    };

    let mut response = Sse::new(response_stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(keepalive_secs))
                .text(": keepalive"),
        )
        .into_response();
    let headers = response.headers_mut();
    headers.insert("cache-control", HeaderValue::from_static("no-cache"));
    headers.insert("x-accel-buffering", HeaderValue::from_static("no"));
    headers.insert("content-type", HeaderValue::from_static("text/event-stream"));
    Ok(response)
}

fn serialize_event(envelope: DiscoveryEventEnvelope) -> Event {
    let event_name = match envelope.event {
        DiscoveryEvent::Snapshot { .. } => "snapshot",
        DiscoveryEvent::Upsert { .. } => "upsert",
        DiscoveryEvent::Remove { .. } => "remove",
        DiscoveryEvent::Reset => "reset",
        DiscoveryEvent::Keepalive => "keepalive",
    };
    let json = serde_json::to_string(&envelope).expect("event serialization must succeed");
    Event::default().event(event_name).data(json)
}

fn authorize(config: &ServerConfig, headers: &HeaderMap) -> Result<(), AppError> {
    if config.bearer_token.is_empty() {
        return Ok(());
    }
    let Some(header) = headers.get(axum::http::header::AUTHORIZATION) else {
        return Err(AppError::Unauthorized);
    };
    let value = header
        .to_str()
        .map_err(|_| AppError::BadRequest("invalid authorization header".to_string()))?;
    let expected = format!("Bearer {}", config.bearer_token);
    if value != expected {
        return Err(AppError::Unauthorized);
    }
    Ok(())
}

fn parse_filter(raw_query: Option<&str>, query: &FilterQuery) -> Result<DiscoveryFilter, AppError> {
    let network_id = query
        .network_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::BadRequest("missing network_id".to_string()))?;
    let tags = raw_query
        .map(|query| {
            url::form_urlencoded::parse(query.as_bytes())
                .filter(|(key, _)| key == "tag")
                .map(|(_, value)| value.into_owned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(DiscoveryFilter {
        network_id,
        service: query.service.clone(),
        tags,
    })
}

fn filter_matches(filter: &DiscoveryFilter, announcement: &NodeAnnouncement) -> bool {
    if announcement.network_id != filter.network_id {
        return false;
    }
    if let Some(service) = &filter.service
        && &announcement.service != service
    {
        return false;
    }
    filter
        .tags
        .iter()
        .all(|tag| announcement.tags.iter().any(|value| value == tag))
}

fn event_matches_filter(event: &DiscoveryEventEnvelope, filter: &DiscoveryFilter) -> bool {
    match &event.event {
        DiscoveryEvent::Snapshot { nodes } => nodes.iter().any(|node| discovered_matches(filter, node)),
        DiscoveryEvent::Upsert { node } => discovered_matches(filter, node),
        DiscoveryEvent::Remove { node } => discovered_matches(filter, node),
        DiscoveryEvent::Reset | DiscoveryEvent::Keepalive => true,
    }
}

fn discovered_matches(filter: &DiscoveryFilter, node: &DiscoveredNode) -> bool {
    if node.network_id != filter.network_id {
        return false;
    }
    if let Some(service) = &filter.service
        && &node.service != service
    {
        return false;
    }
    filter.tags.iter().all(|tag| node.tags.iter().any(|value| value == tag))
}

fn dedupe_announcement(mut announcement: NodeAnnouncement) -> NodeAnnouncement {
    announcement.lan_addrs.sort();
    announcement.lan_addrs.dedup();
    announcement.tags.sort();
    announcement.tags.dedup();
    announcement
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn sample_announcement(node_id: &str, tags: &[&str], ttl_secs: u64) -> NodeAnnouncement {
        NodeAnnouncement {
            network_id: "net-a".to_string(),
            node_id: node_id.to_string(),
            service: "svc".to_string(),
            display_name: "node".to_string(),
            port: 8080,
            lan_addrs: vec![
                "192.168.1.10:8080".parse().unwrap(),
                "192.168.1.10:8080".parse().unwrap(),
            ],
            tags: tags.iter().map(|value| (*value).to_string()).collect(),
            metadata: BTreeMap::new(),
            ttl_secs,
        }
    }

    #[test]
    fn list_filters_by_tags() {
        let registry = InMemoryRegistry::new(32);
        registry.upsert(sample_announcement("node-1", &["alpha", "beta"], 30)).unwrap();
        registry.upsert(sample_announcement("node-2", &["beta"], 30)).unwrap();

        let snapshot = registry.list(&DiscoveryFilter {
            network_id: "net-a".to_string(),
            service: Some("svc".to_string()),
            tags: vec!["alpha".to_string()],
        });

        assert_eq!(snapshot.nodes.len(), 1);
        assert_eq!(snapshot.nodes[0].node_id, "node-1");
    }

    #[test]
    fn remove_expired_nodes_emits_remove_event() {
        let registry = InMemoryRegistry::new(32);
        registry.upsert(sample_announcement("node-1", &[], 1)).unwrap();
        std::thread::sleep(Duration::from_millis(1100));
        assert_eq!(registry.remove_expired(), 1);

        let events = registry
            .replay_since(
                0,
                &DiscoveryFilter {
                    network_id: "net-a".to_string(),
                    service: None,
                    tags: vec![],
                },
            )
            .unwrap();
        assert!(events.iter().any(|event| matches!(event.event, DiscoveryEvent::Remove { .. })));
    }

    #[test]
    fn replay_since_rejects_old_cursor() {
        let registry = InMemoryRegistry::new(2);
        registry.upsert(sample_announcement("node-1", &[], 30)).unwrap();
        registry.upsert(sample_announcement("node-2", &[], 30)).unwrap();
        registry.upsert(sample_announcement("node-3", &[], 30)).unwrap();
        registry.upsert(sample_announcement("node-4", &[], 30)).unwrap();

        let result = registry.replay_since(
            1,
            &DiscoveryFilter {
                network_id: "net-a".to_string(),
                service: None,
                tags: vec![],
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn dedupe_lan_addrs_and_tags() {
        let registry = InMemoryRegistry::new(32);
        let node = registry
            .upsert(sample_announcement("node-1", &["beta", "alpha", "alpha"], 30))
            .unwrap();
        assert_eq!(node.lan_addrs.len(), 1);
        assert_eq!(node.tags, vec!["alpha".to_string(), "beta".to_string()]);
    }
}
