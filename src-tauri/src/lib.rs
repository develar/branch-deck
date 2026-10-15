pub mod auto_update;
pub mod blocking;
pub mod command_error;
pub mod commands;
pub mod legacy_model_cache;
pub mod menu;
pub mod menu_state;
pub mod progress;
pub mod repository_state;

#[cfg(test)]
mod bindings_test;
#[cfg(test)]
mod repository_state_test;

use auto_update::{SharedUpdateState, UpdateState, check_for_updates, get_update_status, install_update};
use commands::add_issue_reference::add_issue_reference_to_commits;
use commands::agent_cli::{agent_cli_command, install_agent_cli};
use commands::amend_to_branch::amend_uncommitted_to_branch;
use commands::archived_branches::{delete_archived_branch, get_archived_branch_commits};
use commands::branch_prefix::{check_git_setup, get_branch_prefix_from_git_config};
use commands::commit_files::{get_commit_file_diff, get_commit_files, get_commits_files};
use commands::fetch_baseline::fetch_baseline;
use commands::menu_commands::update_menu_checkbox;
use commands::move_commits::move_commits_to_branch;
use commands::patronus::{
  PatronusState, cancel_patronus_robot, create_patronus_merge_request, get_patronus_review_requirement, get_patronus_robot_details, get_patronus_robot_problems,
  get_patronus_robot_status, get_patronus_robots, get_patronus_start_options, poll_patronus_robots, post_patronus_command, read_patronus_command_reply, restart_patronus_robot,
  start_patronus_robot,
};
use commands::pull_requests::get_pull_requests;
use commands::push::push_branch;
use commands::repository_browser::{browse_repository, validate_repository_path};
use commands::reword_commit::reword_commits;
use commands::squash_commits::{restore_branch_head, squash_commits};
use commands::squash_fixups::{get_autosquash_plan, squash_fixup_commits};
use commands::stack_operations::{rename_branch, restack_branch, undo_issue_groups};
use commands::suggest_branch_names::{BranchNameSuggestionState, suggest_branch_names};
use commands::sync_branches::sync_branches;
use commands::unapply_branch::unapply_branch;
use commands::uncommitted_changes::{get_file_content_for_diff, get_uncommitted_changes};
use commands::update_project::{dismiss_landed, update_project};
use commands::window_management::open_sub_window;
use tauri_specta::{Builder, collect_commands};

use git_executor::git_command_executor::GitCommandExecutor;
use menu::{configure_app_menu, handle_menu_event};
use menu_state::MenuState;
use repository_state::RepositoryStateCache;
use tauri::Manager;
// The log plugin is replaced by the devtools plugin in devtools builds; keep the dependency referenced.
#[cfg(feature = "devtools")]
use tauri_plugin_log as _;

/// The commands of the app, for the invoke handler and the TypeScript bindings
fn specta_builder() -> Builder<tauri::Wry> {
  // Unified serde mode: our IPC types are one-directional, so separate `_Serialize`/`_Deserialize`
  // aliases only add noise (and type `Channel<T>` arguments with the wrong phase)
  Builder::<tauri::Wry>::new().disable_serde_phases().commands(collect_commands![
    push_branch,
    get_pull_requests,
    get_patronus_robots,
    poll_patronus_robots,
    get_patronus_robot_details,
    get_patronus_start_options,
    start_patronus_robot,
    restart_patronus_robot,
    cancel_patronus_robot,
    get_patronus_robot_status,
    get_patronus_robot_problems,
    get_patronus_review_requirement,
    create_patronus_merge_request,
    post_patronus_command,
    read_patronus_command_reply,
    sync_branches,
    get_branch_prefix_from_git_config,
    check_git_setup,
    browse_repository,
    validate_repository_path,
    check_for_updates,
    get_update_status,
    install_update,
    open_sub_window,
    move_commits_to_branch,
    rename_branch,
    restack_branch,
    undo_issue_groups,
    add_issue_reference_to_commits,
    reword_commits,
    squash_commits,
    restore_branch_head,
    get_autosquash_plan,
    squash_fixup_commits,
    update_project,
    fetch_baseline,
    dismiss_landed,
    amend_uncommitted_to_branch,
    suggest_branch_names,
    get_archived_branch_commits,
    delete_archived_branch,
    unapply_branch,
    get_uncommitted_changes,
    get_file_content_for_diff,
    get_commit_files,
    get_commits_files,
    get_commit_file_diff,
    update_menu_checkbox,
    install_agent_cli,
    agent_cli_command,
  ])
}

/// Write the TypeScript bindings of `builder` (`src/lib/bindings.ts`); a debug run and `bindings_test` call it, a release
/// build has no caller
#[cfg(any(debug_assertions, test))]
fn export_bindings(builder: &Builder<tauri::Wry>, path: &std::path::Path) -> Result<(), specta_typescript::Error> {
  builder.export(specta_typescript::Typescript::default().header("// @ts-nocheck\n"), path)
}

/// How the main window draws its title bar, passed to the page as `window.__BD_TITLE_BAR__` (`src/lib/title-bar.ts`):
/// on macOS the toolbar sits in an overlay title bar beside the traffic lights, on Windows the window has no decorations
/// and the toolbar draws the window buttons, Linux keeps the native title bar above the toolbar.
#[cfg(target_os = "macos")]
const MAIN_TITLE_BAR: Option<&str> = Some("overlay");
#[cfg(target_os = "windows")]
const MAIN_TITLE_BAR: Option<&str> = Some("custom");
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const MAIN_TITLE_BAR: Option<&str> = None;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  let ts_builder = specta_builder();

  // only export on non-release builds
  #[cfg(debug_assertions)]
  export_bindings(&ts_builder, std::path::Path::new("../src/lib/bindings.ts")).expect("Failed to export TypeScript bindings");

  #[cfg(feature = "devtools")]
  let builder = tauri::Builder::default().plugin(tauri_plugin_devtools::init());

  #[cfg(not(feature = "devtools"))]
  let builder = tauri::Builder::default();

  #[cfg(not(feature = "devtools"))]
  let builder = builder.plugin(
    tauri_plugin_log::Builder::new()
      .level(log::LevelFilter::Debug)
      .filter(|metadata| {
        // Filter out logs containing default_window_icon in the message
        // This is a workaround since we can't access the message content in the filter
        // So we filter out the specific log that typically contains this data
        !(metadata.target() == "tauri::app" && metadata.level() == log::Level::Info)
      })
      .build(),
  );

  let builder = builder
    .plugin(tauri_plugin_dialog::init())
    .plugin(tauri_plugin_window_state::Builder::new().build())
    .plugin(tauri_plugin_store::Builder::new().build())
    .plugin(tauri_plugin_opener::init())
    .plugin(tauri_plugin_notification::init());

  #[cfg(feature = "auto-update")]
  let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());

  builder
    .invoke_handler(ts_builder.invoke_handler())
    .on_menu_event(handle_menu_event)
    .setup(move |app| {
      ts_builder.mount_events(app);

      app.manage(MenuState::new());
      app.manage(GitCommandExecutor::new());
      app.manage(RepositoryStateCache::new());
      app.manage(BranchNameSuggestionState::default());
      if let Ok(app_cache_dir) = app.path().app_cache_dir() {
        tauri::async_runtime::spawn_blocking(move || legacy_model_cache::remove_legacy_model_cache(&app_cache_dir));
      }

      let current_version = app.package_info().version.to_string();
      app.manage(PatronusState::new(&current_version));
      // Initialize update state
      app.manage(SharedUpdateState::new(UpdateState::new(current_version)));

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
      let init_script = match MAIN_TITLE_BAR {
        Some(title_bar) => format!("window.__TAURI_STORE__ = {store_data}; window.__BD_TITLE_BAR__ = \"{title_bar}\";"),
        None => format!("window.__TAURI_STORE__ = {store_data};"),
      };

      let builder = tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::App("index.html".into()))
        .title("BranchDeck")
        .inner_size(1024.0, 900.0)
        // the header fits one line from 960 px (`ConfigurationHeader`); narrower, its controls would overlap
        .min_inner_size(960.0, 600.0)
        .initialization_script(&init_script)
        // the native file drop handler would swallow the HTML5 drag and drop of commits (on Windows)
        .disable_drag_drop_handler();
      // the toolbar of the page is the title bar (`MAIN_TITLE_BAR`)
      #[cfg(target_os = "macos")]
      let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        // the 14 px buttons centred in the 40 px toolbar
        .traffic_light_position(tauri::LogicalPosition::new(14.0, 13.0));
      #[cfg(target_os = "windows")]
      let builder = builder.decorations(false).shadow(true);
      builder.build()?;

      Ok(())
    })
    .build(tauri::generate_context!())
    .expect("error while running tauri application")
    .run(|_app_handle, _event| {});
}
