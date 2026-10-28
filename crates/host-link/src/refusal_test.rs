use super::{Code, Exit, Reached, Refusal};
use pretty_assertions::assert_eq;
use std::collections::BTreeSet;

/// The rows of the table "Refusal codes" of spec/cli/host.spec.md: the code, the exit and the words
fn spec_rows() -> Vec<(String, String, String)> {
  let spec = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../spec/cli/host.spec.md")).expect("the spec of bd-host");
  let section = &spec[spec.find("### Refusal codes").expect("the section of the codes")..];
  section
    .lines()
    .filter(|line| line.starts_with("| `"))
    .map(|line| {
      let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
      (cells[0].trim_matches('`').to_owned(), cells[1].to_owned(), cells[2].to_owned())
    })
    .collect()
}

#[test]
fn every_refusal_code_has_one_name_and_one_exit() {
  let table: Vec<(String, String, String)> = Code::ALL
    .iter()
    .map(|code| {
      let row = code.row();
      (row.name.to_owned(), row.exit.text().to_owned(), row.words.to_owned())
    })
    .collect();
  assert_eq!(table, spec_rows(), "the codes are the table of the spec, in its order");
  let names: BTreeSet<&str> = Code::ALL.iter().map(|code| code.name()).collect();
  assert_eq!(names.len(), Code::ALL.len(), "each name names one code");
  for code in Code::ALL {
    assert_eq!(Code::from_name(code.name()), Some(code));
    assert_eq!(serde_json::to_string(&code).expect("json"), format!("\"{code}\""));
  }
  assert_eq!(Code::from_name("lock_timeout"), None, "yonder's lock_timeout is slot_timeout");
}

#[test]
fn the_exit_of_a_code_follows_how_far_the_run_came() {
  let exits = |code: Code| [Reached::Nothing, Reached::Started, Reached::CommandStarted].map(|reached| code.exit(reached).code());
  assert_eq!(exits(Code::Usage), [2, 2, 2]);
  assert_eq!(exits(Code::Interrupted), [130, 130, 130]);
  assert_eq!(exits(Code::SlotTimeout), [75, 75, 75], "a refusal of the agent before the command");
  assert_eq!(exits(Code::TimedOut), [75, 255, 255]);
  assert_eq!(exits(Code::AgentError), [75, 75, 255]);
  assert_eq!(exits(Code::UnknownRun), [255, 255, 255]);
  assert_eq!(Exit::NotRun.code(), 75);
}

#[test]
fn a_refusal_prints_its_code_and_an_unknown_code_of_an_agent_is_an_agent_error() {
  let refusal = Refusal::new(Code::Behind, "the host has none of the commits");
  assert_eq!(refusal.to_string(), "behind: the host has none of the commits");
  let json = serde_json::to_string(&refusal).expect("json");
  assert_eq!(json, r#"{"code":"behind","message":"the host has none of the commits"}"#);
  assert_eq!(serde_json::from_str::<Refusal>(&json).expect("parsed"), refusal);
  serde_json::from_str::<Refusal>(r#"{"code":"lock_timeout","message":""}"#).expect_err("no code of bd-host");

  assert_eq!(Refusal::of_agent("pack_failed", "no pack"), Refusal::new(Code::PackFailed, "no pack"));
  assert_eq!(
    Refusal::of_agent("new_code", "words"),
    Refusal::new(Code::AgentError, "the agent refused with the code new_code, which this build does not know: words")
  );
}
