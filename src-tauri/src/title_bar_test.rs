use serde_json::Value;

fn capability(name: &str) -> Value {
  let path = format!("{}/capabilities/{name}.json", env!("CARGO_MANIFEST_DIR"));
  serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
}

fn strings(value: &Value, key: &str) -> Vec<String> {
  value[key]
    .as_array()
    .unwrap()
    .iter()
    .filter_map(|it| it.as_str().map(str::to_owned))
    .collect()
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
