use crate::LOG_PLUGIN_VERSION;

/// The comment of `LOG_FILE_BYTES` describes the rotation of one version of `tauri_plugin_log` (`RotatingFile::new`,
/// `rotate` and `flush`: a rotation on a write past the size, within a session, `KeepSome(n)` as `n` dated files beside
/// the current one). An earlier text described 2.7.1, which rotated only at the start, while the lockfile linked 2.10.1.
/// So the version the workspace builds must be the one the comment names: after an update, read the rotation of the new
/// one, correct the comment and move `LOG_PLUGIN_VERSION`
#[test]
fn the_log_retention_comment_describes_the_plugin_version_the_lockfile_pins() {
  let lockfile = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../Cargo.lock")).unwrap();
  let versions: Vec<&str> = lockfile
    .split("\n[[package]]\n")
    .filter(|package| package.contains("name = \"tauri-plugin-log\"\n"))
    .filter_map(|package| package.lines().find_map(|line| line.strip_prefix("version = \"")?.strip_suffix('"')))
    .collect();
  assert_eq!(
    versions,
    [LOG_PLUGIN_VERSION],
    "Cargo.lock pins another tauri-plugin-log: read the rotation of that version and correct the comment of LOG_FILE_BYTES"
  );
}
