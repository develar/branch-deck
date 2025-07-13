pub mod auto_update;
pub mod commands;
pub mod git;
pub mod menu;
pub mod progress;
pub mod telemetry;

#[cfg(test)]
mod test_utils;

use auto_update::{SharedUpdateState, UpdateState, check_for_updates, get_update_status, install_update};
use commands::branch_prefix::get_branch_prefix_from_git_config;
use commands::push::push_branch;
use commands::sync_branches::sync_branches;
use tauri_specta::{Builder, collect_commands};

use git::git_command::GitCommandExecutor;
use menu::{configure_app_menu, handle_menu_event};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  let ts_builder = Builder::<tauri::Wry>::new().commands(collect_commands![
    push_branch,
    sync_branches,
    get_branch_prefix_from_git_config,
    check_for_updates,
    get_update_status,
    install_update,
  ]);

  // only export on non-release builds
  #[cfg(debug_assertions)]
  ts_builder
    .export(specta_typescript::Typescript::default(), "../app/utils/bindings.ts")
    .expect("Failed to export TypeScript bindings");

  #[cfg(debug_assertions)]
  // let builder = tauri::Builder::default().plugin(tauri_plugin_devtools::init());
  // #[cfg(not(debug_assertions))]
  let builder = tauri::Builder::default().plugin(
    tauri_plugin_log::Builder::new()
      .filter(|metadata| {
        // Filter out logs containing default_window_icon in the message
        // This is a workaround since we can't access the message content in the filter
        // So we filter out the specific log that typically contains this data
        if metadata.target() == "tauri::app" && metadata.level() == tracing::log::Level::Info {
          return false;
        }
        true
      })
      .build(),
  );

  let builder = builder
    .plugin(tauri_plugin_dialog::init())
    .plugin(tauri_plugin_window_state::Builder::new().build())
    .plugin(tauri_plugin_store::Builder::new().build())
    .plugin(tauri_plugin_opener::init());

  #[cfg(feature = "auto-update")]
  let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());

  builder
    .invoke_handler(ts_builder.invoke_handler())
    .on_menu_event(handle_menu_event)
    .setup(move |app| {
      // let app_name = app.package_info().name.clone();
      let current_version = app.package_info().version.to_string();

      // Initialize telemetry now that Tauri's runtime is available
      // telemetry::init_telemetry(&app_name);

      ts_builder.mount_events(app);

      app.manage(GitCommandExecutor::new());

      // Initialize update state
      #[cfg(feature = "auto-update")]
      {
        let update_state = UpdateState::new(current_version);
        app.manage(SharedUpdateState::new(update_state));
      }
      #[cfg(not(feature = "auto-update"))]
      {
        let update_state = UpdateState::new(current_version.clone());
        app.manage(SharedUpdateState::new(update_state));
      }

      configure_app_menu(app)?;

      Ok(())
    })
    .build(tauri::generate_context!())
    .expect("error while running tauri application")
    .run(|_app_handle, _event| {});
}
