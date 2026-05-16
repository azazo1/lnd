use std::net::SocketAddr;
use std::thread::JoinHandle as ThreadJoinHandle;
use std::time::Duration;

use anyhow::Context;
use lnd::client::{ClientConfig, LndClient};
use lnd::protocol::AnnounceSpec;
use lnd::server::{InMemoryRegistry, ServerConfig, run_server_with_shutdown};
use tokio::sync::oneshot;

pub struct TestServer {
    pub addr: SocketAddr,
    pub bearer_token: String,
    shutdown_tx: Option<oneshot::Sender<()>>,
    join: Option<ThreadJoinHandle<anyhow::Result<()>>>,
}

impl TestServer {
    #[allow(dead_code)]
    pub async fn spawn() -> anyhow::Result<Self> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .context("bind test listener")?;
        let addr = listener.local_addr().context("read local addr")?;
        drop(listener);

        let bearer_token = "test-token".to_string();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let config = ServerConfig {
            listen_addr: addr,
            bearer_token: bearer_token.clone(),
            sse_keepalive_secs: 2,
            event_buffer_capacity: 64,
        };
        let join = std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().context("create test runtime")?;
            runtime.block_on(run_server_with_shutdown(
                config,
                InMemoryRegistry::new(64),
                async move {
                    let _ = shutdown_rx.await;
                },
            ))
        });
        wait_ready(addr, &bearer_token).await?;
        Ok(Self {
            addr,
            bearer_token,
            shutdown_tx: Some(shutdown_tx),
            join: Some(join),
        })
    }

    pub fn shutdown(&mut self) -> anyhow::Result<()> {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        if let Some(join) = self.join.take() {
            match join.join() {
                Ok(result) => result,
                Err(_) => anyhow::bail!("test server thread panicked"),
            }
        } else {
            Ok(())
        }
    }

    #[allow(dead_code)]
    pub fn client(&self) -> LndClient {
        LndClient::new(ClientConfig {
            server_url: format!("http://{}", self.addr),
            bearer_token: self.bearer_token.clone(),
            timeout: Duration::from_secs(5),
            ..ClientConfig::default()
        })
        .expect("client should build")
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

pub fn sample_spec(node_id: &str, ttl_secs: u64) -> AnnounceSpec {
    AnnounceSpec {
        discovery_domain: Some("prod".to_string()),
        node_id: node_id.to_string(),
        service: "svc".to_string(),
        display_name: format!("node-{node_id}"),
        port: 8080,
        lan_addrs: Some(vec!["192.168.1.10:8080".parse().unwrap()]),
        auto_lan_addrs: false,
        address_selection: None,
        reachability_scopes: Some(vec!["192.168.1.0/24".to_string()]),
        auto_reachability_scopes: false,
        tags: vec!["alpha".to_string()],
        metadata: [("role".to_string(), "api".to_string())]
            .into_iter()
            .collect(),
        ttl_secs,
    }
}

async fn wait_ready(addr: SocketAddr, bearer_token: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    for _ in 0..50 {
        if let Ok(response) = client
            .get(format!("http://{addr}/healthz"))
            .header("authorization", format!("Bearer {bearer_token}"))
            .send()
            .await
            && response.status().is_success()
        {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    anyhow::bail!("test server did not become ready");
}
