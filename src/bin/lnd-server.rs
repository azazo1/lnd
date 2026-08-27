use std::path::PathBuf;

use clap::Parser;
use lnd::server::{InMemoryRegistry, ServerConfig, run_server};
use lnd::tracing_utils::init_tracing;

#[derive(Debug, Parser)]
#[command(name = "lnd-server", version)]
struct Args {
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long, env = "LND_LISTEN_ADDR")]
    listen_addr: Option<std::net::SocketAddr>,
    #[arg(long, env = "LND_BEARER_TOKEN")]
    bearer_token: Option<String>,
    #[arg(long, env = "LND_SSE_KEEPALIVE_SECS")]
    sse_keepalive_secs: Option<u64>,
    #[arg(long, env = "LND_EVENT_BUFFER_CAPACITY")]
    event_buffer_capacity: Option<usize>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let args = Args::parse();
    let mut config = ServerConfig::default();
    if let Some(path) = args.config {
        config = config.merge(ServerConfig::from_toml_file(path).await?);
    }
    if let Some(listen_addr) = args.listen_addr {
        config.listen_addr = listen_addr;
    }
    if let Some(bearer_token) = args.bearer_token {
        config.bearer_token = bearer_token;
    }
    if let Some(sse_keepalive_secs) = args.sse_keepalive_secs {
        config.sse_keepalive_secs = sse_keepalive_secs;
    }
    if let Some(event_buffer_capacity) = args.event_buffer_capacity {
        config.event_buffer_capacity = event_buffer_capacity;
    }

    run_server(
        config.clone(),
        InMemoryRegistry::new(config.event_buffer_capacity),
    )
    .await
}
