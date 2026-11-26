use super::RunState;
use host_link::proto::{Phase, RunInfo};
use pretty_assertions::assert_eq;
use serde_json::json;

fn info(phases: &[&str], exit: Option<i32>) -> RunInfo {
  RunInfo {
    run: "1-1".to_owned(),
    affinity: "checkout-0".to_owned(),
    commit: "c".to_owned(),
    command: vec!["true".to_owned()],
    started_ms: 0,
    ended_ms: None,
    exit,
    phases: phases.iter().map(|name| Phase { name: (*name).to_owned(), ms: 1 }).collect(),
    fetch: Vec::new(),
    forgotten: false,
    refused: None,
    cores: None,
    repo: None,
    slot: None,
  }
}

/// The state of a run is the step after the last of `slot`, `prepare` and `command` that ended, or its place in the heavy
/// turn before its slot, and a run that is not heavy waits for a measurement that holds the host
#[test]
fn the_state_of_a_run_comes_from_its_ended_steps_and_the_heavy_queue() {
  let turn = json!({
    "holders": [{"run": "0-0", "pid": 1, "exclusive": false}],
    "queue": [{"run": "0-1", "pid": 2}, {"run": "1-1", "pid": 3}],
  });
  assert_eq!(RunState::of(&info(&[], None), Some(&turn)), RunState::HeavyTurn { ahead: 1 });
  assert_eq!(RunState::of(&info(&[], None), None), RunState::Slot, "an agent without the heavy turn");
  assert_eq!(
    RunState::of(&info(&["heavy"], None), Some(&json!({"holders": [{"run": "1-1"}]}))),
    RunState::Slot,
    "it holds the turn"
  );
  let measured = json!({"holders": [{"run": "0-0", "exclusive": true}], "queue": []});
  assert_eq!(RunState::of(&info(&[], None), Some(&measured)), RunState::Measurement);
  assert_eq!(RunState::of(&info(&["git", "slot", "pack_wait"], None), None), RunState::Prepares);
  assert_eq!(RunState::of(&info(&["slot", "pack_wait", "checkout", "prepare"], None), None), RunState::Command);
  assert_eq!(RunState::of(&info(&["slot", "prepare", "command", "measure"], None), None), RunState::Stages);
  assert_eq!(RunState::of(&info(&["slot", "prepare", "command"], Some(0)), None), RunState::Ended);

  assert_eq!(
    RunState::HeavyTurn { ahead: 2 }.line("1-1", "studio", "14:02:11"),
    "brd-host: 14:02:11 run 1-1 waits for the heavy turn of studio, 2 runs ahead: `brd-host runs studio` names the runs that hold it"
  );
  assert_eq!(RunState::Command.line("1-1", "studio", "14:10:02"), "brd-host: 14:10:02 run 1-1 runs its command on studio");
}
