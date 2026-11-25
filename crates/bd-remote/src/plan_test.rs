//! `brd plan`: the landing plan of the repository as the app's session derives it, read only

use crate::landed_test::util_flow_ui;
use bd_cli::fixture_test::{PREFIX, assert_covered, bd_in, git, run_json};
use bd_cli::remote_fixture_test::Space;
use pretty_assertions::assert_eq;
use serde_json::{Value, json};

/// `brd --prefix test-user plan` of `space`, every field of it in `brd schema plan`
fn covered_plan(space: &Space) -> Value {
  let (exit_code, plan) = run_json(&space.environment(), &["--prefix", PREFIX, "plan"]);
  assert_eq!(exit_code, 0, "{plan}");
  let (_, schema) = run_json(&space.environment(), &["schema", "plan"]);
  assert_covered(&schema["outputs"]["plan"], &schema["outputs"]["plan"], &plan, "plan");
  plan
}

/// On Space `flow` uses a name `util` adds and `ui` is built on `flow`: two units, one edge of the build that nothing
/// lands, `util` next and `flow` after it, and Land Together of `flow` onto `util` with its price and its command; nothing is written
#[test_log::test]
fn plan_prints_the_units_the_edge_the_waves_and_land_together_with_its_price_and_command() {
  let space = Space::new(&[]);
  let brd = bd_in(&space.repo);
  util_flow_ui(&space);
  let head = git(&space.repo, &["rev-parse", "HEAD"]);

  let plan = covered_plan(&space);
  assert_eq!(plan["stale"], json!(false));
  assert_eq!(plan["checks_unknown"], json!("the baseline has no .patronus/config.yaml"));
  let units: Vec<(&Value, &Value)> = plan["units"].as_array().unwrap().iter().map(|unit| (&unit["name"], &unit["branches"])).collect();
  // in the order of the table, the newest first
  assert_eq!(units, [(&json!("flow"), &json!(["flow", "ui"])), (&json!("util"), &json!(["util"]))]);
  assert_eq!(
    plan["edges"],
    json!([{
      "from": "flow",
      "needed": "util",
      "kind": "build",
      "state": "open",
      "names": ["controlUrl"],
      "files": ["src/b/Use.kt"],
      "refusals": 2,
      "refuses": ["ui", "flow"],
      "actions": [
        { "kind": "land_together", "command": format!("{brd} restack flow util") },
        { "kind": "land_first", "command": format!("{brd} safe-push util") },
      ],
    }])
  );
  // flow lands after util, or with it
  assert_eq!(
    plan["waves"],
    json!([{ "kind": "next", "units": ["util"] }, { "kind": "after", "after": ["util"], "units": ["flow"] }])
  );
  assert_eq!(plan["parked"], json!([]));
  assert_eq!(
    plan["proposals"],
    json!([{
      "from": "flow",
      "onto": "util",
      "onto_unit": "util",
      "forks": false,
      "command": format!("{brd} restack flow util"),
      "price": {
        "robots": 2,
        "together": 1,
        "checks": { "kind": "unknown", "reason": "the baseline has no .patronus/config.yaml" },
        "failure_holds_both": true,
        "owners_added": [],
      },
    }])
  );
  // read only
  assert_eq!(git(&space.repo, &["rev-parse", "HEAD"]), head);
}
