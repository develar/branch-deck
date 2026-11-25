use crate::checks::parse_patronus_checks;
use pretty_assertions::assert_eq;

/// Checks as the file of the IntelliJ monorepo writes them, their rules cut to the shapes it has: anchored directories,
/// a name at any depth, `*` of the files of one directory, the negations of the documentation after them, an optional
/// check, a check of every change
const CONFIG: &str = r#"version: "1.0"
checks:
- configurationId: "ijplatform_master_Air_Plugin_Tests_Aggregator"
  attempts: 3
  executeOnChanges:
  - ".idea/libraries/**"
  - "CIDR/*"
  - "plugins/air/**"
  - ".bazelrc"
  - "!/**/[Dd]ocs/**/*.md"
  - "!/**/README.[Mm][Dd]"
  - "!/.patronus/**.md"
  massiveBuildFailureDetector:
    enabled: true
  name: "Air Plugin Tests"
  passParams: {}
- configurationId: "ijplatform_master_AIAssistantTestsPatronusCheck"
  executeOnChanges:
  - "plugins/llm/**"
  - "build/"
  name: "AI Assistant Tests"
- configurationId: "ijplatform_master_AirPluginUiTestsOptional"
  executeOnChanges:
  - "plugins/air/**"
  optional: true
  name: "Air Plugin UI Tests Optional"
  group:
    id: "air"
- configurationId: "ijplatform_master_ZeroToleranceInspections"
  name: "Zero Tolerance Inspections"
mergeCommitPolicy:
  allowedPaths: []
"#;

fn started(paths: &[&str]) -> Vec<String> {
  let checks = parse_patronus_checks(CONFIG).unwrap();
  checks.started_by(paths).map(|check| check.name.clone()).collect()
}

/// The last rule that matches decides: a file of the Air plugin starts its checks and the one of every change, its
/// documentation only the one of every change; a rule anchors at the root, a name matches at any depth, `*` the files of
/// one directory, a trailing `/` no file
#[test]
fn a_change_starts_the_checks_whose_last_matching_rule_is_not_negated() {
  let air = ["Air Plugin Tests", "Air Plugin UI Tests Optional", "Zero Tolerance Inspections"];
  assert_eq!(started(&["plugins/air/core/src/Session.kt"]), air);
  assert_eq!(
    started(&["plugins/air/docs/guide/Start.md"]),
    ["Air Plugin UI Tests Optional", "Zero Tolerance Inspections"]
  );
  assert_eq!(started(&["plugins/air/README.md"]), ["Air Plugin UI Tests Optional", "Zero Tolerance Inspections"]);
  assert_eq!(started(&["CIDR/build.gradle"]), ["Air Plugin Tests", "Zero Tolerance Inspections"]);
  assert_eq!(started(&["CIDR/core/src/A.kt"]), ["Zero Tolerance Inspections"], "`*` is one directory");
  assert_eq!(started(&["a/b/.bazelrc"]), ["Air Plugin Tests", "Zero Tolerance Inspections"], "a name at any depth");
  assert_eq!(started(&["xplugins/air/A.kt"]), ["Zero Tolerance Inspections"], "anchored at the root");
  assert_eq!(started(&["build/src/A.kt"]), ["Zero Tolerance Inspections"], "a directory rule matches no file");
  assert_eq!(started(&[".idea/libraries"]), ["Zero Tolerance Inspections"], "a trailing `**` is what is inside");
  assert_eq!(started(&[".patronus/rules.md"]), ["Zero Tolerance Inspections"], "`**` inside a segment is `*`");
  assert_eq!(
    started(&["plugins/llm/A.kt", "plugins/air/B.kt"]),
    ["Air Plugin Tests", "AI Assistant Tests", "Air Plugin UI Tests Optional", "Zero Tolerance Inspections"]
  );
}

/// What the price reads of a check: its id, its name (the id without one) and whether it is optional
#[test]
fn a_check_has_its_id_its_name_and_whether_it_is_optional() {
  let checks = parse_patronus_checks(CONFIG).unwrap();
  let read: Vec<(&str, &str, bool)> = checks
    .checks
    .iter()
    .map(|check| (check.configuration_id.as_str(), check.name.as_str(), check.optional))
    .collect();
  assert_eq!(
    read,
    [
      ("ijplatform_master_Air_Plugin_Tests_Aggregator", "Air Plugin Tests", false),
      ("ijplatform_master_AIAssistantTestsPatronusCheck", "AI Assistant Tests", false),
      ("ijplatform_master_AirPluginUiTestsOptional", "Air Plugin UI Tests Optional", true),
      ("ijplatform_master_ZeroToleranceInspections", "Zero Tolerance Inspections", false),
    ]
  );
}

/// The checks fail open: a file without them, or one that does not parse, is the reason they are unknown
#[test]
fn checks_that_cannot_be_read_are_unknown_with_the_reason() {
  assert_eq!(parse_patronus_checks("version: \"1.0\"\n").unwrap_err(), ".patronus/config.yaml has no checks");
  let broken = parse_patronus_checks("checks:\n- name: \"no id\"\n").unwrap_err();
  assert!(broken.starts_with("checks of .patronus/config.yaml does not parse"), "{broken}");
}
