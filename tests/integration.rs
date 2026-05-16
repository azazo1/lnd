mod common;

use std::time::Duration;

use common::{TestServer, sample_spec};
use futures::StreamExt;
use lnd::client::ClientConfig;
use lnd::protocol::{DiscoveryEvent, DiscoveryFilter};
use lnd::LndClient;

fn sample_filter() -> DiscoveryFilter {
    DiscoveryFilter {
        discovery_domain: Some("prod".to_string()),
        service: Some("svc".to_string()),
        tags: vec!["alpha".to_string()],
        reachability_scopes: vec!["192.168.1.0/24".to_string()],
    }
}

#[tokio::test]
async fn announce_then_list_returns_node() {
    let server = TestServer::spawn().await.unwrap();
    let client = server.client();

    client
        .announce_once(
            sample_spec("node-1", 30).into_announcement(vec!["192.168.1.10:8080".parse().unwrap()]),
        )
        .await
        .unwrap();

    let nodes = client.list(sample_filter()).await.unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].node_id, "node-1");
}

#[tokio::test]
async fn announce_loop_keeps_node_alive_until_stopped() {
    let server = TestServer::spawn().await.unwrap();
    let client = server.client();

    let handle = client.announce_loop(sample_spec("node-loop", 3)).unwrap();
    tokio::time::sleep(Duration::from_secs(4)).await;
    let nodes = client.list(sample_filter()).await.unwrap();
    assert!(nodes.iter().any(|node| node.node_id == "node-loop"));

    handle.stop().await.unwrap();
}

#[tokio::test]
async fn expired_node_is_removed() {
    let server = TestServer::spawn().await.unwrap();
    let client = server.client();

    client
        .announce_once(
            sample_spec("node-expire", 1)
                .into_announcement(vec!["192.168.1.10:8080".parse().unwrap()]),
        )
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(3)).await;

    let nodes = client.list(sample_filter()).await.unwrap();
    assert!(nodes.iter().all(|node| node.node_id != "node-expire"));
}

#[tokio::test]
async fn watch_receives_snapshot_and_updates() {
    let server = TestServer::spawn().await.unwrap();
    let client = server.client();

    client
        .announce_once(
            sample_spec("node-snapshot", 30)
                .into_announcement(vec!["192.168.1.10:8080".parse().unwrap()]),
        )
        .await
        .unwrap();

    let mut stream = client.watch(sample_filter());
    let first = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(matches!(first.event, DiscoveryEvent::Snapshot { .. }));

    client
        .announce_once(
            sample_spec("node-upsert", 30)
                .into_announcement(vec!["192.168.1.11:8080".parse().unwrap()]),
        )
        .await
        .unwrap();

    loop {
        let next = tokio::time::timeout(Duration::from_secs(5), stream.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        if let DiscoveryEvent::Upsert { node } = next.event {
            assert_eq!(node.node_id, "node-upsert");
            break;
        }
    }
}

#[tokio::test]
async fn server_shutdown_completes_with_active_watch_stream() {
    let server = TestServer::spawn().await.unwrap();
    let client = server.client();

    client
        .announce_once(
            sample_spec("node-watch-shutdown", 30)
                .into_announcement(vec!["192.168.1.10:8080".parse().unwrap()]),
        )
        .await
        .unwrap();

    let mut stream = client.watch(sample_filter());
    let first = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(matches!(first.event, DiscoveryEvent::Snapshot { .. }));

    let shutdown_result = tokio::time::timeout(
        Duration::from_secs(3),
        tokio::task::spawn_blocking(move || {
            let mut server = server;
            server.shutdown()
        }),
    )
    .await
    .unwrap()
    .unwrap();
    shutdown_result.unwrap();
}

#[tokio::test]
async fn watch_survives_idle_period_longer_than_request_timeout() {
    let server = TestServer::spawn().await.unwrap();
    let client = LndClient::new(ClientConfig {
        server_url: format!("http://{}", server.addr),
        bearer_token: server.bearer_token.clone(),
        timeout: Duration::from_secs(1),
        ..ClientConfig::default()
    })
    .unwrap();

    client
        .announce_once(
            sample_spec("node-idle-watch", 30)
                .into_announcement(vec!["192.168.1.10:8080".parse().unwrap()]),
        )
        .await
        .unwrap();

    let mut stream = client.watch(sample_filter());
    let first = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(matches!(first.event, DiscoveryEvent::Snapshot { .. }));

    tokio::time::sleep(Duration::from_secs(3)).await;

    client
        .announce_once(
            sample_spec("node-idle-watch-2", 30)
                .into_announcement(vec!["192.168.1.11:8080".parse().unwrap()]),
        )
        .await
        .unwrap();

    loop {
        let next = tokio::time::timeout(Duration::from_secs(5), stream.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        if let DiscoveryEvent::Upsert { node } = next.event {
            assert_eq!(node.node_id, "node-idle-watch-2");
            break;
        }
    }
}
