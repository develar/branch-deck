pub mod auto_update;
pub mod commands;
pub mod legacy_model_cache;
pub mod lock_wait;
pub mod menu;
pub mod operation;
pub mod progress;
pub mod repository_state;
pub mod settings;
pub mod title_bar;
pub mod tracing_log;
pub mod window_requests;
pub mod zoom;

#[cfg(test)]
mod api_error_contract_test;
#[cfg(test)]
mod bindings_test;
#[cfg(test)]
mod command_scan_test;
#[cfg(test)]
mod log_rotation_test;
#[cfg(test)]
mod menu_test;
#[cfg(test)]
mod repository_state_test;
#[cfg(test)]
mod title_bar_test;
#[cfg(test)]
mod tracing_log_test;
#[cfg(test)]
mod zoom_test;

use auto_update::{SharedUpdateState, UpdateState, check_for_updates, get_update_status, install_update};
use commands::add_issue_reference::add_issue_reference_to_commits;
use commands::agent_cli::{agent_cli_command, agent_cli_status, install_agent_cli};
use commands::amend_to_branch::amend_uncommitted_to_branch;
use commands::archived_branches::{delete_archived_branch, delete_unapplied_branch, get_archived_branch_commits, get_unapplied_branch_commits};
use commands::branch_prefix::{check_git_setup, resolve_branch_prefix};
use commands::branch_review::{get_branch_review, get_file_owners, prefetch_branch_review};
use commands::cli_tools::{CliToolsState, cancel_cli_tool_action, cli_tools_status, get_setup_items, run_cli_tool_action};
use commands::commit_files::{get_commit_files, get_commits_files};
use commands::conflict_fix::{apply_conflict_fix, cancel_conflict_fix, find_conflict_fix};
use commands::fetch_baseline::fetch_baseline;
use commands::file_diffs::{SymbolSearchState, cancel_symbol_search, find_declaration, find_usages, get_blob_syntax, get_blob_texts, get_file_diffs, get_file_merges};
use commands::github::{GitHubState, cancel_github_sign_in, get_github_auth_status, sign_out_of_github, start_github_sign_in, wait_github_sign_in};
use commands::move_commits::move_commits_to_branch;
use commands::parked_branches::{park_branches, unpark_branches};
use commands::patronus::{
  PatronusState, cancel_patronus_robot, check_landing_gate, check_target_conflict, get_branch_review_readiness, get_patronus_failing_robots, get_patronus_robot_details,
  get_patronus_robot_problems, get_patronus_robot_status, get_patronus_robots, get_patronus_start_options, get_patronus_target_branch, get_robot_commit_map, get_robot_failure,
  get_robot_stats, poll_patronus_robots, post_patronus_command, read_patronus_command_reply, restart_patronus_robot, start_patronus_robot,
};
use commands::pull_requests::{
  close_pull_request, create_pull_request, get_branch_rules, get_new_pull_request, get_new_stack_pull_request, get_pull_request_head, get_pull_requests,
  request_pull_request_reviewers, retarget_pull_request, search_github_users, submit_pull_request_review, wait_for_github_branch,
};
use commands::push::{push_branch, push_branches};
use commands::repository_browser::{browse_repository, validate_repository_path};
use commands::repository_config::{get_repository_config, set_repository_config};
use commands::repository_view::{get_repository_view, wait_landing_needs};
use commands::review_requests::{ReviewFetchState, cancel_review_fetch, fetch_review_head, get_range_files, get_review_requests, prune_review_refs};
use commands::reword::reword;
use commands::settings::{get_settings_changes, set_setting};
use commands::squash_commits::{restore_branch_head, squash_commits};
use commands::squash_fixups::{get_autosquash_plan, squash_fixup_commits};
use commands::stack_operations::{get_stack_independence, rename_branch, restack_branch, undo_regroup};
use commands::suggest_branch_names::{BranchNameSuggestionState, suggest_branch_names};
use commands::sync_branches::sync_branches;
use commands::sync_inputs::get_sync_inputs_fingerprint;
use commands::unapply_branch::{reapply_branch, unapply_branch, undo_reapply_branch, undo_unapply_branch};
use commands::uncommitted_changes::get_uncommitted_changes;
use commands::update_project::{dismiss_landed, update_project};
use commands::window_management::{BuiltPage, SubWindowState, epoch_millis, open_script, open_sub_window, show_when_settled, sub_window_loaded, sub_window_ready};
use commands::window_menu::show_window_menu;
use commands::youtrack::{
  YouTrackState, cancel_youtrack_sign_in, get_youtrack_auth_status, get_youtrack_issue, save_youtrack_token, set_youtrack_issue_state, sign_out_of_youtrack,
  start_youtrack_cli_sign_in, start_youtrack_sign_in, wait_youtrack_cli_sign_in, wait_youtrack_sign_in,
};
use lock_wait::cancel_lock_wait;
use sync_utils::span::in_current_span;
use tauri_specta::{Builder, collect_commands};

use git_executor::git_command_executor::GitCommandExecutor;
use menu::{configure_app_menu, handle_menu_event, set_auto_sync_menu_checked};
use repository_state::RepositoryStateCache;
use sync_core::branch_review::BranchReviewMemo;
use tauri::Manager;
use tauri_plugin_window_state::StateFlags;
use title_bar::{init_title_bar, prepare_title_bar, restart_app, title_bar_script, with_title_bar};
use zoom::{MIN_WINDOW_HEIGHT, apply_zoom, min_window_width};
// The log plugin is replaced by the devtools plugin in devtools builds; keep the dependency referenced.
#[cfg(feature = "devtools")]
use tauri_plugin_log as _;

/// The commands of the app, for the invoke handler and the TypeScript bindings (`src/lib/bindings.ts`, which
/// `bindings_test` exports, `UPDATE_BINDINGS=1` writes)
fn specta_builder() -> Builder<tauri::Wry> {
  // Unified serde mode: our IPC types are one-directional, so separate `_Serialize`/`_Deserialize`
  // aliases only add noise (and type `Channel<T>` arguments with the wrong phase)
  Builder::<tauri::Wry>::new()
    .disable_serde_phases()
    .commands(collect_commands![
      push_branch,
      push_branches,
      cancel_lock_wait,
      get_pull_requests,
      retarget_pull_request,
      get_new_pull_request,
      get_new_stack_pull_request,
      create_pull_request,
      wait_for_github_branch,
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
      get_youtrack_auth_status,
      start_youtrack_sign_in,
      wait_youtrack_sign_in,
      cancel_youtrack_sign_in,
      save_youtrack_token,
      sign_out_of_youtrack,
      get_youtrack_issue,
      set_youtrack_issue_state,
      get_review_requests,
      fetch_review_head,
      cancel_review_fetch,
      prune_review_refs,
      get_range_files,
      get_branch_review,
      get_file_owners,
      prefetch_branch_review,
      get_patronus_robots,
      get_robot_stats,
      poll_patronus_robots,
      get_patronus_robot_details,
      get_patronus_start_options,
      start_patronus_robot,
      check_landing_gate,
      check_target_conflict,
      restart_patronus_robot,
      cancel_patronus_robot,
      get_patronus_robot_status,
      get_patronus_robot_problems,
      get_robot_failure,
      get_robot_commit_map,
      get_repository_view,
      wait_landing_needs,
      get_patronus_failing_robots,
      get_branch_review_readiness,
      post_patronus_command,
      get_patronus_target_branch,
      read_patronus_command_reply,
      sync_branches,
      get_sync_inputs_fingerprint,
      resolve_branch_prefix,
      check_git_setup,
      get_repository_config,
      set_repository_config,
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
      set_setting,
      get_settings_changes,
      move_commits_to_branch,
      find_conflict_fix,
      cancel_conflict_fix,
      apply_conflict_fix,
      rename_branch,
      restack_branch,
      park_branches,
      unpark_branches,
      get_stack_independence,
      undo_regroup,
      add_issue_reference_to_commits,
      reword,
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
      get_unapplied_branch_commits,
      delete_unapplied_branch,
      delete_archived_branch,
      unapply_branch,
      undo_unapply_branch,
      reapply_branch,
      undo_reapply_branch,
      get_uncommitted_changes,
      get_commit_files,
      get_commits_files,
      get_file_diffs,
      get_file_merges,
      get_blob_texts,
      find_declaration,
      find_usages,
      cancel_symbol_search,
      set_auto_sync_menu_checked,
      install_agent_cli,
      agent_cli_command,
      agent_cli_status,
      cli_tools_status,
      run_cli_tool_action,
      cancel_cli_tool_action,
      get_setup_items,
      start_youtrack_cli_sign_in,
      wait_youtrack_cli_sign_in,
    ])
    // the fields of `appSettings` Rust reads, with their defaults: the page takes both from here, so the two never
    // disagree on a default (`src/stores/settings/appSettings.ts`)
    .typ::<domain_types::settings::AppSettings>()
    .constant(APP_SETTINGS_DEFAULTS, domain_types::settings::AppSettings::default().to_full())
}

/// The name of the defaults of the fields of `appSettings` Rust reads in the bindings (`AppSettings::to_full` of the
/// defaults)
const APP_SETTINGS_DEFAULTS: &str = "APP_SETTINGS_DEFAULTS";

/// The retention of the app's log, the one place that describes it (the docs and the spec point here). `tauri_plugin_log`
/// (`LOG_PLUGIN_VERSION`, its `RotatingFile`) rotates when a write would push the file past `LOG_FILE_BYTES`, within a
/// session too, and when it opens a file already that large: the file is renamed with its date and a new one starts.
/// `KeepSome(n)` keeps `n` dated files beside the current one (`new` removes down to `n`, `rotate` down to `n - 1` and
/// then renames), so the log dir holds the current file and the `LOG_FILES_KEPT` before it, up to 40 MB of lines. A sync
/// of a large repository logs about 100 KB at debug level, so the lines an `operation_id` of Activity names are still
/// there days later. The plugin's default would be 40 KB and `KeepOne`, which deletes the file at the rotation: a few
/// commands' worth. `log_rotation_test.rs` pins the version this describes
#[cfg(not(feature = "devtools"))]
const LOG_FILE_BYTES: u128 = 10 * 1024 * 1024;
#[cfg(not(feature = "devtools"))]
const LOG_FILES_KEPT: usize = 3;
/// The version of `tauri_plugin_log` whose rotation the comment above describes; after an update, read `RotatingFile::new`,
/// `rotate` and `flush` of the new one, correct the comment and move this
#[cfg(test)]
const LOG_PLUGIN_VERSION: &str = "2.10.1";

/// What the window-state plugin restores: the size, the position, maximized and fullscreen; not the decorations (the page
/// draws the title bar) and not the visibility (a window shows once its page drew its content, `show_when_settled`)
fn window_state_flags() -> StateFlags {
  StateFlags::all() - StateFlags::DECORATIONS - StateFlags::VISIBLE
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[expect(clippy::large_stack_frames, reason = "the closure of `tauri::generate_context!` holds the embedded assets and config")]
pub fn run() {
  // the tokens of the sign-ins in the login keychain on macOS (`sync_utils::secret_store`)
  sync_utils::secret_store::enable_keychain();
  let ts_builder = specta_builder();

  #[cfg(feature = "devtools")]
  let builder = tauri::Builder::default().plugin(tauri_plugin_devtools::init());

  #[cfg(not(feature = "devtools"))]
  let builder = tauri::Builder::default();

  // the devtools plugin sets a subscriber of its own
  #[cfg(not(feature = "devtools"))]
  tracing_log::install();
  // the log an `operation_id` leads to, kept as `LOG_FILE_BYTES` describes
  #[cfg(not(feature = "devtools"))]
  let builder = builder.plugin(
    tauri_plugin_log::Builder::new()
      .level(log::LevelFilter::Debug)
      .max_file_size(LOG_FILE_BYTES)
      .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(LOG_FILES_KEPT))
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
    // one back on Windows; not the visibility: a window shows once its title bar is set (`prepare_title_bar`, GTK takes
    // the titlebar of a custom one only before it is shown) and its page drew its content (`sub_window_ready`)
    .plugin(tauri_plugin_window_state::Builder::new().with_state_flags(window_state_flags()).build())
    .plugin(tauri_plugin_opener::init())
    .plugin(tauri_plugin_notification::init())
    .plugin(tauri_plugin_clipboard_manager::init());

  #[cfg(feature = "auto-update")]
  let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());

  // the commands of the typed bindings, and the one whose answer is raw bytes (`get_blob_syntax`), which they cannot say
  let typed_handler = ts_builder.invoke_handler();
  let raw_invoke_handler: fn(tauri::ipc::Invoke<tauri::Wry>) -> bool = tauri::generate_handler![get_blob_syntax];
  builder
    .invoke_handler(move |invoke| {
      if invoke.message.command() == "get_blob_syntax" {
        raw_invoke_handler(invoke)
      } else {
        typed_handler(invoke)
      }
    })
    .on_menu_event(handle_menu_event)
    .setup(move |app| {
      ts_builder.mount_events(app);

      app.manage(GitCommandExecutor::new());
      app.manage(history_rewrite::lock_wait::LockWaits::default());
      app.manage(RepositoryStateCache::new());
      app.manage(repository_model::views::RepositoryViews::default());
      app.manage(app_settings::branch_prefixes::BranchPrefixes::default());
      app.manage(BranchReviewMemo::new());
      // one cache of the files of a range for the reviews' lists and their diffs (`get_range_files`, `get_file_diffs`)
      let file_diffs = file_diffs::service::FileDiffs::default();
      app.manage(file_diffs.range_files.clone());
      app.manage(file_diffs);
      app.manage(BranchNameSuggestionState::default());
      app.manage(ReviewFetchState::default());
      app.manage(SymbolSearchState::default());
      app.manage(SubWindowState::default());
      if let Ok(app_cache_dir) = app.path().app_cache_dir() {
        tauri::async_runtime::spawn_blocking(in_current_span(move || legacy_model_cache::remove_legacy_model_cache(&app_cache_dir)));
      }

      let current_version = app.package_info().version.to_string();
      app.manage(PatronusState::new(&current_version));
      let app_data_dir = app.path().app_data_dir().ok();
      app.manage(GitHubState::new(
        &current_version,
        app.state::<GitCommandExecutor>().inner().clone(),
        app_data_dir.as_deref(),
      ));
      app.manage(YouTrackState::new(&current_version, app_data_dir.as_deref()));
      app.manage(CliToolsState::new(app_data_dir.as_deref()));
      // Initialize update state
      app.manage(SharedUpdateState::new(UpdateState::new(current_version)));

      // the one writer of settings.json and repository-state.json: every window starts from its copy and changes it
      // through `set_setting`
      let app_data_dir = app.path().app_data_dir().unwrap_or_default();
      let settings_service = settings::open_settings(&app_data_dir)?;
      settings::report_save_errors(&settings_service, app.handle().clone());
      let app_settings = settings_service.app();

      // the title bar of every window, before the menu: a custom one has no native menu bar on Windows and Linux
      init_title_bar(&app_settings);
      configure_app_menu(app, &app_settings)?;

      // the settings and the title bar, before the page's own scripts
      // the main window's page catches up from the version of this snapshot itself (`get_settings_changes`): a reload
      // runs this script again
      let (settings_script, _) = settings::init_script(&settings_service, app_settings::windows::MAIN_WINDOW);
      // the open of the window, which its page settles once it drew its first rows (`sub_window_ready`)
      let windows = app.state::<SubWindowState>();
      let open = windows.next_open(epoch_millis());
      let init_script = format!("{settings_script} {} {}", open_script(&open), title_bar_script());
      app.manage(settings_service);

      let zoom = app_settings.zoom;
      let min_width = min_window_width(zoom);
      // the toolbar of the page is the title bar (`with_title_bar`)
      let builder = with_title_bar(
        tauri::WebviewWindowBuilder::new(app, app_settings::windows::MAIN_WINDOW, tauri::WebviewUrl::App("index.html".into()))
          .title("BranchDeck")
          .inner_size(min_width.max(1024.0), 900.0)
          .min_inner_size(min_width, MIN_WINDOW_HEIGHT)
          .initialization_script(&init_script)
          // the native file drop handler would swallow the HTML5 drag and drop of commits (on Windows)
          .disable_drag_drop_handler(),
      );
      // known before the page can load and say it drew its first rows
      windows.built(app_settings::windows::MAIN_WINDOW, open, BuiltPage::Main);
      let window = builder.build()?;
      apply_zoom(&window, zoom);
      // built hidden (`with_title_bar`), it shows on its first frame with content, as a sub-window does: never a grey
      // canvas, nor a title row over an empty body
      prepare_title_bar(&window);
      show_when_settled(app.handle().clone(), app_settings::windows::MAIN_WINDOW.to_owned(), open.id);

      Ok(())
    })
    .build(tauri::generate_context!())
    .expect("error while running tauri application")
    .run(|app_handle, event| {
      // the app quits: the settings changed within the debounce of the writer reach the file first
      if matches!(event, tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit) {
        if let Some(service) = app_handle.try_state::<app_settings::service::SettingsService>()
          && let Err(e) = service.flush()
        {
          tracing::error!(error = %e, "the settings changed last are lost");
        }
        return;
      }
      // a closed window: its review fetch is killed, its page state forgotten
      if let tauri::RunEvent::WindowEvent {
        label,
        event: tauri::WindowEvent::Destroyed,
        ..
      } = event
      {
        app_handle.state::<ReviewFetchState>().0.forget(&label);
        app_handle.state::<SymbolSearchState>().0.forget(&label);
        app_handle.state::<SubWindowState>().forget(&label);
        title_bar::forget_window(&label);
        if label == app_settings::windows::MAIN_WINDOW {
          close_sub_windows(app_handle);
        }
      }
    });
}

/// The main window closed: its sub-windows go with it, and the app quits with the last window. The main window runs what
/// a sub-window asks for (`sub-window-link.ts`), so a sub-window without it would wait for answers that never come.
fn close_sub_windows(app_handle: &tauri::AppHandle) {
  for (label, window) in app_handle.webview_windows() {
    if let Err(e) = window.destroy() {
      tracing::warn!(label, error = %e, "cannot close the sub-window with the main window");
    }
  }
}
