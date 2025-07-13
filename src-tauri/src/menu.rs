use std::error::Error;
use tauri::menu::MenuItemBuilder;
use tauri::{
  App, Emitter,
  menu::{Menu, MenuEvent, PredefinedMenuItem, SubmenuBuilder},
};
use tracing::instrument;

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

  #[cfg(not(target_os = "macos"))]
  let file_menu = &SubmenuBuilder::new(app, "File").build()?;

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
      #[cfg(not(target_os = "macos"))]
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
    _ => {}
  }
}
