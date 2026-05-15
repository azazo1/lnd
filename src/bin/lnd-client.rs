use std::path::PathBuf;

use anyhow::Context;
use clap::{Args, Parser, Subcommand};
use futures::StreamExt;
use lnd::client::{
    LndClient, default_display_name, default_node_id_path, load_or_create_node_id,
    metadata_from_pairs, parse_socket_addrs,
};
use lnd::protocol::{AnnounceSpec, DiscoveryFilter};
use lnd::tracing_utils::init_tracing;

#[derive(Debug, Parser)]
#[command(name = "lnd-client")]
struct Cli {
    #[arg(long, env = "LND_SERVER_URL")]
    server_url: String,
    #[arg(long, env = "LND_BEARER_TOKEN", default_value = "")]
    bearer_token: String,
    #[arg(long, default_value_t = false)]
    include_loopback: bool,
    #[arg(long, default_value_t = false)]
    include_ipv6: bool,
    #[arg(long = "enable-interface")]
    enable_interfaces: Vec<String>,
    #[arg(long = "disable-interface")]
    disable_interfaces: Vec<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Announce(AnnounceArgs),
    Discover(FilterArgs),
    Watch(FilterArgs),
}

#[derive(Debug, Args)]
struct FilterArgs {
    #[arg(long)]
    network_id: String,
    #[arg(long)]
    service: Option<String>,
    #[arg(long = "tag")]
    tags: Vec<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct AnnounceArgs {
    #[arg(long)]
    network_id: String,
    #[arg(long)]
    service: String,
    #[arg(long)]
    port: u16,
    #[arg(long, default_value_t = default_display_name())]
    display_name: String,
    #[arg(long)]
    node_id: Option<String>,
    #[arg(long)]
    node_id_path: Option<PathBuf>,
    #[arg(long = "tag")]
    tags: Vec<String>,
    #[arg(long = "metadata")]
    metadata: Vec<String>,
    #[arg(long = "lan-addr")]
    lan_addrs: Vec<String>,
    #[arg(long, default_value_t = true)]
    auto_lan_addrs: bool,
    #[arg(long, default_value_t = false)]
    include_loopback: bool,
    #[arg(long, default_value_t = false)]
    include_ipv6: bool,
    #[arg(long = "enable-interface")]
    enable_interfaces: Vec<String>,
    #[arg(long = "disable-interface")]
    disable_interfaces: Vec<String>,
    #[arg(long, default_value_t = 30)]
    ttl_secs: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let cli = Cli::parse();
    let mut builder = LndClient::builder(cli.server_url)
        .bearer_token(cli.bearer_token)
        .include_loopback(cli.include_loopback)
        .include_ipv6(cli.include_ipv6);
    for interface_name in cli.enable_interfaces {
        builder = builder.enable_interface(interface_name);
    }
    for interface_name in cli.disable_interfaces {
        builder = builder.disable_interface(interface_name);
    }
    let client = builder.build()?;

    match cli.command {
        Command::Announce(args) => announce(&client, args).await?,
        Command::Discover(args) => discover(&client, args).await?,
        Command::Watch(args) => watch(&client, args).await?,
    }

    Ok(())
}

async fn announce(client: &LndClient, args: AnnounceArgs) -> anyhow::Result<()> {
    let node_id = match args.node_id {
        Some(node_id) => node_id,
        None => {
            let path = args.node_id_path.unwrap_or_else(default_node_id_path);
            load_or_create_node_id(&path)
                .await
                .with_context(|| format!("failed to load node id from {}", path.display()))?
        }
    };
    let explicit_addrs = if args.lan_addrs.is_empty() {
        None
    } else {
        Some(parse_socket_addrs(&args.lan_addrs, args.port)?)
    };
    let spec = AnnounceSpec {
        network_id: args.network_id,
        node_id,
        service: args.service,
        display_name: args.display_name,
        port: args.port,
        lan_addrs: explicit_addrs,
        auto_lan_addrs: args.auto_lan_addrs,
        address_selection: None,
        tags: args.tags,
        metadata: metadata_from_pairs(&args.metadata)?,
        ttl_secs: args.ttl_secs,
    };
    let mut spec = if args.include_loopback {
        spec.include_loopback(true)
    } else {
        spec
    };
    if args.include_ipv6 {
        spec = spec.include_ipv6(true);
    }
    for interface_name in args.enable_interfaces {
        spec = spec.with_interface(interface_name);
    }
    for interface_name in args.disable_interfaces {
        spec = spec.without_interface(interface_name);
    }
    let initial_addrs = client.resolve_announce_addrs(&spec)?;
    let node = client.announce_once(spec.clone().into_announcement(initial_addrs)).await?;
    println!("{}", serde_json::to_string(&node)?);

    let handle = client.announce_loop(spec)?;
    tokio::signal::ctrl_c().await?;
    handle.stop().await?;
    Ok(())
}

async fn discover(client: &LndClient, args: FilterArgs) -> anyhow::Result<()> {
    let filter = DiscoveryFilter {
        network_id: args.network_id,
        service: args.service,
        tags: args.tags,
    };
    let nodes = client.list(filter).await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&nodes)?);
    } else {
        for node in nodes {
            println!(
                "{} {}:{} [{}] {}",
                node.node_id,
                node.lan_addrs
                    .first()
                    .map(|addr| addr.ip().to_string())
                    .unwrap_or_else(|| "-".to_string()),
                node.port,
                node.service,
                node.display_name
            );
        }
    }
    Ok(())
}

async fn watch(client: &LndClient, args: FilterArgs) -> anyhow::Result<()> {
    let filter = DiscoveryFilter {
        network_id: args.network_id,
        service: args.service,
        tags: args.tags,
    };
    let mut stream = client.watch(filter);
    while let Some(event) = stream.next().await {
        let event = event?;
        if args.json {
            println!("{}", serde_json::to_string(&event)?);
        } else {
            println!("{:?}", event);
        }
    }
    Ok(())
}
