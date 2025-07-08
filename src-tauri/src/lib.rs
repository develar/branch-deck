pub mod auto_update;
pub mod commands;
pub mod git;
pub mod progress;
pub mod telemetry;

#[cfg(test)]
mod test_utils;

use auto_update::{SharedUpdateState, UpdateState, check_for_updates, get_update_status, install_update};
use commands::branch_prefix::get_branch_prefix_from_git_config;
use commands::push::push_branch;
use commands::sync_branches::sync_branches;
use std::error::Error;
use tauri_specta::{Builder, collect_commands};

use git::git_command::GitCommandExecutor;
use tauri::{App, Manager, menu::{Menu, MenuEvent, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder}, Emitter};

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
    .export(specta_typescript::Typescript::default(), "../src/bindings.ts")
    .expect("Failed to export TypeScript bindings");

  #[cfg(debug_assertions)]
  let builder = tauri::Builder::default().plugin(tauri_plugin_devtools::init());
  #[cfg(not(debug_assertions))]
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

  builder
    .plugin(tauri_plugin_dialog::init())
    .plugin(tauri_plugin_updater::Builder::new().build())
    .plugin(tauri_plugin_window_state::Builder::new().build())
    .plugin(tauri_plugin_store::Builder::new().build())
    .invoke_handler(ts_builder.invoke_handler())
    .on_menu_event(handle_menu_event)
    .setup(move |app| {
      let app_name = app.package_info().name.clone();
      let current_version = app.package_info().version.to_string();

      // Initialize telemetry now that Tauri's runtime is available
      telemetry::init_telemetry(&app_name);

      ts_builder.mount_events(app);

      app.manage(GitCommandExecutor::new());

      // Initialize update state
      let update_state = UpdateState::new(current_version);
      app.manage(SharedUpdateState::new(update_state));

      configure_app_menu(app)?;

      Ok(())
    })
    .build(tauri::generate_context!())
    .expect("error while running tauri application")
    .run(|_app_handle, _event| {});
}

fn configure_app_menu(app: &mut App) -> Result<(), Box<dyn Error>> {
  let check_for_updates = MenuItemBuilder::with_id("check_for_updates", "Check for updates…").build(app)?;

  #[cfg(target_os = "macos")]
  let app_name = app.package_info().name.clone();

  #[cfg(target_os = "macos")]
  let mac_menu = &SubmenuBuilder::new(app, app_name)
    .about(None)
    .separator()
    .item(&check_for_updates)
    .separator()
    .services()
    .separator()
    .hide()
    .hide_others()
    .show_all()
    .separator()
    .quit()
    .build()?;

  #[cfg(not(target_os = "macos"))]
  let file_menu = &SubmenuBuilder::new(app, "File").build()?;

  #[cfg(not(target_os = "linux"))]
  let edit_menu = &SubmenuBuilder::new(app, "Edit")
    .items(&[
      &PredefinedMenuItem::cut(app, None)?,
      &PredefinedMenuItem::copy(app, None)?,
      &PredefinedMenuItem::paste(app, None)?,
    ])
    .build()?;

  #[cfg(target_os = "macos")]
  edit_menu.append(&PredefinedMenuItem::select_all(app, None)?)?;

  let view_menu = &SubmenuBuilder::new(app, "View").build()?;

  #[cfg(target_os = "macos")]
  view_menu.append(&PredefinedMenuItem::fullscreen(app, None)?)?;

  #[cfg(target_os = "macos")]
  let window_menu = &SubmenuBuilder::new(app, "Window")
    .items(&[
      &PredefinedMenuItem::minimize(app, None)?,
      &PredefinedMenuItem::maximize(app, None)?,
      &PredefinedMenuItem::separator(app)?,
      &PredefinedMenuItem::close_window(app, None)?,
    ])
    .build()?;

  let menu = Menu::with_items(
    app,
    &[
      #[cfg(target_os = "macos")]
      mac_menu,
      #[cfg(not(target_os = "macos"))]
      file_menu,
      #[cfg(not(target_os = "linux"))]
      edit_menu,
      view_menu,
      #[cfg(target_os = "macos")]
      window_menu,
    ],
  )?;

  app.set_menu(menu)?;
  Ok(())
}

fn handle_menu_event(app: &tauri::AppHandle, event: MenuEvent) {
  if event.id() == "check_for_updates" {
    let result = app.emit("check_for_updates", ());
    if result.is_err() {
      tracing::error!("Error while checking for updates: {:?}", result.err());
    }
  }
}