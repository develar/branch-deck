//! `BD_VERSION`: the version of the app, which release-please keeps in the root `package.json` (`tauri.conf.json`
//! reads it from there too). The workspace version of the crates stays 0.1.0, so `CARGO_PKG_VERSION` is not the
//! product version. bd-cli runs this script too (`build` in its manifest), for the version in its help texts

const PACKAGE_JSON: &str = "../../package.json";

fn main() {
  println!("cargo::rerun-if-changed={PACKAGE_JSON}");
  let text = std::fs::read_to_string(PACKAGE_JSON).unwrap_or_else(|e| panic!("cannot read {PACKAGE_JSON}: {e}"));
  let package: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("cannot parse {PACKAGE_JSON}: {e}"));
  let version = package["version"].as_str().unwrap_or_else(|| panic!("no version in {PACKAGE_JSON}"));
  println!("cargo::rustc-env=BD_VERSION={version}");
}
