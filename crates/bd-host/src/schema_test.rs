use pretty_assertions::assert_eq;
use serde_json::Value;
use std::collections::BTreeSet;

use super::*;

const SPEC: &str = include_str!("../../../spec/cli/host.spec.md");

/// The golden JSON of the fenced block `json golden=<name>` of the spec
fn golden(name: &str) -> Value {
  let start = format!("```json golden={name}\n");
  let rest = &SPEC[SPEC.find(&start).unwrap_or_else(|| panic!("the block {name}")) + start.len()..];
  serde_json::from_str(&rest[..rest.find("```").expect("the end of the block")]).expect("JSON")
}

fn keys(value: &Value) -> BTreeSet<String> {
  value.as_object().expect("an object").keys().cloned().collect()
}

/// The properties of the root object of a JSON schema, or of the definition that `#/$defs/<name>` names
fn properties<'a>(schema: &'a Value, name: Option<&str>) -> &'a Value {
  let object = name.map_or(schema, |name| &schema["$defs"][name]);
  &object["properties"]
}

/// `bd-host schema` lists the table of the spec, code by code, and the report and the hello have the fields of the
/// golden JSON of the spec, no more and no less
#[test]
fn schema_lists_every_code_and_the_shapes_of_the_report_and_the_hello() {
  let output = serde_json::to_value(schema()).expect("JSON");
  // the test of host-link pins the table of the codes to the table "Refusal codes" of the spec
  let codes = output["codes"].as_array().expect("the codes");
  assert_eq!(codes.len(), Code::ALL.len());
  for (listed, code) in codes.iter().zip(Code::ALL) {
    assert_eq!(listed["code"].as_str(), Some(code.name()));
    assert_eq!(listed["exit"].as_str(), Some(code.row().exit.text()), "{code}");
    assert_eq!(listed["words"].as_str(), Some(code.row().words), "{code}");
  }
  assert_eq!((output["protocol"]["schema"].as_u64(), output["protocol"]["port"].as_u64()), (Some(1), Some(7742)));
  let exits: Vec<u64> = output["exits"].as_array().expect("the exits").iter().filter_map(|exit| exit["exit"].as_u64()).collect();
  assert_eq!(exits, [2, 75, 130, 255]);

  let report = golden("run-report");
  assert_eq!(keys(properties(&output["report"], None)), keys(&report));
  let refusal_codes = output["report"]["$defs"]["RefusalCode"]["enum"].as_array().expect("the codes of a refusal");
  assert_eq!(refusal_codes.len(), Code::ALL.len());

  let hello = golden("hello");
  let schema = &output["hello"];
  assert_eq!(keys(properties(schema, None)), keys(&hello));
  assert_eq!(keys(properties(schema, Some("AgentHello"))), keys(&hello["agent"]));
  assert_eq!(keys(properties(schema, Some("ClientHello"))), keys(&hello["client"]));
  assert_eq!(
    keys(properties(&output["hello_refusal"], None)),
    ["alias", "refusal", "schema"].map(str::to_owned).into_iter().collect()
  );
}
