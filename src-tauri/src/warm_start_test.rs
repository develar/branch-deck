use crate::warm_start::script;
use domain_types::session::{CachedModel, RepositoryView, TableHeader, Version};

/// The text of the string literal of `script`, with its escapes read back as JavaScript reads them
fn read_back(script: &str) -> String {
  let literal = script.strip_prefix("window.__BD_MODEL__ = JSON.parse('").and_then(|rest| rest.strip_suffix("');")).unwrap();
  assert!(!literal.contains('\n') && !literal.contains('\r') && !literal.contains('\u{2028}'), "{literal}");
  let mut text = String::new();
  let mut chars = literal.chars();
  while let Some(char) = chars.next() {
    if char != '\\' {
      assert_ne!(char, '\'', "an unescaped quote ends the literal: {literal}");
      text.push(char);
      continue;
    }
    match chars.next().unwrap() {
      'n' => text.push('\n'),
      'r' => text.push('\r'),
      'u' => {
        let code: String = chars.by_ref().take(4).collect();
        text.push(char::from_u32(u32::from_str_radix(&code, 16).unwrap()).unwrap());
      }
      other => text.push(other),
    }
  }
  text
}

#[test]
fn the_model_is_the_json_of_the_path_and_the_view_in_a_string_that_json_parse_reads() {
  let view = RepositoryView {
    version: Version { epoch: 7, seq: 0 },
    cached: true,
    header: TableHeader {
      prefix: "jane's \\ prefix\u{2028}".to_owned(),
      ..TableHeader::default()
    },
    ..RepositoryView::default()
  };
  let text = script(&CachedModel {
    repository_path: "/work/it's here".to_owned(),
    view: view.clone(),
  });
  let json: serde_json::Value = serde_json::from_str(&read_back(&text)).unwrap();
  assert_eq!(json["repositoryPath"], "/work/it's here");
  assert_eq!(json["view"], serde_json::to_value(&view).unwrap());
}
