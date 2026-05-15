use lnd::{AnnounceSpec, LndClient};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = LndClient::builder("http://127.0.0.1:8765")
        .bearer_token("dev-token")
        .include_loopback(true)
        .build()?;

    let spec = AnnounceSpec::new("example-node-a", "_demo._tcp", "example-node-a", 8080)
    .with_network_id("office-a")
    .add_tag("stable")
    .insert_metadata("version", "1.0.0")
    .include_loopback(true);

    let addrs = client.resolve_announce_addrs(&spec)?;
    let scopes = client.resolve_reachability_scopes(&spec)?;
    let mut announcement = spec.into_announcement(addrs);
    announcement.reachability_scopes = scopes;
    let node = client.announce_once(announcement).await?;
    println!("{}", serde_json::to_string_pretty(&node)?);
    Ok(())
}
