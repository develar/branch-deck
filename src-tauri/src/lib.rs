pub mod auto_update;
pub mod commands;
pub mod menu;
pub mod progress;
pub mod repository_state;

// ONNX tests disabled since ONNX is disabled
// #[cfg(test)]
// mod onnx_branch_name_generator_test;

#[cfg(test)]
mod repository_state_test;

use auto_update::{SharedUpdateState, UpdateState, check_for_updates, get_update_status, install_update};
use commands::add_issue_reference::add_issue_reference_to_commits;
use commands::archived_branches::{delete_archived_branch, get_archived_branch_commits};
use commands::branch_prefix::get_branch_prefix_from_git_config;
use commands::clear_model_cache::clear_model_cache;
use commands::create_branch::create_branch_from_commits;
use commands::push::push_branch;
use commands::repository_browser::{browse_repository, validate_repository_path};
use commands::suggest_branch_name::suggest_branch_name_stream;
use commands::sync_branches::sync_branches;
use commands::window_management::open_sub_window;
use tauri_specta::{Builder, collect_commands};

use git_executor::git_command_executor::GitCommandExecutor;
use menu::{configure_app_menu, handle_menu_event};
use repository_state::RepositoryStateCache;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  let ts_builder = Builder::<tauri::Wry>::new().commands(collect_commands![
    push_branch,
    sync_branches,
    get_branch_prefix_from_git_config,
    browse_repository,
    validate_repository_path,
    check_for_updates,
    get_update_status,
    install_update,
    open_sub_window,
    create_branch_from_commits,
    add_issue_reference_to_commits,
    suggest_branch_name_stream,
    get_archived_branch_commits,
    delete_archived_branch,
    model_tauri::commands::download_model,
    model_tauri::commands::check_model_status,
    model_tauri::commands::cancel_model_download,
    clear_model_cache,
  ]);

  // only export on non-release builds
  #[cfg(debug_assertions)]
  ts_builder
    .export(specta_typescript::Typescript::default().header("// @ts-nocheck\n"), "../app/utils/bindings.ts")
    .expect("Failed to export TypeScript bindings");

  #[cfg(feature = "devtools")]
  let builder = tauri::Builder::default().plugin(tauri_plugin_devtools::init());

  #[cfg(not(feature = "devtools"))]
  let builder = tauri::Builder::default();

  #[cfg(not(feature = "devtools"))]
  let builder = builder.plugin(
    tauri_plugin_log::Builder::new()
      .level(log::LevelFilter::Debug)
      .level_for("tokenizers", log::LevelFilter::Off)
      .level_for("candle", log::LevelFilter::Off)
      .level_for("candle_core", log::LevelFilter::Off)
      .level_for("candle_nn", log::LevelFilter::Off)
      .level_for("candle_transformers", log::LevelFilter::Off)
      .filter(|metadata| {
        // Filter out logs containing default_window_icon in the message
        // This is a workaround since we can't access the message content in the filter
        // So we filter out the specific log that typically contains this data
        if metadata.target() == "tauri::app" && metadata.level() == log::Level::Info {
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
      ts_builder.mount_events(app);

      app.manage(GitCommandExecutor::new());
      app.manage(RepositoryStateCache::new());
      app.manage(model_tauri::generator::ModelGeneratorState::new(
        model_tauri::generator::ModelBasedBranchGenerator::with_config(model_core::config::ModelConfig::default()).expect("Failed to create model-based generator"),
      ));

      let current_version = app.package_info().version.to_string();
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

      // Read settings.json for preloading
      let app_data_dir = app.path().app_data_dir().unwrap_or_default();
      let store_path = app_data_dir.join("settings.json");

      let store_data = if store_path.exists() {
        std::fs::read_to_string(&store_path).unwrap_or_else(|_| "{}".to_string())
      } else {
        "{}".to_string()
      };

      // Create main window with initialization script
      let init_script = format!("window.__TAURI_STORE__ = {store_data};");

      tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::App("index.html".into()))
        .title("BranchDeck")
        .inner_size(1024.0, 900.0)
        .initialization_script(&init_script)
        .build()?;

      Ok(())
    })
    .build(tauri::generate_context!())
    .expect("error while running tauri application")
    .run(|_app_handle, _event| {});
}
