use crate::identity::APP_IDENTIFIER;
use crate::identity_source::identifier_of;

const BASE: &str = r#"{"productName":"BranchDeck","identifier":"org.develar.branch-deck"}"#;

#[test]
fn the_identifier_is_the_one_of_the_configuration_under_the_overlay_of_tauri_build() {
  // (the overlay of `TAURI_CONFIG`, the identifier)
  let cases = [
    (None, Some("org.develar.branch-deck")),
    (
      Some(r#"{"identifier":"org.develar.branch-deck.check","productName":"BranchDeckCheck"}"#),
      Some("org.develar.branch-deck.check"),
    ),
    // an overlay of other keys keeps the identifier of the configuration
    (Some(r#"{"build":{"features":[]}}"#), Some("org.develar.branch-deck")),
    (Some("not json"), Some("org.develar.branch-deck")),
  ];
  for (overlay, expected) in cases {
    assert_eq!(identifier_of(BASE, overlay).as_deref(), expected, "{overlay:?}");
  }
  assert_eq!(identifier_of("{}", None), None);
}

#[test]
fn the_compiled_identifier_is_the_one_of_tauri_conf_json() {
  let config = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../src-tauri/tauri.conf.json")).unwrap();
  assert_eq!(identifier_of(&config, option_env!("TAURI_CONFIG")).as_deref(), Some(APP_IDENTIFIER));
}
