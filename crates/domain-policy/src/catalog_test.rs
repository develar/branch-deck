//! A catalog the page's formatters are tested on (`reasons.json`, `actions.json`): written by a test with its variable
//! set, compared by value otherwise, so a stale catalog fails

use serde::Serialize;
use std::path::Path;

/// Writes `value` to the catalog `file` of the crate while `variable` is set, else asserts the committed one is it;
/// `filter` the test filter that writes it, for the message
pub(crate) fn assert_catalog(file: &str, variable: &str, filter: &str, value: &impl Serialize) {
  let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(file);
  if std::env::var_os(variable).is_some() {
    let mut written = serde_json::to_string_pretty(value).expect("the catalog serializes");
    written.push('\n');
    std::fs::write(&path, &written).expect("the catalog is written");
    return;
  }
  // by value: the formatter of the repository lays the file out its own way
  let committed: Option<serde_json::Value> = std::fs::read_to_string(&path).ok().and_then(|text| serde_json::from_str(&text).ok());
  assert!(
    committed == serde_json::to_value(value).ok(),
    "{file} is stale: {variable}=1 cargo nextest run -p domain-policy -E 'test(/{filter}/)' writes it"
  );
}
