//! The server on a free port and the repositories of its tests, for the route tests of this crate

use serde_json::{Value, json};
use std::net::SocketAddr;

pub(crate) async fn start_test_server() -> SocketAddr {
  let app = crate::create_test_app().await;
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("Failed to bind to port");
  let addr = listener.local_addr().unwrap();
  tokio::spawn(async move {
    axum::serve(listener, app).await.unwrap();
  });
  addr
}

/// A repository from the `simple` template: its id and path
pub(crate) async fn create_repository(client: &reqwest::Client, addr: SocketAddr) -> (String, String) {
  let created: Value = client
    .post(format!("http://{addr}/repositories"))
    .json(&json!({ "template": "simple" }))
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
  (created["id"].as_str().unwrap().to_string(), created["path"].as_str().unwrap().to_string())
}
