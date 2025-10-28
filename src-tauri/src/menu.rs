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

    // Only add update menu item when auto-update feature is enabled
    #[cfg(feature = "auto-update")]
    let submenu = {
      let check_for_updates = MenuItemBuilder::with_id("check_for_updates", "Check for updates…").build(app)?;
      submenu.item(&check_for_updates).separator()
    };

    &submenu.services().separator().hide().hide_others().show_all().separator().quit().build()?
  };

  let sync_branches = MenuItemBuilder::with_id("sync_branches", "Sync Branches").accelerator("CmdOrCtrl+R").build(app)?;
  let auto_sync_checkbox = CheckMenuItemBuilder::with_id("auto_sync_on_focus", "Auto-sync on Focus")
    .checked(false) // Updated by frontend after settings load
    .build(app)?;

  // Store checkbox reference in MenuState for programmatic access
  let menu_state = app.state::<MenuState>();
  let checkbox_clone = auto_sync_checkbox.clone();
  tauri::async_runtime::block_on(async move {
    *menu_state.auto_sync_checkbox.write().await = Some(checkbox_clone);
  });

  let file_menu = &SubmenuBuilder::new(app, "File").item(&sync_branches).separator().item(&auto_sync_checkbox).build()?;

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

  let view_menu_builder = SubmenuBuilder::new(app, "View");

  #[cfg(target_os = "macos")]
  let view_menu_builder = view_menu_builder.item(&PredefinedMenuItem::fullscreen(app, None)?);

  let color_selector = MenuItemBuilder::with_id("color_selector", "Primary Color...").build(app)?;
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

  let github_link = MenuItemBuilder::with_id("github_link", "GitHub Repository").build(app)?;
  let help_menu = &SubmenuBuilder::new(app, "Help").item(&github_link).build()?;

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
  Ok(())
}

#[instrument(skip(app), fields(menu_id = ?event.id()))]
pub fn handle_menu_event(app: &tauri::AppHandle, event: MenuEvent) {
  match event.id().as_ref() {
    "check_for_updates" => {
      let result = app.emit("check_for_updates", ());
      if result.is_err() {
        tracing::error!(error = ?result.err(), "error while checking for updates");
      }
    }
    "github_link" => {
      let url = "https://github.com/develar/branch-deck";
      if let Err(e) = tauri_plugin_opener::open_url(url, None::<String>) {
        tracing::error!(error = ?e, "failed to open GitHub link");
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
