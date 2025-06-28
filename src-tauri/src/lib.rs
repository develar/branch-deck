#[macro_use]
mod commands;
mod git;
mod progress;

use commands::branch_prefix::get_branch_prefix_from_git_config;
use commands::push::push_branch;
use commands::sync_branches::sync_branches;
use tauri_specta::{Builder, collect_commands};

use git::git_command::GitCommandExecutor;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  let ts_builder = Builder::<tauri::Wry>::new().commands(collect_commands![push_branch, sync_branches, get_branch_prefix_from_git_config]);

  // only export on non-release builds
  #[cfg(debug_assertions)]
  ts_builder
    .export(specta_typescript::Typescript::default(), "../src/bindings.ts")
    .expect("Failed to export TypeScript bindings");

  #[cfg(debug_assertions)]
  let builder = tauri::Builder::default().plugin(tauri_plugin_devtools::init());
  #[cfg(not(debug_assertions))]
  let builder = tauri::Builder::default().plugin(tauri_plugin_log::Builder::new().build());

  builder
    .plugin(tauri_plugin_dialog::init())
    .plugin(tauri_plugin_updater::Builder::new().build())
    .plugin(tauri_plugin_window_state::Builder::new().build())
    .plugin(tauri_plugin_store::Builder::new().build())
    .invoke_handler(ts_builder.invoke_handler())
    .setup(move |app| {
      ts_builder.mount_events(app);

      app.manage(GitCommandExecutor::new());

      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
