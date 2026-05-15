use clap::Parser;
use lnd::server::{InMemoryRegistry, ServerConfig, run_server};
use lnd::tracing_utils::init_tracing;

#[derive(Debug, Parser)]
#[command(name = "lnd-server")]
struct Args {
    #[arg(long, env = "LND_LISTEN_ADDR", default_value = "0.0.0.0:8765")]
    listen_addr: std::net::SocketAddr,
    #[arg(long, env = "LND_BEARER_TOKEN", default_value = "")]
    bearer_token: String,
    #[arg(long, env = "LND_SSE_KEEPALIVE_SECS", default_value_t = 15)]
    sse_keepalive_secs: u64,
    #[arg(long, env = "LND_EVENT_BUFFER_CAPACITY", default_value_t = 4096)]
    event_buffer_capacity: usize,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let args = Args::parse();
    let config = ServerConfig {
        listen_addr: args.listen_addr,
        bearer_token: args.bearer_token,
        sse_keepalive_secs: args.sse_keepalive_secs,
        event_buffer_capacity: args.event_buffer_capacity,
    };
    run_server(config.clone(), InMemoryRegistry::new(config.event_buffer_capacity)).await
}

