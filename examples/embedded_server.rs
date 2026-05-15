use lnd::{InMemoryRegistry, ServerConfig, run_server};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = ServerConfig {
        listen_addr: "127.0.0.1:8765".parse()?,
        bearer_token: "dev-token".to_string(),
        ..ServerConfig::default()
    };
    let registry = InMemoryRegistry::new(config.event_buffer_capacity);
    run_server(config, registry).await
}
