//! The default Cargo entrypoint checks frontend and backend test network boundaries.
//! Node is already required for the repository's frontend checks.
mod common;

#[test]
fn test_network_policy_has_no_unregistered_listeners() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let output = std::process::Command::new("node")
        .arg(root.join("scripts/check-test-network-policy.mjs"))
        .current_dir(root)
        .output()
        .expect("Node.js is required to check the repository test network policy");
    assert!(output.status.success(), "{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
}

#[tokio::test]
#[should_panic(expected = "loopback")]
async fn ordinary_server_rejects_ipv4_wildcard_before_binding() {
    common::TestServer::start_app_on("0.0.0.0:0".parse().unwrap(), axum::Router::new()).await;
}

#[tokio::test]
#[should_panic(expected = "loopback")]
async fn ordinary_server_rejects_ipv6_wildcard_before_binding() {
    common::TestServer::start_app_on("[::]:0".parse().unwrap(), axum::Router::new()).await;
}
