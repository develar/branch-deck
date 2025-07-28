use pretty_assertions::assert_eq;
use std::net::SocketAddr;
use std::time::Duration;

async fn start_test_server() -> (SocketAddr, tokio::task::JoinHandle<()>) {
  let app = test_server::create_test_app().await;

  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("Failed to bind to port");
  let addr = listener.local_addr().unwrap();

  let handle = tokio::spawn(async move {
    axum::serve(listener, app).await.unwrap();
  });

  // Give server time to start
  tokio::time::sleep(Duration::from_millis(100)).await;

  (addr, handle)
}

#[tokio::test]
async fn test_sync_branches_sse_stream() {
  // Initialize test environment
  let _ = tracing_subscriber::fmt::try_init();

  // Start test server
  let (addr, _server_handle) = start_test_server().await;

  // Create a test repository using test-utils
  let temp_dir = tempfile::TempDir::new().unwrap();
  let repo_path = temp_dir.path();

  // Build a simple test repository
  test_utils::templates::simple().build(repo_path).expect("Failed to create test repository");

  // Create HTTP client
  let client = reqwest::Client::new();

  // First, register the repository with the test server
  let create_response = client
    .post(format!("http://{addr}/repositories"))
    .json(&serde_json::json!({
        "template": "simple"
    }))
    .send()
    .await
    .expect("Failed to create repository");

  assert_eq!(create_response.status(), 200);
  let create_result: serde_json::Value = create_response.json().await.unwrap();
  let registered_path = create_result["path"].as_str().unwrap();

  // Make SSE request to sync_branches
  let response = client
    .post(format!("http://{addr}/invoke/sync_branches"))
    .header("Content-Type", "application/json")
    .header("Accept", "text/event-stream")
    .json(&serde_json::json!({
        "repositoryPath": registered_path,
        "branchPrefix": "user-name"
    }))
    .send()
    .await
    .expect("Failed to send sync request");

  assert_eq!(response.status(), 200);

  // Read SSE stream
  let mut events = Vec::new();
  let mut stream = response.bytes_stream();
  let mut buffer = String::new();

  // Set a timeout for reading events
  let start = std::time::Instant::now();
  let timeout = Duration::from_secs(5);

  use futures::StreamExt;
  while let Ok(Some(chunk)) = tokio::time::timeout(Duration::from_millis(100), stream.next()).await {
    if start.elapsed() > timeout {
      break;
    }

    let chunk = chunk.expect("Failed to read chunk");
    buffer.push_str(&String::from_utf8_lossy(&chunk));

    // Parse SSE events from buffer
    while let Some(event_end) = buffer.find("\n\n") {
      let event_data = buffer[..event_end].to_string();
      buffer = buffer[event_end + 2..].to_string();

      for line in event_data.lines() {
        if let Some(data) = line.strip_prefix("data: ") {
          if let Ok(event) = serde_json::from_str::<serde_json::Value>(data) {
            println!(
              "Received event: type={}, data={:?}",
              event.get("type").and_then(|v| v.as_str()).unwrap_or("?"),
              event.get("data")
            );
            events.push(event);
          }
        }
      }
    }
  }

  // Verify we got events
  assert!(!events.is_empty(), "Should receive at least one event");

  // Check for expected event types
  let event_types: Vec<&str> = events.iter().filter_map(|e| e["type"].as_str()).collect();

  println!("Event types received: {event_types:?}");

  // Progress events are optional - they might not be sent for very fast syncs
  assert!(event_types.contains(&"branchesGrouped"), "Should have branchesGrouped event");
  assert!(event_types.contains(&"branchStatusUpdate"), "Should have branchStatusUpdate events");
  assert!(event_types.contains(&"completed"), "Should have completed event");

  // Verify branchesGrouped contains our test branch
  let branches_grouped = events.iter().find(|e| e["type"] == "branchesGrouped").expect("Should have branchesGrouped event");

  let branches = &branches_grouped["data"]["branches"];
  assert!(branches.is_array(), "Branches should be an array");

  let branch_names: Vec<&str> = branches.as_array().unwrap().iter().filter_map(|b| b["name"].as_str()).collect();

  println!("Branch names found: {branch_names:?}");
  assert!(branch_names.contains(&"test-branch"), "Should have test-branch");

  // Verify we got branchStatusUpdate events for each branch
  let status_updates: Vec<&serde_json::Value> = events.iter().filter(|e| e["type"] == "branchStatusUpdate").collect();

  assert!(!status_updates.is_empty(), "Should have branchStatusUpdate events");

  // Check that we have status updates for test-branch
  let test_branch_status = status_updates.iter().find(|e| e["data"]["branchName"] == "test-branch");

  assert!(test_branch_status.is_some(), "Should have status update for test-branch");

  // The status should be one of the valid states (not "Syncing")
  let status = test_branch_status.unwrap()["data"]["status"].as_str().unwrap();
  println!("test-branch status: {status}");
  assert!(
    ["Created", "Updated", "Unchanged", "Error", "MergeConflict"].contains(&status),
    "Branch status should be a final status, not 'Syncing'"
  );
}

#[tokio::test]
async fn test_sse_format() {
  // Test that we can parse SSE format correctly
  let sse_data = r#"event: sync
data: {"type":"branchesGrouped","data":{"branches":[]}}

event: sync
data: {"type":"completed"}

"#;

  let mut events = Vec::new();
  for chunk in sse_data.split("\n\n") {
    for line in chunk.lines() {
      if let Some(data) = line.strip_prefix("data: ") {
        if let Ok(event) = serde_json::from_str::<serde_json::Value>(data) {
          events.push(event);
        }
      }
    }
  }

  assert_eq!(events.len(), 2);
  assert_eq!(events[0]["type"], "branchesGrouped");
  assert_eq!(events[1]["type"], "completed");
}
