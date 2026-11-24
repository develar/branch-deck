// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::ffi::{OsStr, OsString};
use std::path::Path;

use bd_core::bd_command::CLI_NAME;

/// The argument of the bridge of an IDE: `branch-deck ide`, which the app file of the IDE names
const IDE_ARG: &str = "ide";

fn main() {
  // `branch-deck cli <command>`, or the binary symlinked as `brd`: the agent CLI, without starting Tauri
  let mut args = std::env::args_os();
  let program = args.next();
  if program.as_deref().is_some_and(is_agent_cli) {
    std::process::exit(bd_cli::run(args));
  }
  let mut args = args.peekable();
  if args.peek().is_some_and(|arg| arg == "cli") {
    args.next();
    std::process::exit(bd_cli::run(args));
  }
  // `branch-deck ide`: the bridge of an IDE with Air on the stdin and the stdout, without starting Tauri; `brd mcp` with
  // the arguments after it (`-v`), its alias
  if args.peek().is_some_and(|arg| arg == IDE_ARG) {
    args.next();
    std::process::exit(bd_cli::run(bridge_args(args)));
  }
  branch_deck_lib::run();
}

/// The arguments of `brd` that run the bridge with `rest`, the arguments after `ide`
fn bridge_args(rest: impl Iterator<Item = OsString>) -> impl Iterator<Item = OsString> {
  std::iter::once(OsString::from("mcp")).chain(rest)
}

fn is_agent_cli(program: &OsStr) -> bool {
  Path::new(program).file_stem().is_some_and(|name| name == CLI_NAME)
}

#[cfg(test)]
mod tests {
  use super::{bridge_args, is_agent_cli};
  use std::ffi::{OsStr, OsString};

  #[test]
  fn branch_deck_ide_is_brd_mcp_with_the_arguments_after_it() {
    let args: Vec<OsString> = bridge_args([OsString::from("-v")].into_iter()).collect();
    assert_eq!(args, [OsString::from("mcp"), OsString::from("-v")]);
  }

  #[test]
  fn only_the_brd_executable_name_starts_the_cli() {
    assert!(is_agent_cli(OsStr::new("brd")));
    assert!(is_agent_cli(OsStr::new("brd.exe")));
    assert!(!is_agent_cli(OsStr::new("bd")));
    assert!(!is_agent_cli(OsStr::new("branch-deck")));
  }
}
