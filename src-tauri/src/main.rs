// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::ffi::OsStr;
use std::path::Path;

use bd_core::bd_command::CLI_NAME;

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
  branch_deck_lib::run();
}

fn is_agent_cli(program: &OsStr) -> bool {
  Path::new(program).file_stem().is_some_and(|name| name == CLI_NAME)
}

#[cfg(test)]
mod tests {
  use super::is_agent_cli;
  use std::ffi::OsStr;

  #[test]
  fn only_the_brd_executable_name_starts_the_cli() {
    assert!(is_agent_cli(OsStr::new("brd")));
    assert!(is_agent_cli(OsStr::new("brd.exe")));
    assert!(!is_agent_cli(OsStr::new("bd")));
    assert!(!is_agent_cli(OsStr::new("branch-deck")));
  }
}
