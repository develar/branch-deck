use domain_types::settings::AppSettings;
use serde::Serialize;
use std::sync::OnceLock;
use tauri::{Manager, Runtime, WebviewWindow, WebviewWindowBuilder};

/// How every window draws its title bar, decided once per process (`init_title_bar`) and passed to the page as
/// `window.__BD_TITLE_BAR__` (`src/lib/title-bar.ts`):
/// - `Overlay` on macOS: the page's top row sits in an overlay title bar beside the traffic lights;
/// - `Custom` on Windows and Linux: the row is the title bar and draws the window buttons (Windows: no decorations;
///   Linux: GTK client-side decorations with an empty titlebar, so GTK still draws the shadow and the resize edges);
/// - `Native` where the system title bar stays above the row: on Linux where a custom one doesn't fit (WSL, a tiling
///   window manager, an unknown desktop), and wherever the user asked for it (the setting, the environment variable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum TitleBar {
  /// `check_for_updates`: the build updates itself (the `auto-update` feature), so the command palette offers Check for
  /// Updates… as the app menu does
  Overlay {
    double_click: DoubleClick,
    check_for_updates: bool,
  },
  /// `check_for_updates`: the build updates itself (the `auto-update` feature), so the main menu and the command palette
  /// offer Check for Updates… as the native menu would. `corner_radius`: the page rounds the corners of the window as
  /// the desktop's windows are (Linux with a compositor: GTK3 rounds its decorations but doesn't clip the webview, so the
  /// window is transparent and the page clips its canvas), 0 where the system does it (Windows 11) or can't (no
  /// compositor)
  Custom {
    controls: WindowControls,
    double_click: DoubleClick,
    check_for_updates: bool,
    corner_radius: u32,
  },
  Native {
    reason: NativeReason,
  },
}

/// Why the system title bar stays, for the log and for the setting, which says it
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NativeReason {
  /// `BRANCH_DECK_NATIVE_TITLE_BAR=1`
  Env,
  /// `WSLg` draws its own frame
  Wsl,
  /// No `XDG_CURRENT_DESKTOP`: nothing tells how the desktop decorates its windows
  NoDesktop,
  /// A tiling window manager draws the titles itself
  TilingWm,
  /// "Use the system title bar" (`appSettings.systemTitleBar`)
  Setting,
}

/// The window buttons of a custom title bar, in the order and on the sides the desktop shows them
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WindowControls {
  pub start: Vec<WindowButton>,
  pub end: Vec<WindowButton>,
  pub style: ButtonStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowButton {
  Minimize,
  Maximize,
  Close,
}

/// How the buttons look: the Windows caption buttons, GNOME's (Adwaita) or KDE's (Breeze), as IntelliJ draws them
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ButtonStyle {
  Windows,
  Gnome,
  Kde,
}

/// What a double click on the empty parts of the title bar does, as the system setting says
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DoubleClick {
  ToggleMaximize,
  Minimize,
  None,
}

/// The tiling window managers of IntelliJ (`X11UiUtil.TILE_WM`), and niri
const TILING_WINDOW_MANAGERS: &[&str] = &[
  "awesome",
  "bspwm",
  "cagebreak",
  "compiz",
  "dwl",
  "dwm",
  "frankenwm",
  "herbstluftwm",
  "hyprland",
  "i3",
  "ion",
  "larswm",
  "leftwm",
  "niri",
  "notion",
  "qtile",
  "ratpoison",
  "river",
  "snapwm",
  "spectrwm",
  "stumpwm",
  "sway",
  "wmii",
  "xmonad",
];

/// What the decision reads, apart from the platform: the environment and the stored setting
#[derive(Debug, Default, Clone)]
pub struct TitleBarEnvironment {
  /// `BRANCH_DECK_NATIVE_TITLE_BAR=1`
  pub native_env: bool,
  /// `appSettings.systemTitleBar`
  pub system_title_bar_setting: bool,
  /// `WSL_DISTRO_NAME` or `WSL_INTEROP` (Linux)
  pub wsl: bool,
  /// `XDG_CURRENT_DESKTOP` (Linux)
  pub current_desktop: Option<String>,
}

/// Why the system title bar stays, `None` for a custom one. The environment variable first, then what makes a custom
/// title bar wrong on Linux (so the setting can say why it is fixed), then the setting.
pub fn native_reason(environment: &TitleBarEnvironment, linux: bool) -> Option<NativeReason> {
  if environment.native_env {
    return Some(NativeReason::Env);
  }
  if linux {
    if environment.wsl {
      return Some(NativeReason::Wsl);
    }
    let Some(desktop) = environment.current_desktop.as_deref().filter(|it| !it.trim().is_empty()) else {
      return Some(NativeReason::NoDesktop);
    };
    let tiling = desktop.split(':').any(|part| TILING_WINDOW_MANAGERS.contains(&part.trim().to_lowercase().as_str()));
    if tiling {
      return Some(NativeReason::TilingWm);
    }
  }
  if environment.system_title_bar_setting {
    return Some(NativeReason::Setting);
  }
  None
}

/// The buttons of a GTK decoration layout (`gtk-decoration-layout`, `appmenu:minimize,maximize,close`), by GTK's rules:
/// the start before the first `:`, the end after it (everything at the start without a colon); `icon`, `menu`,
/// `appmenu`, `spacer` and unknown names are dropped. `None` for a layout GTK refuses (a button twice).
pub fn parse_decoration_layout(layout: &str) -> Option<(Vec<WindowButton>, Vec<WindowButton>)> {
  let (start, end) = layout.split_once(':').unwrap_or((layout, ""));
  let side = |names: &str| -> Vec<WindowButton> {
    names
      .split(',')
      .filter_map(|name| match name.trim() {
        "minimize" => Some(WindowButton::Minimize),
        "maximize" => Some(WindowButton::Maximize),
        "close" => Some(WindowButton::Close),
        _ => None,
      })
      .collect()
  };
  let (start, end) = (side(start), side(end));
  let mut seen = Vec::new();
  for button in start.iter().chain(end.iter()) {
    if seen.contains(button) {
      return None;
    }
    seen.push(*button);
  }
  Some((start, end))
}

/// The buttons of a layout, the default of GTK (`:minimize,maximize,close`) for a missing or refused one
pub fn decoration_layout_or_default(layout: Option<&str>) -> (Vec<WindowButton>, Vec<WindowButton>) {
  layout
    .and_then(parse_decoration_layout)
    .unwrap_or_else(|| (Vec::new(), vec![WindowButton::Minimize, WindowButton::Maximize, WindowButton::Close]))
}

/// The look of the buttons on Linux, IntelliJ's rule (`LinuxFrameButton.updateIconTheme`): by the icon theme, else by the
/// desktop
pub fn linux_button_style(icon_theme: Option<&str>, current_desktop: Option<&str>) -> ButtonStyle {
  let theme = icon_theme.unwrap_or_default().to_lowercase();
  if theme.starts_with("adwaita") || theme.starts_with("yaru") {
    return ButtonStyle::Gnome;
  }
  if theme.starts_with("breeze") {
    return ButtonStyle::Kde;
  }
  if current_desktop.is_some_and(|it| it.to_uppercase().contains("KDE")) {
    ButtonStyle::Kde
  } else {
    ButtonStyle::Gnome
  }
}

/// The corner radius of a window on the desktop: libadwaita's 12px on GNOME, Breeze's 8px on KDE
pub const fn linux_corner_radius(style: ButtonStyle) -> u32 {
  match style {
    ButtonStyle::Kde => 8,
    ButtonStyle::Gnome | ButtonStyle::Windows => 12,
  }
}

/// The corner radius the page rounds a window with, 0 for none (`TitleBar::Custom::corner_radius`)
pub fn corner_radius() -> u32 {
  match title_bar() {
    TitleBar::Custom { corner_radius, .. } => *corner_radius,
    _ => 0,
  }
}

/// GTK's `gtk-titlebar-double-click` (GNOME's `action-double-click-titlebar`); what a page can't do (`lower`, `menu`)
/// maximizes, as the default does
pub fn gtk_double_click(action: Option<&str>) -> DoubleClick {
  match action {
    Some("minimize") => DoubleClick::Minimize,
    Some("none") => DoubleClick::None,
    _ => DoubleClick::ToggleMaximize,
  }
}

/// macOS's `AppleActionOnDoubleClick` (Desktop & Dock → "Double-click a window's title bar to"); "Fill" and "Maximize"
/// zoom
pub fn macos_double_click(action: Option<&str>) -> DoubleClick {
  match action {
    Some("Minimize") => DoubleClick::Minimize,
    Some("None") => DoubleClick::None,
    _ => DoubleClick::ToggleMaximize,
  }
}

static TITLE_BAR: OnceLock<TitleBar> = OnceLock::new();

/// Decides the title bar of the process, once, in `setup`: on the main thread, where GTK can be read, before the menu
/// and the first window
pub fn init_title_bar(settings: &AppSettings) -> &'static TitleBar {
  TITLE_BAR.get_or_init(|| {
    let environment = TitleBarEnvironment {
      native_env: std::env::var("BRANCH_DECK_NATIVE_TITLE_BAR").is_ok_and(|it| it == "1"),
      system_title_bar_setting: settings.system_title_bar,
      wsl: std::env::var_os("WSL_DISTRO_NAME").is_some() || std::env::var_os("WSL_INTEROP").is_some(),
      current_desktop: std::env::var("XDG_CURRENT_DESKTOP").ok(),
    };
    let title_bar = platform_title_bar(&environment);
    tracing::info!(?title_bar, "title bar");
    title_bar
  })
}

/// The decided title bar; `Native` before `init_title_bar` (never in the app: setup decides it first)
pub fn title_bar() -> &'static TitleBar {
  static UNDECIDED: TitleBar = TitleBar::Native { reason: NativeReason::Env };
  TITLE_BAR.get().unwrap_or(&UNDECIDED)
}

/// The page draws the window buttons and the main menu: there is no native menu bar on Windows and Linux
pub fn is_custom_title_bar() -> bool {
  matches!(title_bar(), TitleBar::Custom { .. })
}

#[cfg(target_os = "macos")]
fn platform_title_bar(_environment: &TitleBarEnvironment) -> TitleBar {
  use objc2_foundation::{NSUserDefaults, ns_string};
  let defaults = NSUserDefaults::standardUserDefaults();
  let action = defaults.stringForKey(ns_string!("AppleActionOnDoubleClick")).map(|it| it.to_string());
  TitleBar::Overlay {
    double_click: macos_double_click(action.as_deref()),
    check_for_updates: cfg!(feature = "auto-update"),
  }
}

#[cfg(target_os = "windows")]
fn platform_title_bar(environment: &TitleBarEnvironment) -> TitleBar {
  match native_reason(environment, false) {
    Some(reason) => TitleBar::Native { reason },
    None => TitleBar::Custom {
      controls: WindowControls {
        start: Vec::new(),
        end: vec![WindowButton::Minimize, WindowButton::Maximize, WindowButton::Close],
        style: ButtonStyle::Windows,
      },
      double_click: DoubleClick::ToggleMaximize,
      check_for_updates: cfg!(feature = "auto-update"),
      // Windows 11 rounds the window itself (`shadow(true)`)
      corner_radius: 0,
    },
  }
}

#[cfg(target_os = "linux")]
fn platform_title_bar(environment: &TitleBarEnvironment) -> TitleBar {
  if let Some(reason) = native_reason(environment, true) {
    return TitleBar::Native { reason };
  }
  use gtk::prelude::GtkSettingsExt;
  let settings = gtk::Settings::default();
  let layout = settings.as_ref().and_then(GtkSettingsExt::gtk_decoration_layout);
  let icon_theme = settings.as_ref().and_then(GtkSettingsExt::gtk_icon_theme_name);
  let double_click = settings.as_ref().and_then(GtkSettingsExt::gtk_titlebar_double_click);
  let (start, end) = decoration_layout_or_default(layout.as_deref());
  let style = linux_button_style(icon_theme.as_deref(), environment.current_desktop.as_deref());
  // a transparent window needs a compositor; without one GTK draws a solid frame and the corners stay square
  let composited = gtk::gdk::Screen::default().is_some_and(|screen| screen.is_composited());
  TitleBar::Custom {
    controls: WindowControls { start, end, style },
    double_click: gtk_double_click(double_click.as_deref()),
    check_for_updates: cfg!(feature = "auto-update"),
    corner_radius: if composited { linux_corner_radius(style) } else { 0 },
  }
}

/// Elsewhere (the BSDs) the system title bar stays: nothing here knows their desktops
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn platform_title_bar(_environment: &TitleBarEnvironment) -> TitleBar {
  TitleBar::Native { reason: NativeReason::NoDesktop }
}

/// The line of the initialization script that tells the page how it draws the title bar (`title_bar`)
pub fn title_bar_script() -> String {
  let json = serde_json::to_string(title_bar()).unwrap_or_else(|_| r#"{"kind":"native"}"#.to_owned());
  format!("window.__BD_TITLE_BAR__ = {json};")
}

/// A window whose page draws the title bar (`title_bar`): the main window and every sub-window. It is built hidden:
/// `prepare_title_bar` sets the empty titlebar of Linux before GTK realizes the window, then the window shows once its
/// page settled its content (`sub_window_ready`; the main window's page on its first rows, `show_when_settled`). The
/// native title stays set (`title`, `set_title`): the Window menu, Mission Control and screen readers read it.
pub fn with_title_bar<R: Runtime, M: Manager<R>>(builder: WebviewWindowBuilder<'_, R, M>) -> WebviewWindowBuilder<'_, R, M> {
  let builder = builder.visible(false);
  // centred in the 40 px top row, 14 px from the left as from the top. wry takes y as the height of the button container
  // above the buttons, not as their top: the buttons keep their offset in the container, so y = 13 left their centre at
  // 11 px, and y = 22 puts it at 20 px
  #[cfg(target_os = "macos")]
  let builder = builder
    .title_bar_style(tauri::TitleBarStyle::Overlay)
    .hidden_title(true)
    .traffic_light_position(tauri::LogicalPosition::new(14.0, 22.0));
  #[cfg(target_os = "windows")]
  let builder = if is_custom_title_bar() { builder.decorations(false).shadow(true) } else { builder };
  // the page rounds the corners: an RGBA window shows the desktop beyond them (`corner_radius`, `round_corners`)
  #[cfg(target_os = "linux")]
  let builder = if corner_radius() > 0 { builder.transparent(true) } else { builder };
  builder
}

/// Prepares a window built by `with_title_bar` before it shows, on the main thread (a sub-window is built by an async
/// command): on Linux a custom title bar gets an empty GTK titlebar, so GTK draws client-side decorations without a
/// title (the shadow, the resize edges, the tiled and maximized states) and the page's row is the title, and the right
/// clicks of the webview are kept for the window menu. GTK takes a titlebar only before it realizes the window, so the
/// window-state plugin doesn't show it (`StateFlags::VISIBLE` is off). The steps of the main thread run in order: a show
/// queued after this one comes after the titlebar.
#[cfg(target_os = "linux")]
pub fn prepare_title_bar<R: Runtime>(window: &WebviewWindow<R>) {
  if !is_custom_title_bar() {
    return;
  }
  let target = window.clone();
  if let Err(e) = window.run_on_main_thread(move || {
    set_empty_titlebar(&target);
    remember_right_presses(&target);
    if corner_radius() > 0 {
      round_corners(&target);
    }
  }) {
    tracing::error!(error = ?e, window = window.label(), "cannot prepare the title bar on the main thread");
  }
}

/// Elsewhere a window shows as it is built
#[cfg(not(target_os = "linux"))]
pub const fn prepare_title_bar<R: Runtime>(_window: &WebviewWindow<R>) {}

#[cfg(target_os = "linux")]
thread_local! {
  /// The last right button press in the webview of each window (by label) and when it came, on the main thread: the
  /// window menu (`show_window_menu`) needs the real event, whose serial Wayland checks
  static RIGHT_PRESSES: std::cell::RefCell<std::collections::HashMap<String, (gtk::gdk::Event, std::time::Instant)>> = std::cell::RefCell::default();
}

/// How long after its press a right click opens the window menu: the page asks for it on the click's `contextmenu`
#[cfg(target_os = "linux")]
const RIGHT_PRESS_TTL: std::time::Duration = std::time::Duration::from_secs(2);

/// The right button press that the page answers with the window menu (`show_window_menu`), if it just came
#[cfg(target_os = "linux")]
pub fn last_right_press(label: &str) -> Option<gtk::gdk::Event> {
  RIGHT_PRESSES.with(|presses| presses.borrow().get(label).filter(|(_, at)| at.elapsed() < RIGHT_PRESS_TTL).map(|(event, _)| event.clone()))
}

/// The window went: its kept right press goes with it (a reopened window of the label starts clean)
#[cfg(target_os = "linux")]
pub fn forget_window(label: &str) {
  RIGHT_PRESSES.with(|presses| presses.borrow_mut().remove(label));
}

/// Elsewhere nothing is kept per window
#[cfg(not(target_os = "linux"))]
pub const fn forget_window(_label: &str) {}

/// Keeps the right button presses of the webview: its handler runs before the one of `WebKit`, which takes the event
#[cfg(target_os = "linux")]
fn remember_right_presses<R: Runtime>(window: &WebviewWindow<R>) {
  use gtk::prelude::WidgetExt;
  let label = window.label().to_owned();
  let result = window.with_webview(move |webview| {
    webview.inner().connect_button_press_event(move |_, event| {
      if event.button() == 3 {
        let event: &gtk::gdk::Event = event;
        RIGHT_PRESSES.with(|presses| presses.borrow_mut().insert(label.clone(), (event.clone(), std::time::Instant::now())));
      }
      gtk::glib::Propagation::Proceed
    });
  });
  if let Err(e) = result {
    tracing::warn!(error = ?e, window = window.label(), "cannot watch the right clicks of the window");
  }
}

/// The style class of a window whose page rounds its corners (`round_corners`)
#[cfg(target_os = "linux")]
const ROUNDED_CLASS: &str = "bd-rounded";

/// Rounds a transparent window as the desktop's windows are, with GTK's own client-side decorations: GTK keeps drawing
/// the shadow, the resize edges in it and the frame extents, now around the radius of the page (its `decoration` node),
/// none while the window is maximized, tiled or fullscreen; the window paints no background of its own, so the desktop
/// shows beyond the corners the page clips. The page learns when the window is tiled (GTK knows it, Tauri doesn't).
#[cfg(target_os = "linux")]
fn round_corners<R: Runtime>(window: &WebviewWindow<R>) {
  use gtk::prelude::{CssProviderExt, GtkWindowExt, StyleContextExt, WidgetExt};
  use tauri::Emitter;
  thread_local! {
    static PROVIDER_ADDED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
  }
  let Ok(gtk_window) = window.gtk_window() else {
    return;
  };
  // tao paints a transparent window itself and keeps GTK from drawing it, the decorations too: GTK draws them again, and
  // the window background is transparent by its CSS
  gtk_window.set_app_paintable(false);
  if !PROVIDER_ADDED.get() {
    let radius = corner_radius();
    let square = ["maximized", "fullscreen", "tiled", "tiled-top", "tiled-right", "tiled-bottom", "tiled-left"]
      .map(|state| format!("window.{ROUNDED_CLASS}.{state} decoration"))
      .join(", ");
    let css = format!(
      "window.{ROUNDED_CLASS}, window.{ROUNDED_CLASS} > box {{ background-color: transparent; }} \
       window.{ROUNDED_CLASS} decoration {{ border-radius: {radius}px; }} {square} {{ border-radius: 0; }}"
    );
    let provider = gtk::CssProvider::new();
    match provider.load_from_data(css.as_bytes()) {
      Ok(()) => {
        if let Some(screen) = GtkWindowExt::screen(&gtk_window) {
          gtk::StyleContext::add_provider_for_screen(&screen, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
          PROVIDER_ADDED.set(true);
        }
      }
      Err(e) => tracing::error!(error = %e, "cannot load the CSS of the rounded window"),
    }
  }
  gtk_window.style_context().add_class(ROUNDED_CLASS);
  let target = window.clone();
  gtk_window.connect_window_state_event(move |_, event| {
    use gtk::gdk::WindowState;
    let tiled = event
      .new_window_state()
      .intersects(WindowState::TILED | WindowState::TOP_TILED | WindowState::RIGHT_TILED | WindowState::BOTTOM_TILED | WindowState::LEFT_TILED);
    if let Err(e) = target.emit_to(target.label(), "window-tiled", tiled) {
      tracing::warn!(error = ?e, window = target.label(), "cannot tell the page that the window is tiled");
    }
    gtk::glib::Propagation::Proceed
  });
}

#[cfg(target_os = "linux")]
fn set_empty_titlebar<R: Runtime>(window: &WebviewWindow<R>) {
  use gtk::prelude::{GtkWindowExt, WidgetExt};
  match window.gtk_window() {
    Ok(gtk_window) => {
      if gtk_window.is_realized() {
        debug_assert!(false, "the window is realized before its titlebar is set");
        tracing::error!(window = window.label(), "the window is realized: GTK keeps its title bar");
        return;
      }
      gtk_window.set_titlebar(Some(&gtk::EventBox::new()));
    }
    Err(e) => tracing::error!(error = ?e, window = window.label(), "cannot read the GTK window"),
  }
}
