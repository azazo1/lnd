use std::path::{Path, PathBuf};

use anyhow::Context;
use clap::{Args, Parser, Subcommand};
use futures::StreamExt;
use lnd::client::{LndClient, metadata_from_pairs, parse_socket_addrs};
use lnd::protocol::{AnnounceSpec, DiscoveryFilter};
use lnd::tracing_utils::init_tracing;

#[derive(Debug, Parser)]
#[command(name = "lnd-client", version)]
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
    discovery_domain: Option<String>,
    #[arg(long, default_value_t = true)]
    auto_scope_overlap: bool,
    #[arg(long = "scope")]
    reachability_scopes: Vec<String>,
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
    discovery_domain: Option<String>,
    #[arg(long, default_value_t = true)]
    auto_reachability_scopes: bool,
    #[arg(long = "scope")]
    reachability_scopes: Vec<String>,
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

fn default_node_id_path() -> PathBuf {
    let base = dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir);
    base.join("lnd").join("node_id")
}

fn default_display_name() -> String {
    hostname::get()
        .ok()
        .and_then(|value| value.into_string().ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "lnd-node".to_string())
}

async fn load_or_create_node_id(path: &Path) -> anyhow::Result<String> {
    match tokio::fs::read_to_string(path).await {
        Ok(value) => Ok(value.trim().to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            let node_id = uuid::Uuid::new_v4().to_string();
            tokio::fs::write(path, format!("{node_id}\n")).await?;
            Ok(node_id)
        }
        Err(error) => Err(error.into()),
    }
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
        discovery_domain: args.discovery_domain,
        node_id,
        service: args.service,
        display_name: args.display_name,
        port: args.port,
        lan_addrs: explicit_addrs,
        auto_lan_addrs: args.auto_lan_addrs,
        address_selection: None,
        reachability_scopes: if args.reachability_scopes.is_empty() {
            None
        } else {
            Some(args.reachability_scopes)
        },
        auto_reachability_scopes: args.auto_reachability_scopes,
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
    let node = client
        .announce_once(spec.clone().into_announcement(initial_addrs))
        .await?;
    println!("{}", serde_json::to_string(&node)?);

    let handle = client.announce_loop(spec)?;
    wait_for_stop_signal().await?;
    handle.stop().await?;
    Ok(())
}

async fn discover(client: &LndClient, args: FilterArgs) -> anyhow::Result<()> {
    let mut filter = DiscoveryFilter::new();
    if let Some(discovery_domain) = args.discovery_domain {
        filter = filter.with_discovery_domain(discovery_domain);
    }
    if let Some(service) = args.service {
        filter = filter.with_service(service);
    }
    filter = filter.with_tags(args.tags);
    if args.auto_scope_overlap {
        for scope in client.list_reachability_scopes()? {
            filter = filter.add_reachability_scope(scope.scope);
        }
    }
    for scope in args.reachability_scopes {
        filter = filter.add_reachability_scope(scope);
    }
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
    let mut filter = DiscoveryFilter::new();
    if let Some(discovery_domain) = args.discovery_domain {
        filter = filter.with_discovery_domain(discovery_domain);
    }
    if let Some(service) = args.service {
        filter = filter.with_service(service);
    }
    filter = filter.with_tags(args.tags);
    if args.auto_scope_overlap {
        for scope in client.list_reachability_scopes()? {
            filter = filter.add_reachability_scope(scope.scope);
        }
    }
    for scope in args.reachability_scopes {
        filter = filter.add_reachability_scope(scope);
    }
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

async fn wait_for_stop_signal() -> anyhow::Result<()> {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await
    };
    #[cfg(unix)]
    let terminate = async {
        let mut signal = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        signal.recv().await;
        Ok::<(), std::io::Error>(())
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<Result<(), std::io::Error>>();
    tokio::select! {
        result = ctrl_c => result?,
        result = terminate => result?,
    }
    Ok(())
}
