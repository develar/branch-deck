pub mod auto_update;
pub mod blocking;
pub mod command_error;
pub mod commands;
pub mod legacy_model_cache;
pub mod menu;
pub mod menu_state;
pub mod patch_cache;
pub mod progress;
pub mod repository_state;
pub mod title_bar;
pub mod zoom;

#[cfg(test)]
mod bindings_test;
#[cfg(test)]
mod patch_cache_test;
#[cfg(test)]
mod repository_state_test;
#[cfg(test)]
mod title_bar_test;
#[cfg(test)]
mod zoom_test;

use auto_update::{SharedUpdateState, UpdateState, check_for_updates, get_update_status, install_update};
use commands::add_issue_reference::add_issue_reference_to_commits;
use commands::agent_cli::{agent_cli_command, install_agent_cli};
use commands::amend_to_branch::amend_uncommitted_to_branch;
use commands::archived_branches::{delete_archived_branch, get_archived_branch_commits};
use commands::branch_prefix::{check_git_setup, get_branch_prefix_from_git_config};
use commands::branch_review::{get_branch_review, get_file_owners, prefetch_branch_review};
use commands::commit_files::{get_blob_contents, get_commit_files, get_commit_patch, get_commits_files};
use commands::conflict_fix::{apply_conflict_fix, cancel_conflict_fix, find_conflict_fix};
use commands::fetch_baseline::fetch_baseline;
use commands::github::{GitHubState, cancel_github_sign_in, get_github_auth_status, sign_out_of_github, start_github_sign_in, wait_github_sign_in};
use commands::menu_commands::update_menu_checkbox;
use commands::move_commits::move_commits_to_branch;
use commands::patronus::{
  PatronusState, cancel_patronus_robot, get_branch_review_readiness, get_patronus_failing_robots, get_patronus_robot_details, get_patronus_robot_problems,
  get_patronus_robot_status, get_patronus_robots, get_patronus_start_options, get_patronus_target_branch, get_robot_failure, poll_patronus_robots, post_patronus_command,
  read_patronus_command_reply, restart_patronus_robot, start_patronus_robot,
};
use commands::pull_requests::{
  close_pull_request, create_pull_request, get_branch_rules, get_new_pull_request, get_pull_request_head, get_pull_requests, request_pull_request_reviewers, retarget_pull_request,
  search_github_users, submit_pull_request_review,
};
use commands::push::push_branch;
use commands::repository_browser::{browse_repository, validate_repository_path};
use commands::review_requests::{ReviewFetchState, cancel_review_fetch, fetch_review_head, get_range_files, get_range_patch, get_review_requests, prune_review_refs};
use commands::reword_commit::reword_commits;
use commands::squash_commits::{restore_branch_head, squash_commits};
use commands::squash_fixups::{get_autosquash_plan, squash_fixup_commits};
use commands::stack_operations::{get_stack_independence, rename_branch, restack_branch, undo_regroup};
use commands::suggest_branch_names::{BranchNameSuggestionState, suggest_branch_names};
use commands::sync_branches::sync_branches;
use commands::sync_inputs::get_sync_inputs_fingerprint;
use commands::unapply_branch::unapply_branch;
use commands::uncommitted_changes::{get_file_content_for_diff, get_uncommitted_changes};
use commands::update_project::{dismiss_landed, update_project};
use commands::window_management::{SubWindowState, open_sub_window, sub_window_loaded, sub_window_ready};
use commands::window_menu::{restart_app, show_window_menu};
use tauri_specta::{Builder, collect_commands};

use git_executor::git_command_executor::GitCommandExecutor;
use menu::{configure_app_menu, handle_menu_event};
use menu_state::MenuState;
use patch_cache::PatchCache;
use repository_state::RepositoryStateCache;
use sync_core::branch_review::BranchReviewMemo;
use tauri::Manager;
use tauri_plugin_window_state::StateFlags;
use title_bar::{finish_title_bar, init_title_bar, title_bar_script, with_title_bar};
use zoom::{MIN_WINDOW_HEIGHT, apply_zoom, min_window_width, stored_zoom};
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
    retarget_pull_request,
    get_new_pull_request,
    create_pull_request,
    search_github_users,
    get_branch_rules,
    submit_pull_request_review,
    get_pull_request_head,
    close_pull_request,
    request_pull_request_reviewers,
    get_github_auth_status,
    start_github_sign_in,
    wait_github_sign_in,
    cancel_github_sign_in,
    sign_out_of_github,
    get_review_requests,
    fetch_review_head,
    cancel_review_fetch,
    prune_review_refs,
    get_range_files,
    get_range_patch,
    get_branch_review,
    get_file_owners,
    prefetch_branch_review,
    get_patronus_robots,
    poll_patronus_robots,
    get_patronus_robot_details,
    get_patronus_start_options,
    start_patronus_robot,
    restart_patronus_robot,
    cancel_patronus_robot,
    get_patronus_robot_status,
    get_patronus_robot_problems,
    get_robot_failure,
    get_patronus_failing_robots,
    get_branch_review_readiness,
    post_patronus_command,
    get_patronus_target_branch,
    read_patronus_command_reply,
    sync_branches,
    get_sync_inputs_fingerprint,
    get_branch_prefix_from_git_config,
    check_git_setup,
    browse_repository,
    validate_repository_path,
    check_for_updates,
    get_update_status,
    install_update,
    open_sub_window,
    sub_window_loaded,
    sub_window_ready,
    show_window_menu,
    restart_app,
    move_commits_to_branch,
    find_conflict_fix,
    cancel_conflict_fix,
    apply_conflict_fix,
    rename_branch,
    restack_branch,
    get_stack_independence,
    undo_regroup,
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
    get_commit_patch,
    get_blob_contents,
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

/// What the window-state plugin restores: the size, the position, maximized and fullscreen; not the decorations (the page
/// draws the title bar) and not the visibility (`finish_title_bar` shows a window)
fn window_state_flags() -> StateFlags {
  StateFlags::all() - StateFlags::DECORATIONS - StateFlags::VISIBLE
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[expect(clippy::large_stack_frames, reason = "the closure of `tauri::generate_context!` holds the embedded assets and config")]
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
    // not the decorations: the page draws the title bar (`with_title_bar`), a saved `decorated` would bring the native
    // one back on Windows; not the visibility: a window shows once its title bar is set (`finish_title_bar`, GTK takes
    // the titlebar of a custom one only before it is shown) and a sub-window once its page painted (`sub_window_ready`)
    .plugin(tauri_plugin_window_state::Builder::new().with_state_flags(window_state_flags()).build())
    .plugin(tauri_plugin_store::Builder::new().build())
    .plugin(tauri_plugin_opener::init())
    .plugin(tauri_plugin_notification::init())
    .plugin(tauri_plugin_clipboard_manager::init());

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
      app.manage(BranchReviewMemo::new());
      app.manage(PatchCache::new(
        app.state::<GitCommandExecutor>().inner().clone(),
        app.state::<RepositoryStateCache>().inner().clone(),
      ));
      app.manage(BranchNameSuggestionState::default());
      app.manage(ReviewFetchState::default());
      app.manage(SubWindowState::default());
      if let Ok(app_cache_dir) = app.path().app_cache_dir() {
        tauri::async_runtime::spawn_blocking(move || legacy_model_cache::remove_legacy_model_cache(&app_cache_dir));
      }

      let current_version = app.package_info().version.to_string();
      app.manage(PatronusState::new(&current_version));
      let app_data_dir = app.path().app_data_dir().ok();
      app.manage(GitHubState::new(
        &current_version,
        app.state::<GitCommandExecutor>().inner().clone(),
        app_data_dir.as_deref(),
      ));
      // Initialize update state
      app.manage(SharedUpdateState::new(UpdateState::new(current_version)));

      // Read settings.json for preloading
      let app_data_dir = app.path().app_data_dir().unwrap_or_default();
      let store_path = app_data_dir.join("settings.json");

      let store_data = if store_path.exists() {
        std::fs::read_to_string(&store_path).unwrap_or_else(|_| "{}".to_string())
      } else {
        "{}".to_string()
      };

      // the title bar of every window, before the menu: a custom one has no native menu bar on Windows and Linux
      init_title_bar(&store_data);
      configure_app_menu(app)?;

      // Create main window with initialization script
      let init_script = format!("window.__TAURI_STORE__ = {store_data}; {}", title_bar_script());

      let zoom = stored_zoom(&store_data);
      let min_width = min_window_width(zoom);
      // the toolbar of the page is the title bar (`with_title_bar`)
      let builder = with_title_bar(
        tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::App("index.html".into()))
          .title("BranchDeck")
          .inner_size(min_width.max(1024.0), 900.0)
          .min_inner_size(min_width, MIN_WINDOW_HEIGHT)
          .initialization_script(&init_script)
          // the native file drop handler would swallow the HTML5 drag and drop of commits (on Windows)
          .disable_drag_drop_handler(),
      );
      let window = builder.build()?;
      apply_zoom(&window, zoom);
      finish_title_bar(&window);

      Ok(())
    })
    .build(tauri::generate_context!())
    .expect("error while running tauri application")
    .run(|app_handle, event| {
      // a closed window: its review fetch is killed, its page state forgotten
      if let tauri::RunEvent::WindowEvent {
        label,
        event: tauri::WindowEvent::Destroyed,
        ..
      } = event
      {
        app_handle.state::<ReviewFetchState>().forget(&label);
        app_handle.state::<SubWindowState>().forget(&label);
        title_bar::forget_window(&label);
      }
    });
}
