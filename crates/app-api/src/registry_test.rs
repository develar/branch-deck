//! The registry of the commands (`commands.rs`) against what the app registers: every command once, their number
//! pinned, each in the handlers of `src-tauri/src/lib.rs` (a typed one in `collect_commands!`, which the bindings are
//! exported from, a raw one in `generate_handler!`), read as text as `command_registry_test.rs` of the e2e server
//! reads them

use crate::commands::COMMANDS;
use crate::registry::CommandKind;
use std::collections::BTreeSet;
use std::path::Path;

/// The number of commands of the registry: it grows as the handwritten commands move into it (s12-app-api of
/// docs/design/rust-domain-authority.md), and proves the macro listed them all
const REGISTERED: usize = 17;

/// The names inside `<list>![...]` of `src-tauri/src/lib.rs`
fn listed(list: &str) -> BTreeSet<String> {
  let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/src/lib.rs");
  let text = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("Cannot read {}: {e}", file.display()));
  let open = format!("{list}![");
  let start = text.find(&open).unwrap_or_else(|| panic!("{list}! in src-tauri/src/lib.rs")) + open.len();
  let end = start + text[start..].find(']').expect("the end of the list");
  text[start..end].split(',').map(str::trim).filter(|name| !name.is_empty()).map(str::to_owned).collect()
}

#[test]
fn every_command_is_registered_once() {
  let names: BTreeSet<&str> = COMMANDS.iter().map(|command| command.name).collect();
  assert_eq!(names.len(), COMMANDS.len(), "the registry names a command twice");
  assert_eq!(COMMANDS.len(), REGISTERED, "a command moved into the registry: update REGISTERED");
}

#[test]
fn the_app_registers_every_command_of_the_registry() {
  let typed = listed("collect_commands");
  let raw = listed("generate_handler");
  for command in COMMANDS {
    let (handlers, list) = match command.kind {
      CommandKind::Raw => (&raw, "generate_handler!"),
      CommandKind::Json | CommandKind::Plain => (&typed, "collect_commands!"),
    };
    assert!(handlers.contains(command.name), "{}: not in {list} of src-tauri/src/lib.rs", command.name);
  }
}
