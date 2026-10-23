use serde_json::Value;

fn capability(name: &str) -> Value {
  let path = format!("{}/capabilities/{name}.json", env!("CARGO_MANIFEST_DIR"));
  serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

fn strings(value: &Value, key: &str) -> Vec<String> {
  value[key].as_array().unwrap().iter().filter_map(|it| it.as_str().map(str::to_owned)).collect()
}

/// The title bar of a page (`with_title_bar`) needs the window permissions of `title-bar.json`: reading the window
/// state, dragging, the double click and the window buttons. E2E can't tell: the mock's window is always `main`.
#[test]
fn every_window_gets_the_permissions_of_the_title_bar() {
  let title_bar = capability("title-bar");
  let windows = strings(&title_bar, "windows");
  let mut expected = vec!["main".to_owned()];
  expected.extend(strings(&capability("sub-windows"), "windows"));
  for window in &expected {
    assert!(windows.contains(window), "{window} is not in title-bar.json");
  }
  let permissions = strings(&title_bar, "permissions");
  for permission in [
    "core:window:default",
    "core:window:allow-start-dragging",
    "core:window:allow-minimize",
    "core:window:allow-toggle-maximize",
    "core:window:allow-close",
  ] {
    assert!(permissions.iter().any(|it| it == permission), "{permission} is not in title-bar.json");
  }
}

mod decision {
  use crate::title_bar::{
    ButtonStyle, DoubleClick, NativeReason, TitleBarEnvironment, WindowButton, decoration_layout_or_default, gtk_double_click, linux_button_style, macos_double_click,
    native_reason, parse_decoration_layout,
  };
  use WindowButton::{Close, Maximize, Minimize};

  fn desktop(name: &str) -> TitleBarEnvironment {
    TitleBarEnvironment {
      current_desktop: Some(name.to_owned()),
      ..TitleBarEnvironment::default()
    }
  }

  #[test]
  fn linux_keeps_the_system_title_bar_where_intellij_does_and_where_the_user_asked() {
    assert_eq!(native_reason(&desktop("ubuntu:GNOME"), true), None);
    assert_eq!(native_reason(&desktop("KDE"), true), None);
    assert_eq!(native_reason(&TitleBarEnvironment::default(), true), Some(NativeReason::NoDesktop));
    assert_eq!(native_reason(&desktop(" "), true), Some(NativeReason::NoDesktop));
    assert_eq!(native_reason(&desktop("sway"), true), Some(NativeReason::TilingWm));
    // each part of the list, in any case
    assert_eq!(native_reason(&desktop("Hyprland"), true), Some(NativeReason::TilingWm));
    assert_eq!(native_reason(&desktop("niri:GNOME"), true), Some(NativeReason::TilingWm));
    let wsl = TitleBarEnvironment { wsl: true, ..desktop("GNOME") };
    assert_eq!(native_reason(&wsl, true), Some(NativeReason::Wsl));
    // what makes a custom one wrong comes before the setting, so the setting can say why it is fixed
    let setting = TitleBarEnvironment {
      system_title_bar_setting: true,
      ..desktop("sway")
    };
    assert_eq!(native_reason(&setting, true), Some(NativeReason::TilingWm));
    let setting = TitleBarEnvironment {
      system_title_bar_setting: true,
      ..desktop("GNOME")
    };
    assert_eq!(native_reason(&setting, true), Some(NativeReason::Setting));
    let env = TitleBarEnvironment {
      native_env: true,
      ..desktop("sway")
    };
    assert_eq!(native_reason(&env, true), Some(NativeReason::Env));
  }

  #[test]
  fn windows_keeps_the_system_title_bar_only_when_asked() {
    // the desktop variables mean nothing there
    assert_eq!(native_reason(&TitleBarEnvironment::default(), false), None);
    let setting = TitleBarEnvironment {
      system_title_bar_setting: true,
      ..TitleBarEnvironment::default()
    };
    assert_eq!(native_reason(&setting, false), Some(NativeReason::Setting));
  }

  #[test]
  fn reads_the_buttons_of_a_gtk_decoration_layout_by_its_rules() {
    assert_eq!(parse_decoration_layout("appmenu:close"), Some((vec![], vec![Close])));
    assert_eq!(parse_decoration_layout("icon:minimize,maximize,close"), Some((vec![], vec![Minimize, Maximize, Close])));
    assert_eq!(parse_decoration_layout("close,minimize,maximize:"), Some((vec![Close, Minimize, Maximize], vec![])));
    // a split layout keeps each side
    assert_eq!(parse_decoration_layout("close:minimize,maximize"), Some((vec![Close], vec![Minimize, Maximize])));
    // without a colon everything is at the start
    assert_eq!(parse_decoration_layout("close"), Some((vec![Close], vec![])));
    assert_eq!(parse_decoration_layout("menu,spacer:help, minimize ,close"), Some((vec![], vec![Minimize, Close])));
    // GTK refuses a button twice
    assert_eq!(parse_decoration_layout("close:close"), None);
    assert_eq!(parse_decoration_layout(":"), Some((vec![], vec![])));
  }

  #[test]
  fn a_missing_or_refused_layout_is_the_default_of_gtk() {
    let default = (vec![], vec![Minimize, Maximize, Close]);
    assert_eq!(decoration_layout_or_default(None), default);
    assert_eq!(decoration_layout_or_default(Some("close,close")), default);
    assert_eq!(decoration_layout_or_default(Some("appmenu:close")), (vec![], vec![Close]));
  }

  #[test]
  fn picks_the_look_of_the_buttons_by_the_icon_theme_then_the_desktop() {
    assert_eq!(linux_button_style(Some("Yaru-dark"), Some("KDE")), ButtonStyle::Gnome);
    assert_eq!(linux_button_style(Some("Adwaita"), None), ButtonStyle::Gnome);
    assert_eq!(linux_button_style(Some("breeze"), Some("GNOME")), ButtonStyle::Kde);
    assert_eq!(linux_button_style(Some("Papirus"), Some("KDE")), ButtonStyle::Kde);
    assert_eq!(linux_button_style(None, Some("XFCE")), ButtonStyle::Gnome);
  }

  #[test]
  fn rounds_the_window_as_the_desktop_does() {
    assert_eq!(crate::title_bar::linux_corner_radius(ButtonStyle::Gnome), 12);
    assert_eq!(crate::title_bar::linux_corner_radius(ButtonStyle::Kde), 8);
  }

  #[test]
  fn maps_the_double_click_setting_of_the_system() {
    assert_eq!(gtk_double_click(Some("minimize")), DoubleClick::Minimize);
    assert_eq!(gtk_double_click(Some("none")), DoubleClick::None);
    assert_eq!(gtk_double_click(Some("lower")), DoubleClick::ToggleMaximize);
    assert_eq!(gtk_double_click(None), DoubleClick::ToggleMaximize);
    assert_eq!(macos_double_click(Some("Minimize")), DoubleClick::Minimize);
    assert_eq!(macos_double_click(Some("None")), DoubleClick::None);
    assert_eq!(macos_double_click(Some("Fill")), DoubleClick::ToggleMaximize);
  }

  #[test]
  fn reads_the_stored_setting() {
    // the settings as the writer loads the file (`SettingsFile`), the defaults for one that does not parse
    let stored_system_title_bar = |text: &str| app_settings::file::SettingsFile::parse(text).map_or_default(|file| file.app).system_title_bar;
    assert!(stored_system_title_bar(r#"{"appSettings":{"systemTitleBar":true}}"#));
    assert!(!stored_system_title_bar(r#"{"appSettings":{}}"#));
    assert!(!stored_system_title_bar("not json"));
  }

  #[test]
  fn the_page_gets_one_json_object() {
    let custom = crate::title_bar::TitleBar::Custom {
      controls: crate::title_bar::WindowControls {
        start: vec![Close],
        end: vec![],
        style: ButtonStyle::Gnome,
      },
      double_click: DoubleClick::ToggleMaximize,
      check_for_updates: true,
      corner_radius: 12,
    };
    assert_eq!(
      serde_json::to_string(&custom).unwrap(),
      r#"{"kind":"custom","controls":{"start":["close"],"end":[],"style":"gnome"},"doubleClick":"toggle-maximize","checkForUpdates":true,"cornerRadius":12}"#
    );
    let native = crate::title_bar::TitleBar::Native { reason: NativeReason::TilingWm };
    assert_eq!(serde_json::to_string(&native).unwrap(), r#"{"kind":"native","reason":"tiling-wm"}"#);
  }
}

#[test]
fn the_window_state_restores_neither_the_decorations_nor_the_visibility() {
  use tauri_plugin_window_state::StateFlags;
  let flags = crate::window_state_flags();
  assert!(!flags.contains(StateFlags::DECORATIONS));
  assert!(!flags.contains(StateFlags::VISIBLE));
  assert!(flags.contains(StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED));
}
