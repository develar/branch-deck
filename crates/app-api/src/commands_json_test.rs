//! The table of the commands that the page mock of the e2e server reads (`crates/app-api/commands.json`, inlined into
//! its init script by `tests/e2e/setup/tauri-mocks.ts`): each command of the registry with its class and how it answers,
//! JSON, raw bytes, or on the channel its entry names (the argument of the page that gets the events, and whether it
//! stays open after the answer). Written from `COMMANDS` with `UPDATE_COMMANDS=1`, compared by value otherwise, so a
//! stale table fails

use crate::commands::COMMANDS;
use crate::registry::{CommandClass, CommandKind, CommandSpec};
use pretty_assertions::assert_eq;
use serde::Serialize;
use std::path::Path;

/// A command as the page mock sends it
#[derive(Serialize)]
struct PageCommand {
  name: &'static str,
  class: &'static str,
  transport: Transport,
}

/// How a command answers the page
#[derive(Serialize)]
#[serde(untagged)]
enum Transport {
  /// `json` or `raw`
  Answer(&'static str),
  /// Its events on the channel argument `channel` of the page, before the answer, and after it while `open`
  Channel { channel: &'static str, open: bool },
}

fn page_command(spec: &CommandSpec) -> PageCommand {
  let class = match spec.class {
    CommandClass::Read => "read",
    CommandClass::Engine => "engine",
    CommandClass::App(_) => "app",
  };
  let transport = match (&spec.channel, spec.kind) {
    (Some(channel), _) => Transport::Channel {
      channel: channel.name,
      open: channel.open,
    },
    (None, CommandKind::Raw) => Transport::Answer("raw"),
    (None, _) => Transport::Answer("json"),
  };
  PageCommand {
    name: spec.name,
    class,
    transport,
  }
}

/// The table of the registry, in its order
fn table() -> Vec<PageCommand> {
  COMMANDS.iter().map(page_command).collect()
}

#[test]
fn the_commands_json_of_the_page_mock_is_the_registry() {
  let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("commands.json");
  if std::env::var_os("UPDATE_COMMANDS").is_some() {
    let mut written = serde_json::to_string_pretty(&table()).expect("the table serializes");
    written.push('\n');
    std::fs::write(&path, written).expect("the table is written");
    return;
  }
  let expected = serde_json::to_value(table()).expect("the table serializes");
  // by value: the formatter of the repository lays the file out its own way
  let text = std::fs::read_to_string(&path).unwrap_or_default();
  assert_eq!(
    serde_json::from_str::<serde_json::Value>(&text).ok(),
    Some(expected),
    "crates/app-api/commands.json is stale: UPDATE_COMMANDS=1 cargo nextest run -p app-api -E 'test(/commands_json/)' writes it"
  );
}

#[test]
fn a_channel_of_the_table_names_the_argument_of_the_page_and_whether_it_stays_open() {
  let commands = serde_json::to_value(table()).expect("the table serializes");
  let of = |name: &str| commands.as_array().into_iter().flatten().find(|command| command["name"] == name).cloned().expect(name);
  assert_eq!(of("subscribe_view")["transport"], serde_json::json!({ "channel": "patches", "open": true }));
  assert_eq!(of("get_file_diffs")["transport"], serde_json::json!({ "channel": "events", "open": false }));
  assert_eq!(of("execute")["class"], "engine");
  assert_eq!(of("get_blob_syntax")["transport"], "raw");
  assert_eq!(of("find_declaration")["transport"], "json");
}
