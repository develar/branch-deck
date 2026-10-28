pub mod agent_cli;
pub mod api;
pub mod auto_update;
pub mod desktop;
pub mod legacy_model_cache;
pub mod menu;
pub mod presentation;
pub mod services;
pub mod settings;
pub mod title_bar;
pub mod tracing_log;
pub mod warm_start;
pub mod zoom;

#[cfg(test)]
mod api_error_contract_test;
#[cfg(test)]
mod bindings_test;
#[cfg(test)]
mod log_rotation_test;
#[cfg(test)]
mod menu_test;
#[cfg(test)]
mod presentation_test;
#[cfg(test)]
mod title_bar_test;
#[cfg(test)]
mod tracing_log_test;
#[cfg(test)]
mod warm_start_test;
#[cfg(test)]
mod zoom_test;

use app_api::app::windows::{BuiltPage, SubWindowState, epoch_millis, open_script, settings_script, show_when_settled};
use app_api::transport::Desktop;
use auto_update::{AppUpdater, UpdateState};
use services::{AppSignIns, GitHubState, PatronusState, SignInsState, YouTrackState};
use sync_utils::span::in_current_span;
use tauri_specta::Builder;

use app_api::context::AppContext;
use app_api::repository_state::RepositoryStateCache;
use app_api::transport::WindowId;
use git_executor::git_command_executor::GitCommandExecutor;
use github::sign_in_session::GitHubSignInSession;
use menu::{TauriMenu, configure_app_menu, handle_menu_event};
use remote_facts::sign_in::SignIns;
use std::sync::Arc;
use tauri::Manager;
use tauri_plugin_window_state::StateFlags;
use title_bar::{init_title_bar, prepare_title_bar, title_bar_script, with_title_bar};
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
    // every typed command of the registry of app-api (`for_each_command!`), in its order
    .commands(api::typed_commands())
    // the fields of `appSettings` Rust reads, with their defaults: the page takes both from here, so the two never
    // disagree on a default (`src/stores/settings/appSettings.ts`)
    .typ::<domain_types::settings::AppSettings>()
    // how far a fetch or a push is, as the page shows it: the fields of `OperationEvent::Progress` of a fetch, an update
    // and a push (`src/lib/transfer-progress.ts`)
    .typ::<sync_core::transfer_progress::TransferProgress>()
    // what the start hands the main window's page before it subscribes (`window.__BD_MODEL__`, `warm_start`)
    .typ::<domain_types::session::CachedModel>()
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

  // the commands of the typed bindings, and those whose answer is raw bytes (`get_blob_syntax`), which they cannot say
  let typed_handler = ts_builder.invoke_handler();
  let raw_handler = api::raw_handler();
  builder
    .invoke_handler(move |invoke| {
      if api::RAW_COMMANDS.contains(&invoke.message.command()) {
        raw_handler(invoke)
      } else {
        typed_handler(invoke)
      }
    })
    .on_menu_event(handle_menu_event)
    // a page that loads again (a reload) starts without the subscriptions of the one before
    .on_page_load(|webview, payload| {
      if payload.event() == tauri::webview::PageLoadEvent::Started
        && let Some(ctx) = webview.try_state::<AppContext>()
      {
        ctx.forget_page(&WindowId::new(webview.label()));
      }
    })
    .setup(move |app| {
      ts_builder.mount_events(app);

      let git = GitCommandExecutor::new();
      let repository_state = RepositoryStateCache::new();
      app.manage(git.clone());
      app.manage(repository_state.clone());
      let (sessions, registry) = services::start_sessions(app.handle(), &git, &repository_state);
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
      let youtrack = YouTrackState::new(&current_version, app_data_dir.as_deref());
      app.manage(youtrack);
      let github_session = GitHubSignInSession::new(app_data_dir.as_deref().map(github::token_store::token_file));
      let sign_in_host = Arc::new(AppSignIns { app: app.handle().clone() });
      app.manage(SignInsState(SignIns::new(sign_in_host, github_session)));
      let update_state = Arc::new(std::sync::Mutex::new(UpdateState::new(current_version)));

      // the one writer of settings.json and repository-state.json: every window starts from its copy and changes it
      // through `set_setting`
      let app_data_dir = app.path().app_data_dir().unwrap_or_default();
      let settings_service = Arc::new(settings::open_settings(&app_data_dir)?);
      settings::report_save_errors(&settings_service, app.handle().clone());
      let app_settings = settings_service.app();

      // the title bar of every window, before the menu: a custom one has no native menu bar on Windows and Linux
      init_title_bar(&app_settings);
      configure_app_menu(app, &app_settings)?;

      // the state of the commands of app-api
      let services = Arc::new(services::AppServices::new(app.handle()));
      let desktop = Arc::new(desktop::TauriDesktop(app.handle().clone()));
      app.manage(AppContext {
        git,
        settings: Arc::clone(&settings_service),
        prefixes: app_settings::branch_prefixes::BranchPrefixes::default(),
        repository_state,
        // one memory of the diffs and the files of the ranges for the reviews' lists and their diffs (`get_range_files`,
        // `get_file_diffs`) and the read ahead of a branch review (`prefetch_branch_review`)
        file_diffs: file_diffs::service::FileDiffs::default(),
        symbol_indexes: Arc::new(app_api::reads::symbols::Accio),
        conflict_fix: app_api::reads::conflict_fix::ConflictFixSearch::default(),
        sessions,
        registry,
        windows: Arc::default(),
        services: Arc::clone(&services) as Arc<dyn remote_facts::service_sources::ServiceHost>,
        rewrites: Arc::clone(&services) as Arc<dyn operations::rewrite::RewriteHost>,
        events: Arc::new(api::TauriEvents(app.handle().clone())),
        host: services,
        remote: app_api::remote::RemoteMemory::default(),
        desktop: Desktop {
          windows: Arc::clone(&desktop) as Arc<dyn app_api::transport::WindowHost>,
          dialog: Arc::clone(&desktop) as Arc<dyn app_api::transport::DialogHost>,
          menu: Arc::new(TauriMenu(app.handle().clone())),
          restart: desktop,
          updater: Arc::new(AppUpdater {
            app: app.handle().clone(),
            state: update_state,
          }),
          agent_cli: Arc::new(agent_cli::AppAgentCli),
        },
        sub_windows: Arc::new(SubWindowState::default()),
        cli_tools: app_api::app::cli_tools::CliToolsState::default(),
        runtime: tauri::async_runtime::handle().inner().clone(),
      });
      app.manage(settings_service);
      let ctx = app.state::<AppContext>();

      // the settings and the title bar, before the page's own scripts
      // the main window's page catches up from the version of this snapshot itself (`get_settings_changes`): a reload
      // runs this script again
      let (settings_script, _) = settings_script(&ctx.settings, app_settings::windows::MAIN_WINDOW);
      // the rows of the selected project as its session left them, seeded before the window, whose first sync runs
      // while the webview loads (`warm_start`)
      let model_script = app_settings::projects::selected_project(&ctx.settings).map_or_default(|path| warm_start::model_script(&ctx.sessions, &path));
      // the open of the window, which its page settles once it drew its first rows (`sub_window_ready`)
      let open = ctx.sub_windows.next_open(epoch_millis());
      let init_script = format!("{settings_script} {model_script} {} {}", open_script(&open), title_bar_script());

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
      let deadline = ctx.sub_windows.built(app_settings::windows::MAIN_WINDOW, open, BuiltPage::Main);
      let window = builder.build()?;
      apply_zoom(&window, zoom);
      // built hidden (`with_title_bar`), it shows on its first frame with content, as a sub-window does: never a grey
      // canvas, nor a title row over an empty body
      prepare_title_bar(&window);
      show_when_settled(&ctx, app_settings::windows::MAIN_WINDOW.to_owned(), deadline);

      Ok(())
    })
    .build(tauri::generate_context!())
    .expect("error while running tauri application")
    .run(|app_handle, event| {
      // the app quits: the settings changed within the debounce of the writer reach the file first
      if matches!(event, tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit) {
        if let Some(service) = app_handle.try_state::<Arc<app_settings::service::SettingsService>>()
          && let Err(e) = service.flush()
        {
          tracing::error!(error = %e, "the settings changed last are lost");
        }
        return;
      }
      // a closed window: its review fetches and its symbol search are stopped, its page state forgotten
      if let tauri::RunEvent::WindowEvent {
        label,
        event: tauri::WindowEvent::Destroyed,
        ..
      } = event
      {
        app_handle.state::<AppContext>().forget_window(&WindowId::new(label.as_str()));
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
