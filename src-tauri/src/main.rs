// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::Path;

fn main() {
  // `branch-deck cli <command>`, or the binary symlinked as `bd`: the agent CLI, without starting Tauri
  let mut args = std::env::args_os();
  let program = args.next();
  if program.as_deref().and_then(|program| Path::new(program).file_stem()).is_some_and(|name| name == "bd") {
    std::process::exit(bd_cli::run(args));
  }
  let mut args = args.peekable();
  if args.peek().is_some_and(|arg| arg == "cli") {
    args.next();
    std::process::exit(bd_cli::run(args));
  }
  branch_deck_lib::run();
}
