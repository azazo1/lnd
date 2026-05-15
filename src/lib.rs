//! `lnd` crate 的公共入口.
//!
//! 功能:
//! - 提供基于 HTTP(S) 中心注册表的局域网发现 client API.
//! - 提供可嵌入的 server 路由和内存注册表实现.
//! - 提供 C ABI 底座, 供其他语言包装更高层 SDK.
//!
//! 主要模块:
//! - [`client`]: Rust client 和地址解析工具.
//! - [`protocol`]: 请求, 响应, 事件和配置数据模型.
//! - [`server`]: Axum server, 注册表和 SSE watch 能力.
//! - [`ffi`]: C ABI.
//!
//! 最小示例:
//! ```rust
//! use lnd::{DiscoveryFilter, LndClient};
//!
//! # async fn demo() -> Result<(), lnd::client::ClientError> {
//! let client = LndClient::builder("http://127.0.0.1:8765")
//!     .bearer_token("dev-token")
//!     .build()?;
//! let nodes = client.list(DiscoveryFilter::new("office-a")).await?;
//! println!("nodes = {}", nodes.len());
//! # Ok(())
//! # }
//! ```
pub mod client;
#[allow(unsafe_op_in_unsafe_fn)]
pub mod ffi;
pub mod protocol;
pub mod server;
pub mod tracing_utils;

pub use client::{
    AnnounceHandle, ClientBuilder, ClientConfig, LndClient, default_node_id_path,
    load_or_create_node_id,
};
pub use protocol::{
    AddressSelection, AnnounceSpec, ApiErrorBody, DiscoverResponse, DiscoveredNode, DiscoveryEvent,
    DiscoveryEventEnvelope, DiscoveryFilter, LeaseInfo, NodeAnnouncement, WatchResponse,
};
pub use server::{InMemoryRegistry, ServerConfig, ServerConfigFile, build_router, run_server};
