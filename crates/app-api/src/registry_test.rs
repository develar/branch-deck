//! The registry of the commands (`commands.rs`): every command once, their number pinned, every write with its class.
//! Each host registers what the registry lists, generated from the same `for_each_command!` (the app's wrappers and
//! handlers, `src-tauri/src/api.rs`; the routes of the e2e server, `crates/test-server/src/invoke.rs`), and
//! `command_registry_test.rs` of the e2e server holds the list, `src/lib/bindings.ts` and its router to one set

use crate::commands::COMMANDS;
use crate::registry::CommandClass;
use std::collections::BTreeSet;

/// The number of commands of the registry, every command of the app: a new command raises it, and it proves the macro
/// listed them all
const COMMAND_COUNT: usize = 78;

#[test]
fn every_command_is_registered_once() {
  let names: BTreeSet<&str> = COMMANDS.iter().map(|command| command.name).collect();
  assert_eq!(names.len(), COMMANDS.len(), "the registry names a command twice");
  assert_eq!(
    COMMANDS.len(),
    COMMAND_COUNT,
    "a command was added to the registry or removed from it: update COMMAND_COUNT"
  );
}

/// The exit criterion of s11 (docs/design/rust-domain-authority.md), now the grammar's: a write is the engine's or a
/// command of the app with the reason no session admits it (`app(<reason>)`, which the grammar requires), and the
/// engine's writes are its two commands, the intents and their cancel
#[test]
fn the_writes_of_the_engine_are_its_intents_and_their_cancel() {
  let engine: Vec<&str> = COMMANDS
    .iter()
    .filter(|command| command.class == CommandClass::Engine)
    .map(|command| command.name)
    .collect();
  assert_eq!(engine, ["execute", "cancel_operation"], "a write of a repository is an intent of `execute`");
}
