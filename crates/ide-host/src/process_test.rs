use crate::process::serve;
use app_settings::service::SettingsService;
use domain_types::settings::SettingsPatch;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::Path;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream, Lines};
use tokio::task::JoinHandle;

/// The bridge on an app data directory, as the IDE starts it, without an app to join, so it serves the IDE with a
/// backend of its own: its input, the lines of its output and its task
pub(crate) struct Brd {
  input: DuplexStream,
  lines: Lines<BufReader<DuplexStream>>,
  task: JoinHandle<i32>,
}

impl Brd {
  pub(crate) fn start(app_data: &Path) -> Self {
    let (input, from_test) = tokio::io::duplex(1 << 16);
    let (to_test, output) = tokio::io::duplex(1 << 20);
    let task = tokio::spawn(serve(from_test, to_test, Some(app_data.to_path_buf()), None));
    Self {
      input,
      lines: BufReader::new(output).lines(),
      task,
    }
  }

  /// Writes one line to the input and answers the next line of the output as JSON
  pub(crate) async fn ask(&mut self, line: &str) -> Value {
    self.input.write_all(format!("{line}\n").as_bytes()).await.unwrap();
    serde_json::from_str(&self.lines.next_line().await.unwrap().unwrap()).unwrap()
  }

  /// The end of the input ends the bridge with 0
  pub(crate) async fn end(self) {
    drop(self.input);
    assert_eq!(tokio::time::timeout(std::time::Duration::from_secs(30), self.task).await.unwrap().unwrap(), 0);
  }
}

/// `bd_bootstrap` of the Settings window with `arguments`
fn settings_bootstrap(id: u32, arguments: &Value) -> String {
  json!({ "jsonrpc": "2.0", "id": id, "method": "tools/call", "params": { "name": "bd_bootstrap", "arguments": arguments, "_meta": { "x-air-app": { "origin": "click", "instance": "settings:2" } } } }).to_string()
}

/// The settings of the app data directory that the desktop app saved, as `bd_bootstrap` gives them with the title bar
/// of the IDE, `native` unless the host context names an overlay one; the end of the input ends the bridge with 0
#[tokio::test]
async fn the_bridge_serves_the_settings_of_the_app_until_its_input_ends() {
  let app_data = tempfile::tempdir().unwrap();
  {
    let desktop = SettingsService::open(app_data.path().to_path_buf()).unwrap();
    let patch = SettingsPatch::Value {
      key: "theme".to_owned(),
      value: json!("dark"),
    };
    desktop.set(patch, "main").unwrap();
    desktop.flush().unwrap();
  }
  let mut brd = Brd::start(app_data.path());
  let answer = brd.ask(&json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }).to_string()).await;
  assert_eq!(answer["result"]["capabilities"]["experimental"]["x-air-app"]["version"], "1.0.0");

  let answer = brd.ask(&settings_bootstrap(2, &json!({ "view": "settings" }))).await;
  assert_eq!(answer["result"]["structuredContent"]["settings"]["values"]["theme"], "dark", "{answer:#}");
  // the title bar of the IDE: no updates, no folder dialog, a text of the progress
  assert_eq!(
    answer["result"]["structuredContent"]["titleBar"],
    json!({ "kind": "native", "checkForUpdates": false, "folderDialog": false, "progressText": true })
  );

  // a window of the IDE that merges the row of the page into its title bar: the row starts after the traffic lights at
  // the height of the IDE, and the IDE does the double click
  let overlay = json!({ "kind": "overlay", "height": 40, "insetStart": 80 });
  let answer = brd.ask(&settings_bootstrap(3, &json!({ "view": "settings", "titleBar": overlay }))).await;
  assert_eq!(
    answer["result"]["structuredContent"]["titleBar"],
    json!({ "kind": "overlay", "doubleClick": "host", "height": 40, "checkForUpdates": false, "folderDialog": false, "progressText": true })
  );

  // a height of the IDE with a fraction is the row in whole px, and one outside 1 to 200 leaves the row at its own
  for (id, height, row) in [(4, json!(28.4), json!(28)), (5, json!(0), Value::Null), (6, json!(201), Value::Null)] {
    let overlay = json!({ "kind": "overlay", "height": height, "insetStart": 78 });
    let answer = brd.ask(&settings_bootstrap(id, &json!({ "view": "settings", "titleBar": overlay }))).await;
    assert_eq!(answer["result"]["structuredContent"]["titleBar"]["height"], row, "{answer:#}");
  }
  brd.end().await;
}
