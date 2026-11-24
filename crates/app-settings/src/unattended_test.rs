use crate::service::SettingsService;
use crate::unattended::{UNATTENDED_FIELD, append_denial, denials_file, read_denials, set_unattended};
use domain_types::settings::APP_SETTINGS_KEY;
use domain_types::unattended::UnattendedDenial;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;

/// Longer than any test: a save happens only by a flush or a drop
const NEVER: Duration = Duration::from_hours(1);

/// The settings file of the app data directory `directory` as it is on the disk
fn saved(directory: &Path) -> Value {
  serde_json::from_slice(&std::fs::read(directory.join("settings.json")).unwrap()).unwrap()
}

fn denial(time: u32, tool_name: &str) -> UnattendedDenial {
  UnattendedDenial {
    time,
    session_id: "session".to_owned(),
    agent_id: None,
    agent_type: None,
    cwd: "/work".to_owned(),
    tool_name: tool_name.to_owned(),
    summary: "cat /etc/hosts".to_owned(),
  }
}

#[test]
fn turning_the_mode_on_starts_a_new_list_and_turning_it_off_keeps_it() {
  let directory = tempfile::tempdir().unwrap();
  let journal = denials_file(directory.path());
  let service = SettingsService::open_with_debounce(directory.path().to_path_buf(), NEVER).unwrap();
  append_denial(&journal, &denial(1, "Bash")).unwrap();

  let on = set_unattended(&service, &journal, true, "rust").unwrap();
  assert!(on.changed);
  // saved once the switch answers, with no save of the writer due before an hour: the hook reads the file
  assert_eq!(saved(directory.path())[APP_SETTINGS_KEY][UNATTENDED_FIELD], json!(true));
  let change = on.change.expect("the change the windows get");
  assert_eq!(change.value["unattended"], json!(true));
  assert!(service.app().unattended);
  assert_eq!(read_denials(&journal), Vec::new(), "the list of the night before is gone");

  append_denial(&journal, &denial(2, "Read")).unwrap();
  let again = set_unattended(&service, &journal, true, "rust").unwrap();
  assert!(!again.changed && again.change.is_none(), "on again changes nothing");
  assert_eq!(read_denials(&journal), vec![denial(2, "Read")], "and keeps the list");

  let off = set_unattended(&service, &journal, false, "rust").unwrap();
  assert!(off.changed);
  assert!(!service.app().unattended);
  assert_eq!(read_denials(&journal), vec![denial(2, "Read")], "the morning reads the list of the night");

  assert_eq!(saved(directory.path()), json!({}), "saved at once; off is the default: the file keeps no value");
}

#[test]
fn the_journal_keeps_the_lines_that_parse() {
  let directory = tempfile::tempdir().unwrap();
  let journal = denials_file(directory.path());
  assert_eq!(read_denials(&journal), Vec::new(), "no file, no denial");
  append_denial(&journal, &denial(1, "Bash")).unwrap();
  // a line cut by a full disk, and a line of another shape
  let mut text = std::fs::read_to_string(&journal).unwrap();
  text.push_str("{\"time\":2,\"tool_na\n[1,2]\n");
  std::fs::write(&journal, text).unwrap();
  append_denial(&journal, &denial(3, "WebFetch")).unwrap();
  assert_eq!(read_denials(&journal), vec![denial(1, "Bash"), denial(3, "WebFetch")]);
}

#[test]
fn hooks_that_deny_at_once_append_whole_lines() {
  let directory = tempfile::tempdir().unwrap();
  let journal = denials_file(directory.path());
  std::thread::scope(|scope| {
    for thread in 0..8 {
      let journal = &journal;
      scope.spawn(move || {
        for index in 0..50 {
          let mut denial = denial(thread * 100 + index, "Bash");
          denial.summary = "x".repeat(150);
          append_denial(journal, &denial).unwrap();
        }
      });
    }
  });
  let text = std::fs::read_to_string(&journal).unwrap();
  assert_eq!(text.lines().count(), 400);
  assert_eq!(read_denials(&journal).len(), 400, "every line parses: none mixed with another");
}
