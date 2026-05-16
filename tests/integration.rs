mod common;

use std::time::Duration;

use common::{TestServer, sample_spec};
use futures::StreamExt;
use lnd::protocol::{DiscoveryEvent, DiscoveryFilter};

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
