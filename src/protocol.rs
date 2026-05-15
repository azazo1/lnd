use std::collections::BTreeMap;
use std::net::{IpAddr, SocketAddr};

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

impl DiscoveryFilter {
    pub fn new(network_id: impl Into<String>) -> Self {
        Self {
            network_id: network_id.into(),
            service: None,
            tags: Vec::new(),
        }
    }

    pub fn with_service(mut self, service: impl Into<String>) -> Self {
        self.service = Some(service.into());
        self
    }

    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    pub fn add_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }
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
    #[serde(default = "default_true")]
    pub auto_lan_addrs: bool,
    #[serde(default)]
    pub address_selection: Option<AddressSelection>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
    #[serde(default = "default_ttl_secs")]
    pub ttl_secs: u64,
}

impl AnnounceSpec {
    pub fn new(
        network_id: impl Into<String>,
        node_id: impl Into<String>,
        service: impl Into<String>,
        display_name: impl Into<String>,
        port: u16,
    ) -> Self {
        Self {
            network_id: network_id.into(),
            node_id: node_id.into(),
            service: service.into(),
            display_name: display_name.into(),
            port,
            lan_addrs: None,
            auto_lan_addrs: true,
            address_selection: None,
            tags: Vec::new(),
            metadata: BTreeMap::new(),
            ttl_secs: default_ttl_secs(),
        }
    }

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

    pub fn with_lan_addrs(mut self, lan_addrs: impl IntoIterator<Item = SocketAddr>) -> Self {
        self.lan_addrs = Some(lan_addrs.into_iter().collect());
        self
    }

    pub fn add_lan_addr(mut self, lan_addr: SocketAddr) -> Self {
        self.lan_addrs.get_or_insert_with(Vec::new).push(lan_addr);
        self
    }

    pub fn with_auto_lan_addrs(mut self, auto_lan_addrs: bool) -> Self {
        self.auto_lan_addrs = auto_lan_addrs;
        self
    }

    pub fn with_address_selection(mut self, address_selection: AddressSelection) -> Self {
        self.address_selection = Some(address_selection);
        self
    }

    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }

    pub fn add_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

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

    pub fn insert_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    pub fn with_ttl_secs(mut self, ttl_secs: u64) -> Self {
        self.ttl_secs = ttl_secs;
        self
    }

    pub fn include_loopback(mut self, include_loopback: bool) -> Self {
        self.address_selection
            .get_or_insert_with(AddressSelection::default)
            .include_loopback = include_loopback;
        self
    }

    pub fn include_ipv6(mut self, include_ipv6: bool) -> Self {
        self.address_selection
            .get_or_insert_with(AddressSelection::default)
            .include_ipv6 = include_ipv6;
        self
    }

    pub fn with_interface(mut self, interface_name: impl Into<String>) -> Self {
        self.address_selection
            .get_or_insert_with(AddressSelection::default)
            .interface_allowlist
            .push(interface_name.into());
        self
    }

    pub fn without_interface(mut self, interface_name: impl Into<String>) -> Self {
        self.address_selection
            .get_or_insert_with(AddressSelection::default)
            .interface_denylist
            .push(interface_name.into());
        self
    }
}

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
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_loopback(mut self, include_loopback: bool) -> Self {
        self.include_loopback = include_loopback;
        self
    }

    pub fn with_private_ipv4(mut self, include_private_ipv4: bool) -> Self {
        self.include_private_ipv4 = include_private_ipv4;
        self
    }

    pub fn with_link_local_ipv4(mut self, include_link_local_ipv4: bool) -> Self {
        self.include_link_local_ipv4 = include_link_local_ipv4;
        self
    }

    pub fn with_ipv6(mut self, include_ipv6: bool) -> Self {
        self.include_ipv6 = include_ipv6;
        self
    }

    pub fn with_interface(mut self, interface_name: impl Into<String>) -> Self {
        self.interface_allowlist.push(interface_name.into());
        self
    }

    pub fn without_interface(mut self, interface_name: impl Into<String>) -> Self {
        self.interface_denylist.push(interface_name.into());
        self
    }

    pub fn allows_interface(&self, interface_name: &str) -> bool {
        let allowed = self.interface_allowlist.is_empty()
            || self.interface_allowlist.iter().any(|value| value == interface_name);
        let denied = self.interface_denylist.iter().any(|value| value == interface_name);
        allowed && !denied
    }

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

pub fn default_true() -> bool {
    true
}
