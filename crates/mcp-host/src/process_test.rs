use crate::process::serve;
use app_settings::service::SettingsService;
use domain_types::settings::SettingsPatch;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// The settings of the app data directory that the desktop app saved, as `bd_bootstrap` gives them; the end of the
/// input ends `brd mcp` with 0
#[tokio::test]
async fn brd_mcp_serves_the_settings_of_the_app_until_its_input_ends() {
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
  let (to_brd, input) = tokio::io::duplex(1 << 16);
  let (output, from_brd) = tokio::io::duplex(1 << 20);
  let brd = tokio::spawn(serve(input, output, Some(app_data.path().to_path_buf())));
  let (mut to_brd, mut lines) = (to_brd, BufReader::new(from_brd).lines());
  let initialize = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} });
  to_brd.write_all(format!("{initialize}\n").as_bytes()).await.unwrap();
  let answer: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
  assert_eq!(answer["result"]["capabilities"]["experimental"]["x-air-app"]["version"], "1.0.0");

  let bootstrap = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "bd_bootstrap", "arguments": { "view": "settings" }, "_meta": { "x-air-app": { "origin": "click", "instance": "settings:2" } } } });
  to_brd.write_all(format!("{bootstrap}\n").as_bytes()).await.unwrap();
  let answer: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
  assert_eq!(answer["result"]["structuredContent"]["settings"]["values"]["theme"], "dark", "{answer:#}");

  drop(to_brd);
  assert_eq!(tokio::time::timeout(std::time::Duration::from_secs(30), brd).await.unwrap().unwrap(), 0);
}
