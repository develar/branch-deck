//! What a sync of the `branch_table_large` template sends the page: each `SyncEvent` as the channel of the app carries
//! it (`serde_json`), counted and measured by its type, for a first sync and for a sync that changes nothing. A
//! measurement, not a check (`tests/perf/branch-table.perf.ts` counts the same in the page):
//!
//! ```text
//! cargo nextest run -p test-server --run-ignored only -E 'test(/sync_event_bytes/)' --no-capture
//! ```

use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use sync_core::sync::{IssueConfigSource, SyncOptions};
use sync_types::{ProgressReporter, SyncEvent};

/// The count and the bytes of the events of a sync, by their type
type EventBytes = BTreeMap<String, (usize, usize)>;

/// Serializes each event as the channel of the app does and adds it to the count of its type
#[derive(Clone, Default)]
struct MeasuringReporter {
  events: Arc<Mutex<EventBytes>>,
}

impl ProgressReporter for MeasuringReporter {
  fn send(&self, event: SyncEvent) -> anyhow::Result<()> {
    let json = serde_json::to_string(&event)?;
    let value: serde_json::Value = serde_json::from_str(&json)?;
    let kind = value["type"].as_str().unwrap_or("?").to_string();
    let mut events = self.events.lock().map_err(|error| anyhow::anyhow!("the counts of a reporter: {error}"))?;
    let entry = events.entry(kind).or_default();
    entry.0 += 1;
    entry.1 += json.len();
    Ok(())
  }
}

async fn measure_sync(git_executor: &GitCommandExecutor, repository_path: &str) -> EventBytes {
  let reporter = MeasuringReporter::default();
  let options = SyncOptions {
    issue_config: IssueConfigSource::Load,
    ..Default::default()
  };
  sync_core::sync::sync_branches(git_executor, repository_path, "user-name", reporter.clone(), options)
    .await
    .expect("the sync of the template");
  reporter.events.lock().expect("the counts of a reporter").clone()
}

fn print_table(title: &str, events: &EventBytes) {
  let (count, bytes) = events.values().fold((0, 0), |(count, bytes), (n, b)| (count + n, bytes + b));
  println!("{title}: {count} events, {bytes} bytes");
  for (kind, (n, b)) in events {
    println!("  {kind:<32} {n:>5} {b:>10}");
  }
}

#[tokio::test]
#[ignore = "a measurement: prints the events of a sync of 141 branches and their bytes"]
async fn sync_event_bytes_of_branch_table_large() {
  let directory = tempfile::TempDir::new().unwrap();
  test_utils::repo_template::templates::branch_table_large().build(directory.path()).unwrap();
  let repository_path = directory.path().to_str().unwrap();
  let git_executor = GitCommandExecutor::new();

  let first = measure_sync(&git_executor, repository_path).await;
  let resync = measure_sync(&git_executor, repository_path).await;
  print_table("first sync", &first);
  print_table("resync", &resync);
  assert!(first.contains_key("branchesGrouped"), "{first:?}");
}
