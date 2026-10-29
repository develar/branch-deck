use crate::check::{CheckOutcome, CheckReport, check_plan};
use crate::dangling_references::{DanglingKind, DanglingReference, DeclaredIn, read_commit_lines};
use crate::diff_scan_test::unhurried;
use object_cache::cache::ObjectCache;
use object_cache::commit_lines::FileLines;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use test_log::test;
use test_utils::git_test_utils::{TestRepo, append_config};
use test_utils::history::{HistoryCommit, HistoryStream, one_file_each};

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
  let (repo, [_, removed_by, fixed_by]) = TestRepo::with_commits(&[
    HistoryCommit::new("Initial commit", &[(AGENT, AGENT_WITH_CONSTANTS), (HOST, &host("AcpScenarioAgent"))]),
    HistoryCommit::new(
      "(fake-agent) IJAI-3880 Move the control protocol constants",
      &[(AGENT, AGENT_WITHOUT_CONSTANTS), (PROTOCOL, PROTOCOL_WITH_CONSTANTS)],
    ),
    HistoryCommit::new("(lane-document) IJAI-3878 Use the control protocol", &[(HOST, &host("AcpControlProtocol"))]),
  ]);
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
      kind: DanglingKind::Removed,
      identifier: "CONTROL_URL_ENV".to_string(),
      file: HOST.to_string(),
      line: 7,
      removed_by,
      fixed_by: Some(fixed_by),
      on_baseline: false,
      possible: false,
      // the fix uses `AcpControlProtocol`, which the branch adds: only moving the fix into the branch helps
      mutual: true,
      by_syntax: true,
      declared_in: DeclaredIn {
        file: AGENT.to_string(),
        line: Some(4),
      },
    }]
  );
  assert!(!report.dangling.contains_key("lane-document"), "{:?}", report.dangling);
}

/// The incident with a space in the path of the use: git ends such a name with a tab in the `+++` line, and the path of
/// the use still has the commit that fixes it
#[test]
fn a_use_in_a_path_with_a_space_is_fixed_by_its_commit() {
  let spaced_host = "plugins/air/tests/flow/Air Flow Lane Host.kt";
  let (repo, [_, _, fixed_by]) = TestRepo::with_commits(&[
    HistoryCommit::new("Initial commit", &[(AGENT, AGENT_WITH_CONSTANTS), (spaced_host, &host("AcpScenarioAgent"))]),
    HistoryCommit::new(
      "(fake-agent) IJAI-3880 Move the control protocol constants",
      &[(AGENT, AGENT_WITHOUT_CONSTANTS), (PROTOCOL, PROTOCOL_WITH_CONSTANTS)],
    ),
    HistoryCommit::new("(lane-document) IJAI-3878 Use the control protocol", &[(spaced_host, &host("AcpControlProtocol"))]),
  ]);

  let report = check(&repo);

  let found: Vec<(String, Option<String>)> = report
    .dangling
    .get("fake-agent")
    .into_iter()
    .flatten()
    .map(|reference| (reference.file.clone(), reference.fixed_by.clone()))
    .collect();
  assert_eq!(found, [(spaced_host.to_string(), Some(fixed_by))]);
}

/// The changed lines of one commit, without caches
fn commit_lines(repo: &TestRepo, commit: &str) -> HashMap<String, Vec<FileLines>> {
  read_commit_lines(repo.git_executor(), repo.path_str(), &[commit.to_string()], &ObjectCache::disabled(), &unhurried()).unwrap()
}

/// The changed lines of a commit by the paths of its `--raw` entries: a name with a space or a quote is read as git
/// stores it, and a type change from a symlink, whose side has no text, is the lines of the new file
#[test]
fn the_changed_lines_are_read_by_the_paths_as_stored() {
  let repo = TestRepo::new();
  let blob = |content: &str| {
    repo
      .git_executor()
      .cmd(&["hash-object", "-w", "--stdin"], repo.path_str())
      .stdin(content.as_bytes())
      .text()
      .unwrap()
  };
  let target = blob("target.kt");
  repo.git(&["update-index", "--add", "--cacheinfo", &format!("120000,{target},link.kt")]);
  repo.create_commit_in_index("Base", &[("z.kt", "val z = 1\n")]);
  let commit = repo.create_commit_in_index(
    "Change",
    &[
      ("link.kt", "val linked = 1\n"),
      ("q\"uote.kt", "val quoted = 1\n"),
      ("My File.kt", "fun spaced() {}\n"),
      ("z.kt", "val z = 2\n"),
    ],
  );

  let mut lines = commit_lines(&repo, &commit);

  let file = |path: &str, removed: &[&str], added: &[&str]| FileLines {
    path: path.to_string(),
    removed: removed.iter().map(ToString::to_string).collect(),
    added: added.iter().map(ToString::to_string).collect(),
  };
  assert_eq!(
    lines.remove(&commit).unwrap(),
    [
      file("My File.kt", &[], &["fun spaced() {}"]),
      file("link.kt", &[], &["val linked = 1"]),
      file("q\"uote.kt", &[], &["val quoted = 1"]),
      file("z.kt", &["val z = 1"], &["val z = 2"]),
    ]
  );
}

/// The cached lines don't depend on the user's diff config: `diff.indentHeuristic=false` slides an added function so
/// that its lines come in another order, `diff.interHunkContext` fuses hunks
#[test]
fn the_changed_lines_do_not_depend_on_the_diff_config() {
  let (repo, [_, commit]) = TestRepo::with_commits(&one_file_each([
    ("Base", "f.kt", "1\n2\n/* function */\nfoo() {\n\tfoo\n}\n\n3\n4\n"),
    ("Add bar", "f.kt", "1\n2\n/* function */\nbar() {\n\tfoo\n}\n\n/* function */\nfoo() {\n\tfoo\n}\n\n3\n4\n"),
  ]));
  append_config(repo.path(), &[("diff.indentHeuristic", "false"), ("diff.interHunkContext", "10")]);

  let mut lines = commit_lines(&repo, &commit);

  let added: Vec<String> = ["/* function */", "bar() {", "\tfoo", "}", ""].iter().map(ToString::to_string).collect();
  assert_eq!(
    lines.remove(&commit).unwrap(),
    [FileLines {
      path: "f.kt".to_string(),
      removed: Vec::new(),
      added,
    }]
  );
}

/// The fix in the same branch: nothing dangles
#[test]
fn a_use_changed_by_the_branch_itself_is_not_dangling() {
  let (repo, _) = TestRepo::with_commits(&[
    HistoryCommit::new("Initial commit", &[(AGENT, AGENT_WITH_CONSTANTS), (HOST, &host("AcpScenarioAgent"))]),
    HistoryCommit::new(
      "(fake-agent) Move the control protocol constants",
      &[(AGENT, AGENT_WITHOUT_CONSTANTS), (PROTOCOL, PROTOCOL_WITH_CONSTANTS)],
    ),
    HistoryCommit::new("(fake-agent) Use the control protocol", &[(HOST, &host("AcpControlProtocol"))]),
  ]);
  assert!(check(&repo).dangling.is_empty());
}

/// A declaration changed in place (or one overload of several) is still declared in its file
#[test]
fn a_declaration_changed_in_place_is_not_dangling() {
  let changed = AGENT_WITH_CONSTANTS.replace("\"AIR_CONTROL_URL\"", "\"AIR_CONTROL_URL_V2\"");
  let (repo, _) = TestRepo::with_commits(&[
    HistoryCommit::new("Initial commit", &[(AGENT, AGENT_WITH_CONSTANTS), (HOST, &host("AcpScenarioAgent"))]),
    HistoryCommit::new("(fake-agent) Change the variable", &[(AGENT, &changed)]),
    HistoryCommit::new("(other) Drop the host", &[(HOST, "package flow\n")]),
  ]);
  assert!(check(&repo).dangling.is_empty());
}

/// Another branch only reformats a line that uses the name: HEAD has as many uses, so the split breaks nothing that
/// HEAD doesn't break too
#[test]
fn a_reformatted_use_is_not_dangling() {
  let reformatted = host("AcpScenarioAgent").replace(
    "mapOf(AcpScenarioAgent.CONTROL_URL_ENV to url)",
    "mapOf(\n      AcpScenarioAgent.CONTROL_URL_ENV to url,\n    )",
  );
  let (repo, _) = TestRepo::with_commits(&[
    HistoryCommit::new("Initial commit", &[(AGENT, AGENT_WITH_CONSTANTS), (HOST, &host("AcpScenarioAgent"))]),
    HistoryCommit::new("(fake-agent) Drop the constants", &[(AGENT, AGENT_WITHOUT_CONSTANTS), (PROTOCOL, PROTOCOL_WITH_CONSTANTS)]),
    HistoryCommit::new("(other) Reformat", &[(HOST, &reformatted)]),
  ]);
  assert!(check(&repo).dangling.is_empty());
}

/// A private member is used in its own file only: a use elsewhere is another name
#[test]
fn a_private_declaration_is_not_searched_outside_its_file() {
  let agent = "package acp;\n\npublic final class AcpScenarioAgent {\n  private static final String CONTROL_URL_ENV = \"AIR\";\n}\n";
  let other = "package other;\n\nclass Other {\n  static final String CONTROL_URL_ENV = \"OTHER\";\n  String url() { return CONTROL_URL_ENV; }\n}\n";
  let changed = other.replace("return CONTROL_URL_ENV;", "return \"\";");
  let (repo, _) = TestRepo::with_commits(&[
    HistoryCommit::new("Initial commit", &[(AGENT, agent), ("src/other/Other.java", other)]),
    HistoryCommit::new("(fake-agent) Drop the constant", &[(AGENT, "package acp;\n\npublic final class AcpScenarioAgent {\n}\n")]),
    HistoryCommit::new("(other) Change other", &[("src/other/Other.java", &changed)]),
  ]);
  assert!(check(&repo).dangling.is_empty());
}

/// A removed top-level function that another branch stops calling: a whole-word search; the fix needs nothing of the
/// branch, so stacking helps
#[test]
fn a_removed_top_level_function_is_searched_by_its_name() {
  let util = "package util\n\nfun controlUrl(): String = \"\"\n\nfun other(): String = \"\"\n";
  let user = "package flow\n\nimport util.controlUrl\n\nfun start() = controlUrl()\n";
  let (repo, [_, removed_by, fixed_by]) = TestRepo::with_commits(&[
    HistoryCommit::new("Initial commit", &[("src/util/Util.kt", util), ("src/flow/Start.kt", user)]),
    HistoryCommit::new("(cleanup) Drop controlUrl", &[("src/util/Util.kt", "package util\n\nfun other(): String = \"\"\n")]),
    HistoryCommit::new("(flow) Start without it", &[("src/flow/Start.kt", "package flow\n\nfun start() = \"\"\n")]),
  ]);

  let dangling = check(&repo).dangling;
  assert_eq!(
    dangling.get("cleanup").cloned().unwrap_or_default(),
    [DanglingReference {
      kind: DanglingKind::Removed,
      identifier: "controlUrl".to_string(),
      file: "src/flow/Start.kt".to_string(),
      line: 3,
      removed_by,
      fixed_by: Some(fixed_by),
      on_baseline: false,
      possible: false,
      mutual: false,
      by_syntax: true,
      declared_in: DeclaredIn {
        file: "src/util/Util.kt".to_string(),
        line: Some(3),
      },
    }]
  );
}

/// A use that came with the baseline after the base: Patronus merges onto the tip, where it breaks; no commit here
/// fixes it, so it is only possible
#[test]
fn a_use_landed_on_the_baseline_is_a_possible_reference() {
  let repo = TestRepo::new();
  // upstream adds a use after the base: `origin/master` has it, `master` is the base
  let mut history = HistoryStream::new("refs/remotes/origin/master");
  history.commit(&HistoryCommit::new("Initial commit", &[(AGENT, AGENT_WITH_CONSTANTS), (HOST, "package flow\n")]));
  history.commit(&HistoryCommit::new("Use the URL", &[(HOST, &host("AcpScenarioAgent"))]));
  history.reset_to_root("refs/heads/master");
  history.write(repo.path()).unwrap();
  append_config(repo.path(), &[("remote.origin.url", "."), ("remote.origin.fetch", "+refs/heads/*:refs/remotes/origin/*")]);
  let [removed_by] = repo.commits_on(
    "refs/heads/master",
    &[HistoryCommit::new(
      "(fake-agent) Drop the constants",
      &[(AGENT, AGENT_WITHOUT_CONSTANTS), (PROTOCOL, PROTOCOL_WITH_CONSTANTS)],
    )],
  );

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
  let json = |package: &str| format!("package {package};\n\npublic final class Json {{\n  public static String quote(String value) {{\n    return value;\n  }}\n}}\n");
  let user = "package flow\n\nimport protocol.Json\n\nfun shown(name: String) = Json.quote(name)\n";
  let (repo, _) = TestRepo::with_commits(&[
    HistoryCommit::new("Initial commit", &[("old/support/Json.java", &json("support")), ("src/flow/Other.kt", "package flow\n")]),
    // the move, and a user of the new place in the same branch
    HistoryCommit::new("(protocol) Move Json", &[("new/protocol/Json.java", &json("protocol")), ("src/flow/Shown.kt", user)]),
    HistoryCommit::new("(protocol) Drop the old Json", &[("src/flow/Other.kt", "package flow\n\n// moved\n")]).deleting(&["old/support/Json.java"]),
    HistoryCommit::new("(other) Drop the user", &[("src/flow/Shown.kt", "package flow\n")]),
  ]);
  assert!(check(&repo).dangling.is_empty(), "{:?}", check(&repo).dangling);
}

/// A top-level function moved to another file of its package, and a property of the same name left in the old file:
/// the function still resolves, and the property is no use of it
#[test]
fn a_top_level_function_moved_within_its_package_is_not_dangling() {
  let dsl = "package acp\n\nfun requestKey(value: String): String = value\n\nclass Permission(\n  val requestKey: String,\n)\n";
  let (repo, _) = TestRepo::with_commits(&[
    HistoryCommit::new("Initial commit", &[("src/acp/Dsl.kt", dsl)]),
    HistoryCommit::new(
      "(keys) Move requestKey",
      &[
        ("src/acp/Dsl.kt", "package acp\n\nclass Permission(\n  val requestKey: String,\n)\n"),
        ("src/acp/Keys.kt", "package acp\n\nfun requestKey(value: String): String = value\n"),
      ],
    ),
    HistoryCommit::new("(other) Drop the property", &[("src/acp/Dsl.kt", "package acp\n\nclass Permission\n")]),
  ]);
  assert!(check(&repo).dangling.is_empty(), "{:?}", check(&repo).dangling);
}

/// A private function moved to another file and made public there: its old file imports it from the new place
#[test]
fn a_private_declaration_moved_elsewhere_is_not_dangling() {
  let utils = "package flow\n\nprivate fun String.listedIdentity(): String = this\n\nfun shown(name: String) = name.listedIdentity()\n";
  let (repo, _) = TestRepo::with_commits(&[
    HistoryCommit::new("Initial commit", &[("src/flow/Utils.kt", utils)]),
    HistoryCommit::new(
      "(authored) Share listedIdentity",
      &[
        ("src/bridge/Authored.kt", "package bridge\n\nfun String.listedIdentity(): String = this\n"),
        (
          "src/flow/Utils.kt",
          "package flow\n\nimport bridge.listedIdentity\n\nfun shown(name: String) = name.listedIdentity()\n",
        ),
      ],
    ),
    HistoryCommit::new("(other) Drop the utils", &[("src/flow/Utils.kt", "package flow\n")]),
  ]);
  assert!(check(&repo).dangling.is_empty(), "{:?}", check(&repo).dangling);
}

/// The lines find a dangling reference of `branch` (the symbols off), and the syntax trees take it away
fn assert_found_by_the_lines_only(repo: &TestRepo, branch: &str) {
  repo.set_config("branchdeck.symbols", "off").unwrap();
  let by_lines = check(repo).dangling;
  assert!(by_lines.contains_key(branch), "the lines alone find it: {by_lines:?}");
  repo.set_config("branchdeck.symbols", "report").unwrap();
  let by_syntax = check(repo).dangling;
  assert!(!by_syntax.contains_key(branch), "{by_syntax:?}");
}

/// The IntelliJ checkout, 2026-10-17 (a): the branch replaces its enum by a `pub(crate) use` of the one another crate
/// declares; the name stays bound in its file, so the uses still compile
#[test]
fn a_name_re_exported_by_a_use_is_not_removed() {
  let identity = "crates/guest/src/lane/identity.rs";
  let context = "crates/guest/src/lane/context.rs";
  let (repo, _) = TestRepo::with_commits(&[
    HistoryCommit::new(
      "Initial commit",
      &[
        ("crates/guest/Cargo.toml", "[package]\nname = \"guest\"\n"),
        (identity, "pub(crate) enum ControlOwner {\n  Agent,\n}\n"),
        (
          context,
          "use super::identity::ControlOwner;\n\npub fn owner() -> ControlOwner {\n  ControlOwner::Agent\n}\n",
        ),
      ],
    ),
    HistoryCommit::new("(wire) Use the owner of the wire", &[(identity, "pub(crate) use wire::program::lane::ControlOwner;\n")]),
    HistoryCommit::new(
      "(context) Name the owner once",
      &[(context, "use super::identity::ControlOwner;\n\npub fn owner() -> ControlOwner {\n  wire::agent()\n}\n")],
    ),
  ]);
  assert_found_by_the_lines_only(&repo, "wire");
}

/// (b): a removed function named in a doc comment only: a comment is no use
#[test]
fn a_word_in_kdoc_is_no_use() {
  let (repo, _) = TestRepo::with_commits(&[
    HistoryCommit::new(
      "Initial commit",
      &[
        ("src/util/Util.kt", "package util\n\nfun controlUrl(): String = \"\"\n"),
        (
          "src/flow/Start.kt",
          "package flow\n\n/** Starts without [controlUrl] and its controlUrl. */\nfun start() = 1\n",
        ),
      ],
    ),
    HistoryCommit::new("(cleanup) Drop controlUrl", &[("src/util/Util.kt", "package util\n")]),
    HistoryCommit::new("(flow) Document start", &[("src/flow/Start.kt", "package flow\n\n/** Starts. */\nfun start() = 1\n")]),
  ]);
  assert_found_by_the_lines_only(&repo, "cleanup");
}

/// A removed function of the same name as an `override` member that another branch drops: the line search takes
/// `override fun` for a use, the syntax tree has it as an override, which is no use of a top-level function
#[test]
fn an_override_line_is_no_use() {
  let agent = |body: &str| format!("package flow\n\nclass Agent : Base() {{\n{body}}}\n");
  let (repo, _) = TestRepo::with_commits(&[
    HistoryCommit::new(
      "Initial commit",
      &[
        ("src/util/Util.kt", "package util\n\nfun controlUrl(): String = \"\"\n"),
        ("src/flow/Agent.kt", &agent("  override fun controlUrl(): String = \"agent\"\n")),
      ],
    ),
    HistoryCommit::new("(cleanup) Drop controlUrl", &[("src/util/Util.kt", "package util\n")]),
    HistoryCommit::new("(flow) Drop the override", &[("src/flow/Agent.kt", &agent(""))]),
  ]);
  assert_found_by_the_lines_only(&repo, "cleanup");
}

/// (c): a local `val` of another class in the file shadows the private property the branch removes, and a property of
/// the same name in another class is no use of it either
#[test]
fn a_local_val_shadows_a_private_member_of_another_class() {
  let dsl = "src/journey/Dsl.kt";
  let journey = |handle: &str, turn: &str| {
    format!("package journey\n\nclass Handle {{\n{handle}  fun id() = 1\n}}\n\nclass Turn {{\n  fun run(): Int {{\n    val turnId = 2\n{turn}    return turnId\n  }}\n}}\n")
  };
  let (repo, _) = TestRepo::with_commits(&one_file_each([
    ("Initial commit", dsl, &journey("  private val turnId: Int = 1\n", "    println(turnId)\n")),
    ("(handle) Drop the turn id", dsl, &journey("", "    println(turnId)\n")),
    ("(turn) Print nothing", dsl, &journey("", "")),
  ]));
  assert_found_by_the_lines_only(&repo, "handle");
}

const GUEST_LIB: &str = "crates/avl-guest/src/lib.rs";
const GUEST_SUM: &str = "crates/avl-guest/src/sum.rs";
const VM_TESTS: &str = "crates/avl-vm/src/bench/classes/tests.rs";

/// Two crates of a workspace (the IntelliJ checkout, 2026-10-19): `(cleanup)` removes `collect` (declared as
/// `declaration`) from `avl-guest`, whose `sum.rs` calls it; `(vm)` changes `sum.rs` and the file `vm_tests` of
/// `avl-vm`. Returns `(the commit that removes it, the commit that changes the uses)`
fn two_crates(declaration: &str, vm_tests: &str) -> (TestRepo, String, String) {
  let lib = format!("{declaration} {{\n  values.iter().sum()\n}}\n\npub fn other() {{}}\n");
  let (repo, [_, removed_by, fixed_by]) = TestRepo::with_commits(&[
    HistoryCommit::new(
      "Initial commit",
      &[
        ("crates/avl-guest/Cargo.toml", "[package]\nname = \"avl-guest\"\n"),
        (GUEST_LIB, &lib),
        (GUEST_SUM, "use crate::collect;\n\npub fn total() -> u32 {\n  collect(&[1, 2])\n}\n"),
        ("crates/avl-vm/Cargo.toml", "[package]\nname = \"avl-vm\"\n"),
        (VM_TESTS, vm_tests),
      ],
    ),
    HistoryCommit::new("(cleanup) Drop collect", &[(GUEST_LIB, "pub fn other() {}\n")]),
    HistoryCommit::new(
      "(vm) Count without collect",
      &[(GUEST_SUM, "pub fn total() -> u32 {\n  3\n}\n"), (VM_TESTS, "fn names() {}\n")],
    ),
  ]);
  (repo, removed_by, fixed_by)
}

/// The files of the dangling references of `(cleanup)`
fn files_of_cleanup(repo: &TestRepo) -> Vec<String> {
  check(repo).dangling.get("cleanup").into_iter().flatten().map(|reference| reference.file.clone()).collect()
}

/// The incident of 2026-10-19: `.collect()` of an iterator in another crate is a method call, no use of the removed
/// free function, by the lines alone too; the call in its own crate still is one
#[test]
fn a_method_call_is_no_use_of_a_removed_free_function() {
  let vm_tests = "fn names() -> Vec<String> {\n  let all: Vec<u32> = (0..3).collect();\n  let _ = all.iter().copied().collect::<Vec<_>>();\n  all\n    .iter()\n    .map(u32::to_string)\n    .collect()\n}\n";
  let (repo, ..) = two_crates("pub fn collect(values: &[u32]) -> u32", vm_tests);
  for mode in ["off", "on"] {
    repo.set_config("branchdeck.symbols", mode).unwrap();
    assert_eq!(files_of_cleanup(&repo), [GUEST_SUM], "branchdeck.symbols {mode}");
  }
}

/// With the crates known (`branchdeck.symbols on`), a bare call in another crate uses the removed function only where
/// the file has a `use` of its crate that names it or a glob; a path through the crate or a module that a `use` of it
/// binds uses it too. With the symbols off the crates are not known, and the bare call counts
#[test]
fn a_free_call_in_another_crate_without_a_use_is_no_use() {
  let found = |vm_tests: &str, mode: &str| {
    let (repo, ..) = two_crates("pub fn collect(values: &[u32]) -> u32", vm_tests);
    append_config(repo.path(), &[("branchdeck.symbols", mode)]);
    files_of_cleanup(&repo)
  };
  let bare = "fn total() -> u32 {\n  collect(&[1])\n}\n";
  assert_eq!(found(bare, "on"), [GUEST_SUM]);
  assert_eq!(found(bare, "off"), [GUEST_SUM, VM_TESTS]);
  for imported in [
    format!("use avl_guest::*;\n\n{bare}"),
    format!("use avl_guest::{{\n  collect,\n  other,\n}};\n\n{bare}"),
    "fn total() -> u32 {\n  avl_guest::collect(&[1])\n}\n".to_string(),
    "use avl_guest as guest;\nuse ::avl_guest::{self as lib};\n\nfn total() -> u32 {\n  lib::collect(&[1])\n}\n".to_string(),
  ] {
    assert_eq!(found(&imported, "on"), [GUEST_SUM, VM_TESTS], "{imported}");
  }
}

/// A call in the crate of the removed function is a use: the reference names the commit that removes it and where it
/// was declared
#[test]
fn a_free_call_in_the_same_crate_is_a_use_with_removed_by_and_declared_in() {
  let (repo, removed_by, fixed_by) = two_crates("pub fn collect(values: &[u32]) -> u32", "fn names() -> Vec<u32> {\n  Vec::new()\n}\n");
  assert_eq!(
    check(&repo).dangling.get("cleanup").cloned().unwrap_or_default(),
    [DanglingReference {
      kind: DanglingKind::Removed,
      identifier: "collect".to_string(),
      file: GUEST_SUM.to_string(),
      line: 1,
      removed_by,
      fixed_by: Some(fixed_by),
      on_baseline: false,
      possible: false,
      mutual: false,
      by_syntax: true,
      declared_in: DeclaredIn {
        file: GUEST_LIB.to_string(),
        line: Some(1),
      },
    }]
  );
}

/// `pub(crate)` ends at its crate: another crate's `use` of it is no use of it; `pub(self)` is private
#[test]
fn a_pub_crate_function_is_not_searched_in_another_crate() {
  let vm_tests = "use avl_guest::collect;\n\nfn total() -> u32 {\n  avl_guest::collect(&[1])\n}\n";
  for (declaration, found) in [
    ("pub(crate) fn collect(values: &[u32]) -> u32", vec![GUEST_SUM]),
    ("pub(in crate) fn collect(values: &[u32]) -> u32", vec![GUEST_SUM]),
    ("pub fn collect(values: &[u32]) -> u32", vec![GUEST_SUM, VM_TESTS]),
    // searched in its own file only
    ("pub(self) fn collect(values: &[u32]) -> u32", vec![]),
  ] {
    let (repo, ..) = two_crates(declaration, vm_tests);
    assert_eq!(files_of_cleanup(&repo), found, "{declaration}");
  }
}

/// A Kotlin extension is called on a receiver, so such a call stays a use; a top-level function without a receiver is
/// never called on an expression (`client.controlUrl()` is a member of `Client`), which the syntax trees tell, not the
/// lines
#[test]
fn a_call_on_an_expression_is_a_use_of_an_extension_only() {
  let util = "package util\n\nfun String.shout(): String = this\n\nfun controlUrl(): String = \"\"\n";
  let (repo, _) = TestRepo::with_commits(&[
    HistoryCommit::new(
      "Initial commit",
      &[
        ("src/util/Util.kt", util),
        ("src/flow/Start.kt", "package flow\n\nimport util.shout\n\nfun start(name: String) = name.shout()\n"),
        ("src/flow/Connect.kt", "package flow\n\nfun connect(client: Client) = client.controlUrl()\n"),
      ],
    ),
    HistoryCommit::new("(cleanup) Drop the helpers", &[("src/util/Util.kt", "package util\n")]),
    HistoryCommit::new(
      "(flow) Start and connect plainly",
      &[
        ("src/flow/Start.kt", "package flow\n\nfun start(name: String) = name\n"),
        ("src/flow/Connect.kt", "package flow\n\nfun connect(client: Client) = client\n"),
      ],
    ),
  ]);
  let found = |mode: &str| -> Vec<(String, String)> {
    repo.set_config("branchdeck.symbols", mode).unwrap();
    check(&repo)
      .dangling
      .get("cleanup")
      .into_iter()
      .flatten()
      .map(|reference| (reference.identifier.clone(), reference.file.clone()))
      .collect()
  };
  let found_by = |pairs: &[(&str, &str)]| -> Vec<(String, String)> { pairs.iter().map(|(name, file)| ((*name).to_string(), (*file).to_string())).collect() };
  assert_eq!(found("off"), found_by(&[("controlUrl", "src/flow/Connect.kt"), ("shout", "src/flow/Start.kt")]));
  assert_eq!(found("on"), found_by(&[("shout", "src/flow/Start.kt")]));
}

/// The IntelliJ checkout, 2026-10-19: `(stage)` deletes the module `stage/` of `avl-guest` with its `pub(crate) fn
/// collect`, so HEAD has no directory of the removed file; a test of `avl-vm` calls a `collect` of its own module
/// (`record::collect`) and an iterator's `.collect()`, and a commit outside the branch changes the call. The crate of
/// the removed file is still known, by its nearest directory HEAD has
#[test]
fn a_crate_item_of_a_deleted_module_is_not_searched_in_another_crate() {
  let gc = "crates/avl-guest/src/stage/gc.rs";
  let tests = "crates/avl-vm/src/bench/tests.rs";
  let uses = |call: &str| format!("fn run() {{\n  let run = {call};\n  let names: Vec<u32> = (0..2).collect();\n}}\n");
  let (repo, _) = TestRepo::with_commits(&[
    HistoryCommit::new(
      "Initial commit",
      &[
        ("crates/avl-guest/Cargo.toml", "[package]\nname = \"avl-guest\"\n"),
        ("crates/avl-guest/src/lib.rs", "mod stage;\n"),
        (gc, "pub(crate) fn collect(root: &str) -> u32 {\n  1\n}\n"),
        ("crates/avl-vm/Cargo.toml", "[package]\nname = \"avl-vm\"\n"),
        (tests, &uses("record::collect(\"session\")")),
      ],
    ),
    HistoryCommit::new("(stage) The guest has no stage", &[("crates/avl-guest/src/lib.rs", "pub mod fs;\n")]).deleting(&[gc]),
    HistoryCommit::new("(bench) Rank a plugin", &[(tests, &uses("record::read(\"session\")"))]),
  ]);
  for mode in ["on", "off"] {
    repo.set_config("branchdeck.symbols", mode).unwrap();
    let found: Vec<String> = check(&repo).dangling.get("stage").into_iter().flatten().map(|reference| reference.file.clone()).collect();
    // with the symbols off the crates are not known, and `record::collect` is a call of the name
    let expected: Vec<String> = if mode == "on" { Vec::new() } else { vec![tests.to_string()] };
    assert_eq!(found, expected, "branchdeck.symbols {mode}");
  }
}
