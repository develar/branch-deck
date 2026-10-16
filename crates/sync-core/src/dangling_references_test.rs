use crate::check::{CheckOutcome, CheckReport, check_plan};
use crate::dangling_references::DanglingReference;
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

const PREFIX: &str = "test-prefix";

const AGENT: &str = "plugins/air/src/acp/AcpScenarioAgent.java";
const PROTOCOL: &str = "plugins/air/src/acp/AcpControlProtocol.java";
const HOST: &str = "plugins/air/tests/flow/AirFlowLaneHost.kt";

const AGENT_WITH_CONSTANTS: &str = "package acp;

public final class AcpScenarioAgent {
  public static final String CONTROL_URL_ENV = \"AIR_CONTROL_URL\";
  public static final String CONTROL_AGENT_KEY_ENV = \"AIR_CONTROL_AGENT_KEY\";

  public static void main(String[] args) {
    System.out.println(System.getenv(CONTROL_URL_ENV));
  }
}
";

const AGENT_WITHOUT_CONSTANTS: &str = "package acp;

public final class AcpScenarioAgent {
  public static void main(String[] args) {
    System.out.println(System.getenv(AcpControlProtocol.CONTROL_URL_ENV));
  }
}
";

const PROTOCOL_WITH_CONSTANTS: &str = "package acp;

public final class AcpControlProtocol {
  public static final String CONTROL_URL_ENV = \"AIR_CONTROL_URL\";
  public static final String CONTROL_AGENT_KEY_ENV = \"AIR_CONTROL_AGENT_KEY\";
}
";

fn host(qualifier: &str) -> String {
  format!(
    "package flow

import acp.{qualifier}

class AirFlowLaneHost {{
  fun environment(url: String): Map<String, String> {{
    return mapOf({qualifier}.CONTROL_URL_ENV to url)
  }}
}}
"
  )
}

/// The incident of 2026-10-16: `(fake-agent)` moves two constants from `AcpScenarioAgent` into the new
/// `AcpControlProtocol`; the Kotlin file that used one was changed by a commit of `(lane-document)`
fn incident() -> (TestRepo, String, String) {
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial commit", &[(AGENT, AGENT_WITH_CONSTANTS), (HOST, &host("AcpScenarioAgent"))]);
  let removed_by = repo.create_commit_with_files(
    "(fake-agent) IJAI-3880 Move the control protocol constants",
    &[(AGENT, AGENT_WITHOUT_CONSTANTS), (PROTOCOL, PROTOCOL_WITH_CONSTANTS)],
  );
  let fixed_by = repo.create_commit_with_files("(lane-document) IJAI-3878 Use the control protocol", &[(HOST, &host("AcpControlProtocol"))]);
  (repo, removed_by, fixed_by)
}

fn check(repo: &TestRepo) -> CheckReport {
  check_plan(repo.git_executor(), repo.path().to_str().unwrap(), PREFIX, None, &ObjectCache::disabled()).unwrap()
}

#[test]
fn the_incident_a_moved_constant_whose_use_is_fixed_in_another_branch() {
  let (repo, removed_by, fixed_by) = incident();
  let report = check(&repo);
  assert!(report.branches.iter().all(|branch| matches!(branch.outcome, CheckOutcome::Ok { .. })), "{report:?}");
  assert_eq!(
    report.dangling.get("fake-agent").cloned().unwrap_or_default(),
    [DanglingReference {
      identifier: "CONTROL_URL_ENV".to_string(),
      file: HOST.to_string(),
      line: 7,
      removed_by,
      fixed_by: Some(fixed_by),
      on_baseline: false,
      // the fix uses `AcpControlProtocol`, which the branch adds: only moving the fix into the branch helps
      mutual: true,
    }]
  );
  assert!(!report.dangling.contains_key("lane-document"), "{:?}", report.dangling);
}

/// The fix in the same branch: nothing dangles
#[test]
fn a_use_changed_by_the_branch_itself_is_not_dangling() {
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial commit", &[(AGENT, AGENT_WITH_CONSTANTS), (HOST, &host("AcpScenarioAgent"))]);
  repo.create_commit_with_files(
    "(fake-agent) Move the control protocol constants",
    &[(AGENT, AGENT_WITHOUT_CONSTANTS), (PROTOCOL, PROTOCOL_WITH_CONSTANTS)],
  );
  repo.create_commit_with_files("(fake-agent) Use the control protocol", &[(HOST, &host("AcpControlProtocol"))]);
  assert!(check(&repo).dangling.is_empty());
}

/// A declaration changed in place (or one overload of several) is still declared in its file
#[test]
fn a_declaration_changed_in_place_is_not_dangling() {
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial commit", &[(AGENT, AGENT_WITH_CONSTANTS), (HOST, &host("AcpScenarioAgent"))]);
  let changed = AGENT_WITH_CONSTANTS.replace("\"AIR_CONTROL_URL\"", "\"AIR_CONTROL_URL_V2\"");
  repo.create_commit_with_files("(fake-agent) Change the variable", &[(AGENT, &changed)]);
  repo.create_commit_with_files("(other) Drop the host", &[(HOST, "package flow\n")]);
  assert!(check(&repo).dangling.is_empty());
}

/// Another branch only reformats a line that uses the name: HEAD has as many uses, so the split breaks nothing that
/// HEAD doesn't break too
#[test]
fn a_reformatted_use_is_not_dangling() {
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial commit", &[(AGENT, AGENT_WITH_CONSTANTS), (HOST, &host("AcpScenarioAgent"))]);
  repo.create_commit_with_files("(fake-agent) Drop the constants", &[(AGENT, AGENT_WITHOUT_CONSTANTS), (PROTOCOL, PROTOCOL_WITH_CONSTANTS)]);
  let reformatted = host("AcpScenarioAgent").replace(
    "mapOf(AcpScenarioAgent.CONTROL_URL_ENV to url)",
    "mapOf(\n      AcpScenarioAgent.CONTROL_URL_ENV to url,\n    )",
  );
  repo.create_commit_with_files("(other) Reformat", &[(HOST, &reformatted)]);
  assert!(check(&repo).dangling.is_empty());
}

/// A private member is used in its own file only: a use elsewhere is another name
#[test]
fn a_private_declaration_is_not_searched_outside_its_file() {
  let repo = TestRepo::new();
  let agent = "package acp;\n\npublic final class AcpScenarioAgent {\n  private static final String CONTROL_URL_ENV = \"AIR\";\n}\n";
  let other = "package other;\n\nclass Other {\n  static final String CONTROL_URL_ENV = \"OTHER\";\n  String url() { return CONTROL_URL_ENV; }\n}\n";
  repo.create_commit_with_files("Initial commit", &[(AGENT, agent), ("src/other/Other.java", other)]);
  repo.create_commit_with_files("(fake-agent) Drop the constant", &[(AGENT, "package acp;\n\npublic final class AcpScenarioAgent {\n}\n")]);
  let changed = other.replace("return CONTROL_URL_ENV;", "return \"\";");
  repo.create_commit_with_files("(other) Change other", &[("src/other/Other.java", &changed)]);
  assert!(check(&repo).dangling.is_empty());
}

/// A removed top-level function that another branch stops calling: a whole-word search; the fix needs nothing of the
/// branch, so stacking helps
#[test]
fn a_removed_top_level_function_is_searched_by_its_name() {
  let repo = TestRepo::new();
  let util = "package util\n\nfun controlUrl(): String = \"\"\n\nfun other(): String = \"\"\n";
  let user = "package flow\n\nimport util.controlUrl\n\nfun start() = controlUrl()\n";
  repo.create_commit_with_files("Initial commit", &[("src/util/Util.kt", util), ("src/flow/Start.kt", user)]);
  let removed_by = repo.create_commit_with_files("(cleanup) Drop controlUrl", &[("src/util/Util.kt", "package util\n\nfun other(): String = \"\"\n")]);
  let fixed_by = repo.create_commit_with_files("(flow) Start without it", &[("src/flow/Start.kt", "package flow\n\nfun start() = \"\"\n")]);

  let dangling = check(&repo).dangling;
  assert_eq!(
    dangling.get("cleanup").cloned().unwrap_or_default(),
    [DanglingReference {
      identifier: "controlUrl".to_string(),
      file: "src/flow/Start.kt".to_string(),
      line: 3,
      removed_by,
      fixed_by: Some(fixed_by),
      on_baseline: false,
      mutual: false,
    }]
  );
}

/// A use that came with the baseline after the base: Patronus merges onto the tip, where it breaks; no commit here
/// fixes it, so it is only possible
#[test]
fn a_use_landed_on_the_baseline_is_a_possible_reference() {
  let repo = TestRepo::new();
  let initial = repo.create_commit_with_files("Initial commit", &[(AGENT, AGENT_WITH_CONSTANTS), (HOST, "package flow\n")]);
  let path = repo.path().to_str().unwrap();
  let git = |args: &[&str]| repo.git_executor().execute_command(args, path).unwrap();
  // upstream adds a use after the base
  let upstream = repo.create_commit_with_files("Use the URL", &[(HOST, &host("AcpScenarioAgent"))]);
  git(&["remote", "add", "origin", "."]);
  git(&["update-ref", "refs/remotes/origin/master", &upstream]);
  git(&["reset", "-q", "--hard", &initial]);
  let removed_by = repo.create_commit_with_files("(fake-agent) Drop the constants", &[(AGENT, AGENT_WITHOUT_CONSTANTS), (PROTOCOL, PROTOCOL_WITH_CONSTANTS)]);

  let dangling = check(&repo).dangling;
  let references = &dangling["fake-agent"];
  assert_eq!(references.len(), 1, "{references:?}");
  assert_eq!(
    (
      references[0].file.as_str(),
      &references[0].removed_by,
      references[0].on_baseline,
      references[0].is_confirmed()
    ),
    (HOST, &removed_by, true, false)
  );
}

/// A class moved to another package with its members (the same file name): `Json.quote` still resolves, so a use that
/// another branch deletes is no dangling reference; only an import of the old package would be
#[test]
fn a_member_moved_with_its_class_is_not_dangling() {
  let repo = TestRepo::new();
  let json = |package: &str| format!("package {package};\n\npublic final class Json {{\n  public static String quote(String value) {{\n    return value;\n  }}\n}}\n");
  let user = "package flow\n\nimport protocol.Json\n\nfun shown(name: String) = Json.quote(name)\n";
  repo.create_commit_with_files("Initial commit", &[("old/support/Json.java", &json("support")), ("src/flow/Other.kt", "package flow\n")]);
  // the move, and a user of the new place in the same branch
  repo.create_commit_with_files("(protocol) Move Json", &[("new/protocol/Json.java", &json("protocol")), ("src/flow/Shown.kt", user)]);
  repo
    .git_executor()
    .execute_command(&["rm", "-q", "old/support/Json.java"], repo.path().to_str().unwrap())
    .unwrap();
  repo.create_commit_with_files("(protocol) Drop the old Json", &[("src/flow/Other.kt", "package flow\n\n// moved\n")]);
  repo.create_commit_with_files("(other) Drop the user", &[("src/flow/Shown.kt", "package flow\n")]);
  assert!(check(&repo).dangling.is_empty(), "{:?}", check(&repo).dangling);
}

/// A top-level function moved to another file of its package, and a property of the same name left in the old file:
/// the function still resolves, and the property is no use of it
#[test]
fn a_top_level_function_moved_within_its_package_is_not_dangling() {
  let repo = TestRepo::new();
  let dsl = "package acp\n\nfun requestKey(value: String): String = value\n\nclass Permission(\n  val requestKey: String,\n)\n";
  repo.create_commit_with_files("Initial commit", &[("src/acp/Dsl.kt", dsl)]);
  repo.create_commit_with_files(
    "(keys) Move requestKey",
    &[
      ("src/acp/Dsl.kt", "package acp\n\nclass Permission(\n  val requestKey: String,\n)\n"),
      ("src/acp/Keys.kt", "package acp\n\nfun requestKey(value: String): String = value\n"),
    ],
  );
  repo.create_commit_with_files("(other) Drop the property", &[("src/acp/Dsl.kt", "package acp\n\nclass Permission\n")]);
  assert!(check(&repo).dangling.is_empty(), "{:?}", check(&repo).dangling);
}

/// A private function moved to another file and made public there: its old file imports it from the new place
#[test]
fn a_private_declaration_moved_elsewhere_is_not_dangling() {
  let repo = TestRepo::new();
  let utils = "package flow\n\nprivate fun String.listedIdentity(): String = this\n\nfun shown(name: String) = name.listedIdentity()\n";
  repo.create_commit_with_files("Initial commit", &[("src/flow/Utils.kt", utils)]);
  repo.create_commit_with_files(
    "(authored) Share listedIdentity",
    &[
      ("src/bridge/Authored.kt", "package bridge\n\nfun String.listedIdentity(): String = this\n"),
      (
        "src/flow/Utils.kt",
        "package flow\n\nimport bridge.listedIdentity\n\nfun shown(name: String) = name.listedIdentity()\n",
      ),
    ],
  );
  repo.create_commit_with_files("(other) Drop the utils", &[("src/flow/Utils.kt", "package flow\n")]);
  assert!(check(&repo).dangling.is_empty(), "{:?}", check(&repo).dangling);
}
