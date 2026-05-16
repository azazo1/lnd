use futures::StreamExt;
use lnd::{DiscoveryFilter, LndClient};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = LndClient::builder("http://127.0.0.1:8765")
        .bearer_token("dev-token")
        .build()?;

    let mut stream = client.watch(
        DiscoveryFilter::new()
            .with_discovery_domain("office-a")
            .with_service("_http._tcp"),
    );
    while let Some(event) = stream.next().await {
        println!("{}", serde_json::to_string_pretty(&event?)?);
    }
    Ok(())
}
