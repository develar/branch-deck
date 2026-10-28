use super::*;
use host_link::refusal::Code;
use pretty_assertions::assert_eq;
use serde_json::Value;

/// The text of the spec of bd-host, whose fenced blocks hold the golden JSON
const SPEC: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../spec/cli/host.spec.md"));

/// The JSON of the fenced block ```` ```json golden=<name> ```` of the spec
fn golden(name: &str) -> Value {
  let fence = format!("```json golden={name}\n");
  let start = SPEC.find(&fence).unwrap_or_else(|| panic!("no block {fence:?} in the spec")) + fence.len();
  let end = start + SPEC[start..].find("\n```").expect("the end of the block");
  serde_json::from_str(&SPEC[start..end]).expect("the block is JSON")
}

fn step(name: &str, ms: u64) -> Phase {
  Phase { name: name.to_owned(), ms }
}

/// The run of the golden JSON: a run of the committed HEAD that exited 0 and brought back one file
#[test]
fn the_report_of_a_run_is_the_golden_json_of_the_spec() {
  let commit = "8d4b3790b5c0e7f1a2d3c4b5a6978877665544aa";
  let phases = [
    ("pack", 3100),
    ("upload", 2000),
    ("slot", 120),
    ("pack_wait", 0),
    ("checkout", 4200),
    ("prepare", 0),
    ("command", 600_000),
    ("stage", 800),
    ("fetch", 900),
    ("total", 612_000),
  ];
  let outcome = RunOutcome {
    code: 0,
    stdout: String::new(),
    facts: RunFacts {
      run: Some("1793181600000-4242".to_owned()),
      commit: Some(commit.to_owned()),
      head: Some(commit.to_owned()),
      working_tree: false,
      repository: Some("~/projects/idea-1".to_owned()),
      phases: phases.iter().map(|(name, ms)| step(name, *ms)).collect(),
      fetched: vec!["out/bt-runs/20261028-100000-0001/summary.json".to_owned()],
      applied_files: None,
    },
  };
  let report = RunReport::of_outcome("studio", &outcome);
  let expected = golden("run-report");
  assert_eq!(serde_json::to_value(&report).unwrap(), expected);
  assert_eq!(serde_json::from_value::<RunReport>(expected).unwrap(), report, "the golden JSON reads back");
  assert!(report.line().ends_with("}\n"));
  assert_eq!(report.line().lines().count(), 1);
}

/// A refusal carries its code and its message, the exit of bd-host, and `null` for what the client did not learn
#[test]
fn a_refusal_reports_its_code_its_exit_and_what_the_client_knew() {
  let refusal = Refusal::new(Code::Behind, "studio has none of the commits near HEAD");
  let failure = RunFailure {
    exit: refusal.code.exit(host_link::refusal::Reached::Nothing),
    refusal,
    command_exit: None,
    facts: RunFacts {
      commit: Some("c".to_owned()),
      head: Some("c".to_owned()),
      working_tree: true,
      ..RunFacts::default()
    },
  };
  let report = serde_json::to_value(RunReport::of_failure("studio", &failure)).unwrap();
  assert_eq!(report["exit"], 75);
  assert_eq!(
    report["refusal"],
    serde_json::json!({"code": "behind", "message": "studio has none of the commits near HEAD"})
  );
  assert_eq!(
    (report["run"].clone(), report["command_exit"].clone(), report["working_tree"].clone()),
    (Value::Null, Value::Null, Value::Bool(true))
  );

  let usage = RunReport::of_refusal("studio", false, &Refusal::new(Code::Usage, "the descriptor 3 is not open"), Exit::Usage);
  assert_eq!((usage.exit, usage.commit, usage.phases.len()), (2, None, 0));
  assert_eq!((command_exit_code(130), command_exit_code(-1), command_exit_code(300)), (130, 255, 255));
}

/// The hello of the golden JSON: both ends of one build, so `same_build` is true; a refusal has its own shape
#[test]
fn the_hello_is_the_golden_json_of_the_spec() {
  let build = "5e1f0c3a9b7d4e2f8a6c1b3d5e7f9a0b2c4d6e8f1a3b5c7d9e0f2a4b6c8d0e1f";
  let agent = AgentInfo {
    version: "0.37.0".to_owned(),
    schema: 1,
    os: "macos".to_owned(),
    arch: "aarch64".to_owned(),
    features: vec!["upgrade".to_owned()],
    build: Some(build.to_owned()),
    address: "192.168.1.20:7742".parse().unwrap(),
  };
  let client = Build {
    version: "0.37.0".to_owned(),
    sha256: Some(build.to_owned()),
  };
  let hello = HelloReport::new("studio", &agent, &client);
  let expected = golden("hello");
  assert_eq!(serde_json::to_value(&hello).unwrap(), expected);
  assert_eq!(serde_json::from_value::<HelloReport>(expected).unwrap(), hello);

  let other = HelloReport::new("studio", &agent, &Build { sha256: None, ..client });
  assert!(!other.same_build, "an unknown build is no same build");

  let refused = serde_json::to_value(HelloRefusal::new("studio", &Refusal::new(Code::Unreachable, "no agent answers"))).unwrap();
  assert_eq!(
    refused,
    serde_json::json!({"schema": 1, "alias": "studio", "refusal": {"code": "unreachable", "message": "no agent answers"}})
  );
}
