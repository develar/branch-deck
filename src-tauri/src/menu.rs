use std::error::Error;
use tauri::menu::{CheckMenuItemBuilder, MenuItemBuilder};
use tauri::{
  App, Emitter, Manager,
  menu::{Menu, MenuEvent, SubmenuBuilder},
};

#[cfg(not(target_os = "linux"))]
use tauri::menu::PredefinedMenuItem;
use tracing::instrument;

use crate::menu_state::MenuState;

#[instrument(skip(app))]
pub fn configure_app_menu(app: &mut App) -> Result<(), Box<dyn Error>> {
  #[cfg(target_os = "macos")]
  let app_name = app.package_info().name.clone();

  #[cfg(target_os = "macos")]
  let mac_menu = {
    let submenu = SubmenuBuilder::new(app, app_name).about(None).separator();
    // the macOS place of the settings of an app, ⌘,
    let settings = MenuItemBuilder::with_id(SETTINGS, "Settings…").accelerator("CmdOrCtrl+,").build(app)?;
    let submenu = submenu.item(&settings).separator();

    // Only add update menu item when auto-update feature is enabled
    #[cfg(feature = "auto-update")]
    let submenu = {
      let check_for_updates = MenuItemBuilder::with_id("check_for_updates", "Check for Updates…").build(app)?;
      submenu.item(&check_for_updates).separator()
    };

    &submenu.services().separator().hide().hide_others().show_all().separator().quit().build()?
  };

  let sync_branches = MenuItemBuilder::with_id("sync_branches", "Sync Branches").accelerator("CmdOrCtrl+R").build(app)?;
  // the IntelliJ shortcut
  let update_project = MenuItemBuilder::with_id("update_project", "Update Project").accelerator("CmdOrCtrl+T").build(app)?;
  let auto_sync_checkbox = CheckMenuItemBuilder::with_id("auto_sync_on_focus", "Auto-sync on Focus")
    .checked(false) // Updated by frontend after settings load
    .build(app)?;

  // Store checkbox reference in MenuState for programmatic access
  app.state::<MenuState>().set_auto_sync_checkbox(auto_sync_checkbox.clone());

  let file_menu_builder = SubmenuBuilder::new(app, "File")
    .item(&sync_branches)
    .item(&update_project)
    .separator()
    .item(&auto_sync_checkbox);

  // Windows and Linux: File → Settings…, Ctrl+,
  #[cfg(not(target_os = "macos"))]
  let file_menu_builder = {
    let settings = MenuItemBuilder::with_id(SETTINGS, "Settings…").accelerator("CmdOrCtrl+,").build(app)?;
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
  let view_menu_builder = SubmenuBuilder::new(app, VIEW).item(&actual_size).item(&zoom_in).item(&zoom_out);

  #[cfg(target_os = "macos")]
  let view_menu_builder = view_menu_builder.separator().item(&PredefinedMenuItem::fullscreen(app, None)?);

  let color_selector = MenuItemBuilder::with_id("color_selector", "Theme…").build(app)?;
  let view_menu_builder = view_menu_builder.separator().item(&color_selector);

  let view_menu = &view_menu_builder.build()?;

  #[cfg(target_os = "macos")]
  let window_menu = &SubmenuBuilder::new(app, "Window")
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
  let install_agent_cli = MenuItemBuilder::with_id("install_agent_cli", "Install Agent CLI…").build(app)?;
  let help_menu = &SubmenuBuilder::new(app, "Help")
    .item(&getting_started)
    .item(&install_agent_cli)
    .separator()
    .item(&github_link)
    .build()?;

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
const SETTINGS: &str = "settings";
const ZOOM_IN: &str = "Zoom In";

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
pub fn handle_menu_event(app: &tauri::AppHandle, event: MenuEvent) {
  match event.id().as_ref() {
    "check_for_updates" => {
      let result = app.emit("check_for_updates", ());
      if result.is_err() {
        tracing::error!(error = ?result.err(), "error while checking for updates");
      }
    }
    "github_link" => open_url("https://github.com/develar/branch-deck"),
    "getting_started" => open_url("https://github.com/develar/branch-deck#getting-started"),
    "install_agent_cli" => {
      // the main window shows the result
      let result = app.emit_to("main", "install_agent_cli", ());
      if result.is_err() {
        tracing::error!(error = ?result.err(), "error while triggering the agent CLI install");
      }
    }
    // one level for all windows: the main window changes it, the settings sync carries it to the others
    "zoom_in" | "zoom_out" | "zoom_reset" => {
      let result = app.emit_to("main", "zoom", &event.id().as_ref()["zoom_".len()..]);
      if result.is_err() {
        tracing::error!(error = ?result.err(), "error while triggering zoom");
      }
    }
    // the focused window opens the dialog (a conflict window too), the main one when none is focused
    SETTINGS => {
      let label = app
        .webview_windows()
        .into_iter()
        .find(|(_, window)| window.is_focused().unwrap_or(false))
        .map_or_else(|| "main".to_owned(), |(label, _)| label);
      let result = app.emit_to(label.as_str(), "open_settings", ());
      if result.is_err() {
        tracing::error!(error = ?result.err(), "error while opening the settings");
      }
    }
    "color_selector" => {
      let result = app.emit("open_color_selector", ());
      if result.is_err() {
        tracing::error!(error = ?result.err(), "error while opening color selector");
      }
    }
    "sync_branches" => {
      let result = app.emit("sync-branches", ());
      if result.is_err() {
        tracing::error!(error = ?result.err(), "error while triggering sync branches");
      }
    }
    "update_project" => {
      let result = app.emit("update-project", ());
      if result.is_err() {
        tracing::error!(error = ?result.err(), "error while triggering update project");
      }
    }
    "auto_sync_on_focus" => {
      // Get the checkbox's current state after toggle
      if let Some(menu) = app.menu()
        && let Some(item) = menu.get("auto_sync_on_focus")
        && let Some(checkbox) = item.as_check_menuitem()
      {
        match checkbox.is_checked() {
          Ok(checked) => {
            let result = app.emit("menu_auto_sync_toggled", checked);
            if result.is_err() {
              tracing::error!(error = ?result.err(), "error while emitting auto-sync toggle event");
            }
          }
          Err(e) => {
            tracing::error!(error = ?e, "error getting checkbox state");
          }
        }
      }
    }
    _ => {}
  }
}
