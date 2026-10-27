use crate::check_record::{CheckRecord, parse_check_records};
use crate::lease_clean::{parse_clean_rules, parse_pick_requires_check};
use pretty_assertions::assert_eq;

/// Only a green line of schema 1 counts: another schema, a red run, a line that does not parse or has no tree is left
/// out, and an unknown field is ignored
#[test]
fn only_green_records_of_schema_one_count() {
  let text = [
    r#"{"schema":1,"tree":"t1","head":"h1","tool":"bt","command":"build","patterns":["//plugins/air/..."],"exit":0,"at":5}"#,
    r#"{"schema":2,"tree":"t2","exit":0}"#,
    r#"{"schema":1,"tree":"t3","exit":3}"#,
    r#"{"schema":1,"tree":"t4","exit":0"#,
    r#"{"schema":1,"exit":0}"#,
    "",
    r#"{"schema":1,"tree":"t5","head":"h5","tool":"bt","command":"test","patterns":["--lane","fast"],"host":"studio","exit":0,"at":9,"new":true}"#,
  ]
  .join("\n");
  let records = parse_check_records(&text);
  assert_eq!(records.iter().map(|record| record.tree.as_str()).collect::<Vec<_>>(), ["t1", "t5"]);
  assert_eq!(records[0].describe(), "bt build //plugins/air/...");
  assert_eq!(records[1].describe(), "bt test --lane fast --host studio");
  assert_eq!(
    records[1],
    CheckRecord {
      tree: "t5".to_string(),
      head: "h5".to_string(),
      tool: "bt".to_string(),
      command: "test".to_string(),
      patterns: vec!["--lane".to_string(), "fast".to_string()],
      host: Some("studio".to_string()),
      at: 9,
    }
  );
}

/// `pick_requires_check` of `[lease]` is false without the key, and a value that is no boolean is an error of the file
#[test]
fn pick_requires_check_is_a_boolean_of_the_lease_table() {
  assert!(!parse_pick_requires_check("").unwrap());
  assert!(!parse_pick_requires_check("[lease]\nclean = []\n").unwrap());
  let text = "[lease]\npick_requires_check = true\nclean = [{ delete = \"x\" }]\n";
  assert!(parse_pick_requires_check(text).unwrap());
  assert_eq!(parse_clean_rules(text).unwrap().len(), 1, "the clean rules read the file with the key");
  let error = format!("{:#}", parse_pick_requires_check("[lease]\npick_requires_check = \"yes\"\n").unwrap_err());
  assert!(error.contains("not a boolean"), "{error}");
  let error = format!("{:#}", parse_pick_requires_check("[lease]\npick_requires_checks = true\n").unwrap_err());
  assert!(error.contains("unknown key lease.pick_requires_checks"), "{error}");
}
