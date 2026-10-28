use crate::api::TauriEvents;
use app_api::operation::operation_now;
use app_api::transport::{AppEvents, WindowId};
use domain_types::error::ApiError;
use domain_types::settings::AppSettings;
use std::error::Error;
use tauri::menu::{CheckMenuItemBuilder, MenuItemBuilder};
use tauri::{
  App, AppHandle,
  menu::{Menu, MenuEvent, SubmenuBuilder},
};

#[cfg(not(target_os = "linux"))]
use tauri::menu::PredefinedMenuItem;
use tracing::instrument;

/// The menu of the app: the macOS app menu; the menu bar of each window on Windows and Linux where the system title bar
/// stays. A custom title bar on Windows and Linux has none: the page's main menu (`src/lib/main-menu.ts`, ☰ in the
/// title bar) holds the same items and the page handles their keys, so no menu bar sits above the title bar and no
/// accelerator table takes the keys.
///
/// A menu event goes to the main window, named as the command it runs there (`AppCommand` of `src/lib/app-commands.ts`).
#[instrument(skip(app, settings))]
pub fn configure_app_menu(app: &mut App, settings: &AppSettings) -> Result<(), Box<dyn Error>> {
  #[cfg(not(target_os = "macos"))]
  if crate::title_bar::is_custom_title_bar() {
    return Ok(());
  }

  #[cfg(target_os = "macos")]
  let app_name = app.package_info().name.clone();

  #[cfg(target_os = "macos")]
  let mac_menu = {
    let submenu = SubmenuBuilder::new(app, app_name).about(None).separator();
    // the macOS place of the settings of an app, ⌘,
    let settings = MenuItemBuilder::with_id(OPEN_SETTINGS, "Settings…").accelerator("CmdOrCtrl+,").build(app)?;
    let submenu = submenu.item(&settings).separator();

    // Only add update menu item when auto-update feature is enabled
    #[cfg(feature = "auto-update")]
    let submenu = {
      let check_for_updates = MenuItemBuilder::with_id(CHECK_FOR_UPDATES, "Check for Updates…").build(app)?;
      submenu.item(&check_for_updates).separator()
    };

    &submenu.services().separator().hide().hide_others().show_all().separator().quit().build()?
  };

  let sync_branches = MenuItemBuilder::with_id(SYNC_BRANCHES, "Sync Branches").accelerator("CmdOrCtrl+R").build(app)?;
  // the IntelliJ shortcut
  let update_project = MenuItemBuilder::with_id(UPDATE_PROJECT, "Update Project").accelerator("CmdOrCtrl+T").build(app)?;
  // the main window keeps it checked as the setting is (`set_auto_sync_menu_checked`)
  let auto_sync_checkbox = CheckMenuItemBuilder::with_id(AUTO_SYNC_ON_FOCUS, "Auto-sync on Focus")
    .checked(settings.auto_sync_on_focus)
    .build(app)?;

  let file_menu_builder = SubmenuBuilder::new(app, "File")
    .item(&sync_branches)
    .item(&update_project)
    .separator()
    .item(&auto_sync_checkbox);

  // Windows and Linux: File → Settings…, Ctrl+,
  #[cfg(not(target_os = "macos"))]
  let file_menu_builder = {
    let settings = MenuItemBuilder::with_id(OPEN_SETTINGS, "Settings…").accelerator("CmdOrCtrl+,").build(app)?;
    file_menu_builder.separator().item(&settings)
  };

  let file_menu = &file_menu_builder.build()?;

  #[cfg(not(target_os = "linux"))]
  let edit_menu_builder = SubmenuBuilder::new(app, "Edit").items(&[
    &PredefinedMenuItem::cut(app, None)?,
    &PredefinedMenuItem::copy(app, None)?,
    &PredefinedMenuItem::paste(app, None)?,
  ]);

  #[cfg(target_os = "macos")]
  let edit_menu_builder = edit_menu_builder.item(&PredefinedMenuItem::select_all(app, None)?);

  #[cfg(not(target_os = "linux"))]
  let edit_menu = &edit_menu_builder.build()?;

  // in the order of Safari; the page zooms itself (`useZoom`), WKWebView has no zoom keys
  let actual_size = MenuItemBuilder::with_id("zoom_reset", "Actual Size").accelerator("CmdOrCtrl+0").build(app)?;
  // ⌘+ on macOS (`show_zoom_in_as_plus`)
  let zoom_in = MenuItemBuilder::with_id("zoom_in", ZOOM_IN).accelerator("CmdOrCtrl+=").build(app)?;
  let zoom_out = MenuItemBuilder::with_id("zoom_out", "Zoom Out").accelerator("CmdOrCtrl+-").build(app)?;
  // the command palette of the main window (⌘K): its views, Activity, the commands of the app, a branch
  let command_palette = MenuItemBuilder::with_id(OPEN_COMMAND_PALETTE, "Command Palette…").accelerator("CmdOrCtrl+K").build(app)?;
  // the views of the main window (`MAIN_VIEWS` of `src/stores/main-view.ts`)
  let show_branches = MenuItemBuilder::with_id(SHOW_BRANCHES, "Branches").accelerator("CmdOrCtrl+1").build(app)?;
  let show_reviews = MenuItemBuilder::with_id(SHOW_REVIEWS, "Reviews").accelerator("CmdOrCtrl+2").build(app)?;
  let show_landing = MenuItemBuilder::with_id(SHOW_LANDING, "Landing").accelerator("CmdOrCtrl+3").build(app)?;
  let view_menu_builder = SubmenuBuilder::new(app, VIEW)
    .item(&command_palette)
    .separator()
    .item(&show_branches)
    .item(&show_reviews)
    .item(&show_landing)
    .separator()
    .item(&actual_size)
    .item(&zoom_in)
    .item(&zoom_out);

  #[cfg(target_os = "macos")]
  let view_menu_builder = view_menu_builder.separator().item(&PredefinedMenuItem::fullscreen(app, None)?);

  // Windows and Linux: View → Activity, the window of the log (macOS has it in the Window menu)
  #[cfg(not(target_os = "macos"))]
  let view_menu_builder = view_menu_builder.separator().item(&MenuItemBuilder::with_id(OPEN_ACTIVITY, "Activity").build(app)?);

  let view_menu = &view_menu_builder.build()?;

  // the window of the log above the windows of the system
  #[cfg(target_os = "macos")]
  let window_menu = &SubmenuBuilder::new(app, "Window")
    .item(&MenuItemBuilder::with_id(OPEN_ACTIVITY, "Activity").build(app)?)
    .separator()
    .items(&[
      &PredefinedMenuItem::minimize(app, None)?,
      &PredefinedMenuItem::maximize(app, None)?,
      &PredefinedMenuItem::separator(app)?,
      &PredefinedMenuItem::close_window(app, None)?,
    ])
    .build()?;

  let getting_started = MenuItemBuilder::with_id("getting_started", "Getting Started").build(app)?;
  let github_link = MenuItemBuilder::with_id("github_link", "GitHub Repository").build(app)?;
  // `bd install`: the agent CLI on PATH and the skill that tells coding agents about it
  let install_agent_cli = MenuItemBuilder::with_id(INSTALL_AGENT_CLI, "Install Agent CLI…").build(app)?;
  let help_menu_builder = SubmenuBuilder::new(app, "Help").item(&getting_started).item(&install_agent_cli);
  // macOS has it in the app menu
  #[cfg(all(feature = "auto-update", not(target_os = "macos")))]
  let help_menu_builder = help_menu_builder.item(&MenuItemBuilder::with_id(CHECK_FOR_UPDATES, "Check for Updates…").build(app)?);
  let help_menu = &help_menu_builder.separator().item(&github_link).build()?;

  let menu = Menu::with_items(
    app,
    &[
      #[cfg(target_os = "macos")]
      mac_menu,
      file_menu,
      #[cfg(not(target_os = "linux"))]
      edit_menu,
      view_menu,
      #[cfg(target_os = "macos")]
      window_menu,
      help_menu,
    ],
  )?;

  app.set_menu(menu)?;
  #[cfg(target_os = "macos")]
  show_zoom_in_as_plus();
  Ok(())
}

const VIEW: &str = "View";
const ZOOM_IN: &str = "Zoom In";
// the items whose event is their id: the command of the main window; the main window opens the Settings window, as
// it has the current settings to start it from, and the Activity window, as it owns the log
const OPEN_SETTINGS: &str = "open_settings";
const OPEN_ACTIVITY: &str = "open_activity";
const OPEN_COMMAND_PALETTE: &str = "open_command_palette";
const SHOW_BRANCHES: &str = "show_branches";
const SHOW_REVIEWS: &str = "show_reviews";
const SHOW_LANDING: &str = "show_landing";
const SYNC_BRANCHES: &str = "sync_branches";
const UPDATE_PROJECT: &str = "update_project";
const INSTALL_AGENT_CLI: &str = "install_agent_cli";
const CHECK_FOR_UPDATES: &str = "check_for_updates";
const AUTO_SYNC_ON_FOCUS: &str = "auto_sync_on_focus";

/// Checks Auto-sync on Focus of the native menu as the setting is, whichever window changed it: the main window calls it
/// for every value of `appSettings.autoSyncOnFocus` (`useMenuSync`). Without a native menu (a custom title bar on
/// Windows and Linux, whose main menu reads the setting itself) there is nothing to check.
#[tauri::command]
#[specta::specta]
#[instrument(skip(app))]
#[expect(clippy::needless_pass_by_value, reason = "tauri passes the app handle of a command by value")]
pub fn set_auto_sync_menu_checked(app: AppHandle, checked: bool) -> Result<(), ApiError> {
  operation_now(|| {
    let Some(checkbox) = app.menu().and_then(|menu| menu.get(AUTO_SYNC_ON_FOCUS)).and_then(|item| item.as_check_menuitem().cloned()) else {
      return Ok(());
    };
    checkbox
      .set_checked(checked)
      .map_err(|e| ApiError::internal(format!("Cannot check Auto-sync on Focus: {e}")))
  })
}

/// Sends a menu event to the main window, which runs it
fn emit_to_main(app: &AppHandle, event: &str, payload: impl serde::Serialize) {
  match serde_json::to_value(payload) {
    Ok(payload) => TauriEvents(app.clone()).emit_to(&WindowId::main(), event, payload),
    Err(e) => tracing::error!(error = ?e, event, "cannot send a menu event to the main window"),
  }
}

/// Zoom In is `⌘+` on macOS, as in Safari and Electron apps. Tauri builds accelerators from key codes only, and `+` is
/// no key but `⇧=` on most layouts, so the key equivalent is set on the native item: `AppKit` shows `⌘+` and takes `⌘⇧=`
/// (and `⌘+` of the keypad), the page takes `⌘=` (`useZoom`).
#[cfg(target_os = "macos")]
fn show_zoom_in_as_plus() {
  use objc2::MainThreadMarker;
  use objc2_app_kit::NSApplication;
  use objc2_foundation::NSString;

  // the setup runs on the main thread
  let Some(mtm) = MainThreadMarker::new() else {
    tracing::warn!("the menu is built off the main thread, Zoom In keeps ⌘=");
    return;
  };
  let item = NSApplication::sharedApplication(mtm)
    .mainMenu()
    .and_then(|menu| menu.itemWithTitle(&NSString::from_str(VIEW)))
    .and_then(|view| view.submenu())
    .and_then(|view| view.itemWithTitle(&NSString::from_str(ZOOM_IN)));
  if let Some(item) = item {
    // the modifier mask stays ⌘
    item.setKeyEquivalent(&NSString::from_str("+"));
  } else {
    tracing::warn!("Zoom In is not in the View menu, it keeps ⌘=");
  }
}

fn open_url(url: &str) {
  if let Err(e) = tauri_plugin_opener::open_url(url, None::<String>) {
    tracing::error!(error = ?e, url, "failed to open a link");
  }
}

#[expect(clippy::needless_pass_by_value, reason = "signature is dictated by tauri's on_menu_event callback")]
#[instrument(skip(app), fields(menu_id = ?event.id()))]
pub fn handle_menu_event(app: &AppHandle, event: MenuEvent) {
  match event.id().as_ref() {
    "github_link" => open_url("https://github.com/develar/branch-deck"),
    "getting_started" => open_url("https://github.com/develar/branch-deck#getting-started"),
    id @ (SYNC_BRANCHES | UPDATE_PROJECT | OPEN_SETTINGS | OPEN_ACTIVITY | OPEN_COMMAND_PALETTE | SHOW_BRANCHES | SHOW_REVIEWS | SHOW_LANDING | INSTALL_AGENT_CLI
    | CHECK_FOR_UPDATES) => {
      emit_to_main(app, id, ());
    }
    // one level for all windows: the main window changes it, the settings sync carries it to the others
    id @ ("zoom_in" | "zoom_out" | "zoom_reset") => emit_to_main(app, "zoom", &id["zoom_".len()..]),
    // the item checked itself; the main window saves the setting, and its sync reaches the other windows
    AUTO_SYNC_ON_FOCUS => {
      let checked = app
        .menu()
        .and_then(|menu| menu.get(AUTO_SYNC_ON_FOCUS))
        .and_then(|item| item.as_check_menuitem()?.is_checked().ok());
      if let Some(checked) = checked {
        emit_to_main(app, "auto_sync_on_focus_toggled", checked);
      } else {
        tracing::error!("cannot read the state of Auto-sync on Focus");
      }
    }
    _ => {}
  }
}
