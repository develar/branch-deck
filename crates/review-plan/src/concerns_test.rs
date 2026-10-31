use crate::ReviewPlan;
use crate::concerns::{ConcernName, EvidenceKind};
use crate::test_support::{plan_of, range};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

fn plan(repo: &TestRepo, base: &str, head: &str) -> ReviewPlan {
  plan_of(repo, None, &range(repo, base, head)).unwrap()
}

/// Each concern of `plan`: its name and its files
fn concerns(plan: &ReviewPlan) -> Vec<(ConcernName, Vec<&str>)> {
  plan
    .concerns
    .iter()
    .map(|concern| (concern.name.clone(), concern.paths.iter().map(String::as_str).collect()))
    .collect()
}

fn token(token: &str) -> ConcernName {
  ConcernName::Token { token: token.to_string() }
}

fn declaration(name: &str) -> ConcernName {
  ConcernName::Declaration { name: name.to_string() }
}

const MCP_JSON: &str = "{\n  \"mcpServers\": {\n    \"Glean\": {\n      \"type\": \"http\"\n    },\n    \"ijproxy\": {\n      \"type\": \"stdio\",\n      \"command\": \"bun\",\n      \"args\": [\"community/build/mcp-servers/ij-proxy/dist/index.mjs\"]\n    }\n  }\n}\n";
const CODEX_TOML: &str = "[mcp_servers.glean]\nurl = \"https://glean\"\n\n[mcp_servers.ijproxy]\ntype = \"stdio\"\ncommand = \"node\"\nargs = [\"../community/build/mcp-servers/ij-proxy/dist/ij-mcp-proxy.mjs\"]\n";
const GUIDE: &str = "# Guide\n\nSearch the source with grep.\n\nKeep the rules short.\n";

/// A server dropped from configs of three formats in three directories, one of them deleted whole, is one concern by
/// the identifier their removed lines share; an edit of a guide that names none of it is not part of it
#[test]
fn the_configs_that_drop_one_server_are_one_concern_by_the_identifier_they_share() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files(
    "Add the configs",
    &[
      (".mcp.json", MCP_JSON),
      (".codex/config.toml", CODEX_TOML),
      (
        "dotnet/.codex/config.toml",
        "[mcp_servers.ijproxy]\ntype = \"stdio\"\ncommand = \"node\"\nargs = [\"../community/build/mcp-servers/ij-proxy/dist/ij-mcp-proxy.mjs\"]\n",
      ),
      ("docs/guide.md", GUIDE),
    ],
  );
  repo.git(&["rm", "-q", "dotnet/.codex/config.toml"]);
  let head = repo.create_commit_with_files(
    "Drop ijproxy",
    &[
      (".mcp.json", "{\n  \"mcpServers\": {\n    \"Glean\": {\n      \"type\": \"http\"\n    }\n  }\n}\n"),
      (".codex/config.toml", "[mcp_servers.glean]\nurl = \"https://glean\"\n"),
      ("docs/guide.md", &GUIDE.replace("grep", "accio")),
    ],
  );
  let plan = plan(&repo, &base, &head);
  assert_eq!(
    concerns(&plan),
    [
      (token("ijproxy"), vec![".codex/config.toml", "dotnet/.codex/config.toml", ".mcp.json"]),
      (ConcernName::Rest, vec!["docs/guide.md"]),
    ]
  );
  assert_eq!(plan.concerns[0].evidence[0].kind, EvidenceKind::Token);
}

/// A changed declaration and the changes that name it, in its file or a file that imports it, are one concern; a
/// change elsewhere is not
#[test]
fn a_changed_declaration_and_the_changes_of_its_uses_are_one_concern() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files(
    "Add",
    &[
      ("src/a/Api.kt", "package a\n\nfun open(path: String): Int = 1\n"),
      ("src/b/Use.kt", "package b\n\nimport a.open\n\nfun use() = open(\"x\")\n"),
      ("src/c/Other.kt", "package c\n\nfun other() = 1\n"),
    ],
  );
  let head = repo.create_commit_with_files(
    "Change",
    &[
      ("src/a/Api.kt", "package a\n\nfun open(path: String, mode: Int): Int = mode\n"),
      ("src/b/Use.kt", "package b\n\nimport a.open\n\nfun use() = open(\"x\", 2)\n"),
      ("src/c/Other.kt", "package c\n\nfun other() = 2\n"),
    ],
  );
  let plan = plan(&repo, &base, &head);
  assert_eq!(
    concerns(&plan),
    [(declaration("open"), vec!["src/a/Api.kt", "src/b/Use.kt"]), (ConcernName::Rest, vec!["src/c/Other.kt"])]
  );
}

/// A declaration that more than six changes name is a hub: it joins none of them
#[test]
fn a_declaration_named_by_many_changes_joins_none() {
  let repo = TestRepo::new();
  let users: Vec<(String, String)> = (0..7)
    .map(|index| (format!("src/u/U{index}.kt"), format!("package u\n\nimport a.open\n\nfun u{index}() = open(\"{index}\")\n")))
    .collect();
  let mut files: Vec<(&str, &str)> = vec![("src/a/Api.kt", "package a\n\nfun open(path: String): Int = 1\n")];
  files.extend(users.iter().map(|(path, text)| (path.as_str(), text.as_str())));
  let base = repo.create_commit_with_files("Add", &files);
  let changed: Vec<(String, String)> = users.iter().map(|(path, text)| (path.clone(), text.replace("\")", "\", 2)"))).collect();
  let mut files: Vec<(&str, &str)> = vec![("src/a/Api.kt", "package a\n\nfun open(path: String, mode: Int): Int = mode\n")];
  files.extend(changed.iter().map(|(path, text)| (path.as_str(), text.as_str())));
  let head = repo.create_commit_with_files("Change", &files);
  let plan = plan(&repo, &base, &head);
  assert!(plan.concerns.iter().all(|concern| concern.name == ConcernName::Rest), "{:#?}", plan.concerns);
}

/// A test with no evidence of its own goes to the concern of its subject, and a change alone in a file to the concern of
/// that file
#[test]
fn a_test_follows_its_subject_and_a_change_alone_the_concern_of_its_file() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files(
    "Add",
    &[
      ("src/a/Api.kt", "package a\n\nfun open(path: String): Int = 1\n\nfun close() = 1\n"),
      ("src/b/Use.kt", "package b\n\nimport a.open\n\nfun use() = open(\"x\")\n"),
      ("src/test/a/ApiTest.kt", "package a\n\nfun testClose() = check(1)\n"),
    ],
  );
  let head = repo.create_commit_with_files(
    "Change",
    &[
      ("src/a/Api.kt", "package a\n\nfun open(path: String, mode: Int): Int = mode\n\nfun close() = 2\n"),
      ("src/b/Use.kt", "package b\n\nimport a.open\n\nfun use() = open(\"x\", 2)\n"),
      ("src/test/a/ApiTest.kt", "package a\n\nfun testClose() = check(2)\n"),
    ],
  );
  let plan = plan(&repo, &base, &head);
  assert_eq!(concerns(&plan), [(declaration("open"), vec!["src/a/Api.kt", "src/test/a/ApiTest.kt", "src/b/Use.kt"])]);
  let evidence: Vec<EvidenceKind> = plan.concerns[0].evidence.iter().map(|evidence| evidence.kind).collect();
  assert!(evidence.contains(&EvidenceKind::Rest), "{evidence:?}");
}

/// A generated file whose header names a changed source follows the concern of that source, folded at its end
#[test]
fn a_generated_file_follows_the_concern_of_the_source_its_header_names() {
  let agents = |tool: &str| format!("<!-- Generated from docs/guide.md by tools/render.mjs -->\n\nSearch the source with {tool}.\n");
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files(
    "Add",
    &[
      ("docs/guide.md", "# Guide\n\nSearch the source with grep.\n"),
      ("docs/tools.md", "# Tools\n\n- grep\n"),
      ("AGENTS.md", &agents("grep")),
    ],
  );
  let head = repo.create_commit_with_files(
    "Search with accio",
    &[
      ("docs/guide.md", "# Guide\n\nSearch the source with accioquery.\n"),
      ("docs/tools.md", "# Tools\n\n- accioquery\n"),
      ("AGENTS.md", &agents("accioquery")),
    ],
  );
  let plan = plan(&repo, &base, &head);
  let replacement = ConcernName::Replacement {
    removed: "grep".to_string(),
    added: "accioquery".to_string(),
  };
  assert_eq!(concerns(&plan), [(replacement, vec!["docs/guide.md", "docs/tools.md", "AGENTS.md"])]);
}

/// The changes that share the most rare identifiers join first, and one shared identifier then joins no two concerns of
/// more than one change each
#[test]
fn one_shared_identifier_bridges_no_two_concerns() {
  let lines = |words: &[&str]| words.iter().map(|word| ["- ", word, "\n"].concat()).collect::<String>();
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files(
    "Add",
    &[("a/one.md", "# One\n"), ("a/two.md", "# Two\n"), ("b/three.md", "# Three\n"), ("b/four.md", "# Four\n")],
  );
  let head = repo.create_commit_with_files(
    "Change",
    &[
      ("a/one.md", &format!("# One\n{}", lines(&["zebrafish", "otter", "kestrel"]))),
      ("a/two.md", &format!("# Two\n{}", lines(&["zebrafish", "otter"]))),
      ("b/three.md", &format!("# Three\n{}", lines(&["quokka", "lemur", "kestrel"]))),
      ("b/four.md", &format!("# Four\n{}", lines(&["quokka", "lemur"]))),
    ],
  );
  let plan = plan(&repo, &base, &head);
  assert_eq!(
    concerns(&plan),
    [(token("zebrafish"), vec!["a/one.md", "a/two.md"]), (token("quokka"), vec!["b/four.md", "b/three.md"])]
  );
}

/// A concern keeps its id while it grows, and its fingerprint names the set of its parts
#[test]
fn a_concern_keeps_its_id_as_it_grows() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add", &[("a/one.md", "# One\n"), ("b/two.md", "# Two\n"), ("c/three.md", "# Three\n")]);
  let two = repo.create_commit_with_files("Change", &[("a/one.md", "# One\n- zebrafish\n"), ("b/two.md", "# Two\n- zebrafish\n")]);
  let three = repo.create_commit_with_files("Change more", &[("c/three.md", "# Three\n- zebrafish\n")]);
  let (first, second) = (plan(&repo, &base, &two), plan(&repo, &base, &three));
  assert_eq!(first.concerns[0].id, second.concerns[0].id);
  assert_ne!(first.concerns[0].fingerprint, second.concerns[0].fingerprint);
}
