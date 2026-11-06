use crate::{CommitCheck, MessageRules, Severity, load_rules, parse_commit_check, parse_patronus_rules, read_commit_check};
use pretty_assertions::assert_eq;

/// The tail of `.patronus/config.yaml` of the IntelliJ monorepo, with its `commitMessageFormat` block as it is
const PATRONUS_CONFIG: &str = include_str!("fixtures/patronus-config.yaml");

fn patronus() -> MessageRules {
  parse_patronus_rules(PATRONUS_CONFIG).unwrap()
}

fn paths(paths: &[&str]) -> Vec<String> {
  paths.iter().map(|path| (*path).to_string()).collect()
}

/// The rules of the monorepo: a subject without a ticket and a label is STRICT; a label, a ticket and a revert pass; a
/// marker goes before the check; a rule matches the whole message, so "swipe" in a body is no `WIP`
#[test]
fn the_rules_of_the_monorepo_refuse_a_subject_without_a_ticket_or_a_label() {
  let rules = patronus();
  let code = paths(&["platform/core/src/A.kt"]);
  for (message, verdict) in [
    ("write the pinned Qodana date", Some(Severity::Strict)),
    ("setup build: write the pinned Qodana date", None),
    ("setup build: write the pinned Qodana date\n\nThe body says why.\n", None),
    ("IJAI-5865 session links: open in the project", None),
    ("fixup! setup build: write the pinned Qodana date", None),
    ("squash! fixup! IJAI-5865 x", None),
    ("misc ui: fix the swipe gesture\n\nThe swipe of a WIP state.", None),
    ("WIP setup build: x", Some(Severity::Strict)),
    ("wip IJAI-1 x", Some(Severity::Strict)),
    ("Revert \"IJAI-1 x\"", None),
    ("docs(agents): the skill says why", Some(Severity::Strict)),
    ("feat(brd): a pick refuses a message", Some(Severity::Strict)),
  ] {
    assert_eq!(rules.check(message, &code), verdict, "{message:?}");
  }
  assert_eq!(
    rules.hint.as_deref().map(|hint| hint.starts_with("Commit messages should contain a YouTrack ticket")),
    Some(true)
  );
  assert_eq!(rules.documentation.as_deref(), Some("https://youtrack.jetbrains.com/articles/IJPL-A-217/Commits"));
}

/// A severity rule applies when every changed path matches it: only `/fleet/**` is IGNORE, only `/dotnet/**` WARNING, a
/// change in `/fleet/` and `/platform/` takes the default, STRICT
#[test]
fn the_paths_of_a_commit_decide_its_severity() {
  let rules = patronus();
  let bad = "write the date";
  assert_eq!(rules.check(bad, &paths(&["fleet/a/B.kt", "fleet/c.txt"])), None);
  assert_eq!(rules.check(bad, &paths(&[".air/x.md"])), None);
  assert_eq!(rules.check(bad, &paths(&["dotnet/Rider/a.cs"])), Some(Severity::Warning));
  assert_eq!(rules.check(bad, &paths(&["fleet/a.kt", "platform/b.kt"])), Some(Severity::Strict));
  assert_eq!(rules.check(bad, &paths(&["fleet/a.kt", "dotnet/b.cs"])), Some(Severity::Strict));
  assert_eq!(rules.check(bad, &paths(&["fleetx/a.kt"])), Some(Severity::Strict));
  assert_eq!(rules.check(bad, &[]), Some(Severity::Strict), "no paths: the default");
}

/// The content of `.patronus/config.yaml` fails open: each cause is the reason of a skipped check
#[test]
fn a_patronus_config_that_cannot_give_rules_skips_the_check_with_its_cause() {
  for (text, says) in [
    ("version: \"1.0\"\nchecks: []\n", "has no commitMessageFormat"),
    ("commitMessageFormat:\n  rules: [\n", "does not parse"),
    ("commitMessageFormat:\n  rules:\n  - regex: \"(unclosed\"\n", "does not compile"),
    ("commitMessageFormat:\n  severityDefault: \"LOUD\"\n", "unknown severity"),
  ] {
    let error = parse_patronus_rules(text).unwrap_err();
    assert!(error.contains(says), "{text:?}: {error}");
  }
  let checkout = tempfile::tempdir().unwrap();
  let error = load_rules(checkout.path(), &CommitCheck::Patronus).unwrap_err();
  assert!(error.contains(".patronus/config.yaml cannot be read"), "{error}");
}

/// The block ends at the next top-level key, so a later block of the same shape is not read
#[test]
fn only_the_top_level_block_of_the_format_is_read() {
  let text = "a: 1\ncommitMessageFormat:\n  rules:\n  - regex: \"ok.*\"\n\n# a comment\nlater:\n  rules:\n  - regex: \"(unclosed\"\n";
  let rules = parse_patronus_rules(text).unwrap();
  assert_eq!(rules.check("ok then", &[]), None);
  assert_eq!(rules.check("no", &[]), Some(Severity::Strict));
  let flow = "commitMessageFormat: {rules: [{regex: \"x.*\"}], severityDefault: \"WARNING\"}\nnext: 2\n";
  assert_eq!(parse_patronus_rules(flow).unwrap().check("y", &[]), Some(Severity::Warning));
  let warning_only = "commitMessageFormat:\n  warningOnly: true\n  rules:\n  - regex: \"x.*\"\n";
  assert_eq!(parse_patronus_rules(warning_only).unwrap().check("y", &[]), Some(Severity::Warning));
}

/// `[commit] check` is "patronus" or regexes of its own; absent, no check; a bad value or key is an error of the file
#[test]
fn the_commit_table_names_the_rules_and_a_bad_value_is_an_error() {
  assert_eq!(parse_commit_check("").unwrap(), None);
  assert_eq!(parse_commit_check("[lease]\nclean = []\n[commit]\n").unwrap(), None);
  assert_eq!(parse_commit_check("[commit]\ncheck = \"patronus\"\n").unwrap(), Some(CommitCheck::Patronus));
  assert_eq!(
    parse_commit_check("[commit]\ncheck = [\"(feat|fix)\\\\(.+\\\\): .+\"]\n").unwrap(),
    Some(CommitCheck::Rules(vec![r"(feat|fix)\(.+\): .+".to_string()]))
  );
  for (text, says) in [
    ("[commit]\ncheck = 5\n", "it is \"patronus\" or an array of regexes"),
    ("[commit]\ncheck = \"gitlint\"\n", "it is \"patronus\" or an array of regexes"),
    ("[commit]\ncheck = [5]\n", "not a regex"),
    ("[commit]\ncheck = [\"(unclosed\"]\n", "does not compile"),
    ("[commit]\ncheck = []\n", "names no regex"),
    ("[commit]\nguide = \"x\"\n", "unknown key commit.guide"),
    ("commit = 1\n", "`commit` is not a table"),
  ] {
    let error = format!("{:#}", parse_commit_check(text).unwrap_err());
    assert!(error.contains(says), "{text:?}: {error}");
  }
}

/// Regexes of `[commit] check`: one must match, a violation is STRICT, the hint names them
#[test]
fn the_own_regexes_of_a_repository_are_strict() {
  let checkout = tempfile::tempdir().unwrap();
  std::fs::write(checkout.path().join(".branchdeck.toml"), "[commit]\ncheck = [\"(feat|fix|docs)(\\\\(.+\\\\))?!?: .+\"]\n").unwrap();
  let check = read_commit_check(checkout.path()).unwrap().unwrap();
  let rules = load_rules(checkout.path(), &check).unwrap();
  assert_eq!(rules.check("feat(brd): x\n\nbody", &paths(&["a"])), None);
  assert_eq!(rules.check("fixup! fix: y", &[]), None);
  assert_eq!(rules.check("add x", &paths(&["a"])), Some(Severity::Strict));
  assert!(rules.hint.unwrap().contains("[commit] check"));
  assert_eq!(read_commit_check(&checkout.path().join("missing")).unwrap(), None);
}

/// The `[commit]` of Branch Deck itself: the conventional subjects of its `git log` pass, a plain one is STRICT
#[test]
fn the_rules_of_this_repository_take_its_conventional_subjects() {
  let check = parse_commit_check(include_str!("../../../.branchdeck.toml")).unwrap().unwrap();
  let rules = load_rules(std::path::Path::new("."), &check).unwrap();
  for subject in [
    "feat(brd): a pick refuses a message that the commit rules of the repository reject",
    "fix(e2e): wait for the toast",
    "feat(bd)!: rename the agent CLI to brd",
    "chore(master): release 1.1.0",
    "docs(agents): the skill says why",
    "Revert \"feat(bd): search review story text\"",
  ] {
    assert_eq!(rules.check(subject, &paths(&["crates/a.rs"])), None, "{subject}");
  }
  assert_eq!(rules.check("add the commit rules", &paths(&["crates/a.rs"])), Some(Severity::Strict));
}
