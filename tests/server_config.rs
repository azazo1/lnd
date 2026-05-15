use std::path::PathBuf;

use lnd::{ServerConfig, ServerConfigFile};
use tempfile::tempdir;

#[tokio::test]
async fn server_config_file_parses_toml() {
    let tempdir = tempdir().unwrap();
    let path: PathBuf = tempdir.path().join("server.toml");
    tokio::fs::write(
        &path,
        r#"
listen_addr = "127.0.0.1:9000"
bearer_token = "secret"
sse_keepalive_secs = 7
event_buffer_capacity = 123
"#,
    )
    .await
    .unwrap();

    let parsed = ServerConfig::from_toml_file(&path).await.unwrap();
    assert_eq!(parsed.listen_addr.unwrap().to_string(), "127.0.0.1:9000");
    assert_eq!(parsed.bearer_token.unwrap(), "secret");
    assert_eq!(parsed.sse_keepalive_secs.unwrap(), 7);
    assert_eq!(parsed.event_buffer_capacity.unwrap(), 123);
}

#[test]
fn server_config_merge_prefers_file_values_over_defaults() {
    let base = ServerConfig::default();
    let merged = base.merge(ServerConfigFile {
        listen_addr: Some("127.0.0.1:9999".parse().unwrap()),
        bearer_token: Some("token".to_string()),
        sse_keepalive_secs: Some(3),
        event_buffer_capacity: Some(9),
    });
    assert_eq!(merged.listen_addr.to_string(), "127.0.0.1:9999");
    assert_eq!(merged.bearer_token, "token");
    assert_eq!(merged.sse_keepalive_secs, 3);
    assert_eq!(merged.event_buffer_capacity, 9);
}
