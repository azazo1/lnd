use std::collections::BTreeMap;
use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

pub const DEFAULT_TTL_SECS: u64 = 30;
pub const DEFAULT_RENEW_INTERVAL_SECS: u64 = 10;
pub const DEFAULT_SSE_KEEPALIVE_SECS: u64 = 15;
pub const DEFAULT_EVENT_BUFFER_CAPACITY: usize = 4096;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeAnnouncement {
    pub network_id: String,
    pub node_id: String,
    pub service: String,
    pub display_name: String,
    pub port: u16,
    pub lan_addrs: Vec<SocketAddr>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
    #[serde(default = "default_ttl_secs")]
    pub ttl_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveredNode {
    pub network_id: String,
    pub node_id: String,
    pub service: String,
    pub display_name: String,
    pub port: u16,
    pub lan_addrs: Vec<SocketAddr>,
    pub tags: Vec<String>,
    pub metadata: BTreeMap<String, String>,
    pub lease: LeaseInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LeaseInfo {
    pub revision: u64,
    pub ttl_secs: u64,
    pub expires_at_unix_ms: u64,
    pub last_seen_unix_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveryFilter {
    pub network_id: String,
    #[serde(default)]
    pub service: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoverResponse {
    pub nodes: Vec<DiscoveredNode>,
    pub cursor: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WatchResponse {
    pub cursor: Option<u64>,
    pub event: DiscoveryEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DiscoveryEvent {
    Snapshot { nodes: Vec<DiscoveredNode> },
    Upsert { node: DiscoveredNode },
    Remove { node: DiscoveredNode },
    Reset,
    Keepalive,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveryEventEnvelope {
    pub cursor: Option<u64>,
    pub event: DiscoveryEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApiErrorBody {
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnnounceSpec {
    pub network_id: String,
    pub node_id: String,
    pub service: String,
    pub display_name: String,
    pub port: u16,
    #[serde(default)]
    pub lan_addrs: Option<Vec<SocketAddr>>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
    #[serde(default = "default_ttl_secs")]
    pub ttl_secs: u64,
}

impl AnnounceSpec {
    pub fn into_announcement(self, lan_addrs: Vec<SocketAddr>) -> NodeAnnouncement {
        NodeAnnouncement {
            network_id: self.network_id,
            node_id: self.node_id,
            service: self.service,
            display_name: self.display_name,
            port: self.port,
            lan_addrs,
            tags: self.tags,
            metadata: self.metadata,
            ttl_secs: self.ttl_secs,
        }
    }
}

pub fn default_ttl_secs() -> u64 {
    DEFAULT_TTL_SECS
}
