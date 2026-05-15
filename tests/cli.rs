mod common;

use assert_cmd::Command;
use common::TestServer;
use serde_json::Value;

#[tokio::test]
async fn discover_json_outputs_structured_result() {
    let server = TestServer::spawn().await.unwrap();
    let client = server.client();
    client
        .announce_once(common::sample_spec("node-cli", 30).into_announcement(vec!["192.168.1.10:8080".parse().unwrap()]))
        .await
        .unwrap();

    let mut command = Command::cargo_bin("lnd-client").unwrap();
    let output = command
        .args([
            "--server-url",
            &format!("http://{}", server.addr),
            "--bearer-token",
            &server.bearer_token,
            "discover",
            "--network-id",
            "net-a",
            "--service",
            "svc",
            "--tag",
            "alpha",
            "--json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let parsed: Value = serde_json::from_slice(&output).unwrap();
    assert!(parsed.as_array().is_some());
}

