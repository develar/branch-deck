use crate::{check_old_layout, check_open_spec, check_requirement_groups, check_spec_anchors, check_spec_targets, check_test_links, heading_anchors, openspec_validate};
use serde_json::{Value, json};
use std::cell::RefCell;
use std::path::Path;

/// A repository of `files` (path → text) in a temporary directory, removed when it drops
fn repository_of(files: &[(&str, &str)]) -> tempfile::TempDir {
  let root = tempfile::tempdir().expect("a temporary directory");
  for (path, text) in files {
    let file = root.path().join(path);
    std::fs::create_dir_all(file.parent().expect("a parent")).expect("its directory");
    std::fs::write(file, text).expect("its text");
  }
  root
}

/// A spec whose front matter lists `targets`
fn spec_of(targets: &[&str], body: &str) -> String {
  let mut lines = vec!["---".to_string(), "name: Example".to_string(), "targets:".to_string()];
  lines.extend(targets.iter().map(|target| format!("  - {target}")));
  lines.extend(["---".to_string(), String::new(), body.to_string(), String::new()]);
  lines.join("\n")
}

const SPEC: &str = "openspec/specs/area/example/spec.md";

#[test]
fn a_test_link_names_a_file_and_the_tests_it_declares_on_a_line_of_its_own() {
  let spec = spec_of(
    &["src/example.test.ts"],
    &[
      "- A requirement.",
      "  [@test] src/example.test.ts (`shows the row`, `hides the row`)",
      "  [@test] crates/example/src/lib_test.rs (`keeps_the_row`)",
      "  [@test] src/missing.test.ts (`shows the row`)",
      "- Another one. [@test] src/example.test.ts (`shows the row`)",
    ]
    .join("\n"),
  );
  let root = repository_of(&[
    ("src/example.test.ts", "test(\"shows the row\", () => {})\n"),
    ("crates/example/src/lib_test.rs", "#[test]\nfn keeps_the_row() {}\n"),
    (SPEC, &spec),
    ("docs/design/example.md", "  [@test] ../../src/example.test.ts (`hides the row`)\n"),
  ]);
  assert_eq!(
    check_test_links(root.path()),
    [
      "docs/design/example.md:1: src/example.test.ts has no test `hides the row`".to_string(),
      format!("{SPEC}:8: src/example.test.ts has no test `hides the row`"),
      format!("{SPEC}:10: no file src/missing.test.ts"),
      format!("{SPEC}:11: not a link of its own line, `[@test] <path> (`name`, …)`: - Another one. [@test] src/example.test.ts (`shows the row`)"),
    ]
  );
}

#[test]
fn every_spec_lists_its_targets_once_each_a_path_from_the_root_of_a_file_that_exists() {
  let root = repository_of(&[
    ("src/example.ts", ""),
    ("src/dir/file.ts", ""),
    ("openspec/specs/area/good/spec.md", &spec_of(&["src/example.ts"], "- A requirement.")),
    (
      "openspec/specs/area/bad/spec.md",
      &spec_of(
        &["src/example.ts", "src/example.ts", "src/gone.ts", "../../../../src/example.ts", "src/dir"],
        "- A requirement.",
      ),
    ),
    ("openspec/specs/area/no-front-matter/spec.md", "# Example\n"),
    ("openspec/specs/area/no-targets/spec.md", "---\nname: Example\n---\n"),
    ("openspec/specs/area/empty-targets/spec.md", "---\nname: Example\ntargets:\nstyle: plain-1\n---\n"),
    ("openspec/specs/area/odd-line/spec.md", "---\ntargets:\n  - src/example.ts\n  # a comment\n---\n"),
  ]);
  assert_eq!(
    check_spec_targets(root.path()),
    [
      "openspec/specs/area/bad/spec.md:5: lists src/example.ts twice",
      "openspec/specs/area/bad/spec.md:6: no file src/gone.ts",
      "openspec/specs/area/bad/spec.md:7: the target ../../../../src/example.ts is not from the root of the repository, as every path under openspec/",
      "openspec/specs/area/bad/spec.md:8: src/dir is no file",
      "openspec/specs/area/empty-targets/spec.md: its `targets:` list no file",
      "openspec/specs/area/no-front-matter/spec.md: no front matter (`---` … `---`)",
      "openspec/specs/area/no-targets/spec.md: its front matter lists no `targets:`",
      "openspec/specs/area/odd-line/spec.md:4: not a target line, `  - <path>`: # a comment",
    ]
  );
}

#[test]
fn a_rule_of_domain_policy_names_its_specs_in_its_header_and_each_of_them_lists_it() {
  let rules = "crates/domain-policy/src";
  let specs = "openspec/specs/area";
  let root = repository_of(&[
    (&format!("{rules}/lib.rs"), "pub mod listed;\n"),
    (
      &format!("{rules}/listed.rs"),
      &format!("//! The listed rule ({specs}/rules/spec.md).\n\npub fn rule() {{}}\n"),
    ),
    (&format!("{rules}/listed_test.rs"), "#[test]\nfn decides() {}\n"),
    (
      &format!("{rules}/unlisted.rs"),
      &format!("//! A rule of two specs ({specs}/rules/spec.md, {specs}/other/spec.md).\n"),
    ),
    (&format!("{rules}/unnamed.rs"), "//! A rule that names no spec.\n"),
    (
      &format!("{rules}/below.rs"),
      &format!("//! A rule whose spec is named below its header.\n\n// {specs}/rules/spec.md\n"),
    ),
    (&format!("{rules}/missing.rs"), &format!("//! A rule of a spec that is gone ({specs}/gone/spec.md).\n")),
    (
      &format!("{specs}/rules/spec.md"),
      &spec_of(
        &[&format!("{rules}/listed.rs"), &format!("{rules}/unlisted.rs"), &format!("{rules}/below.rs")],
        "- A requirement.",
      ),
    ),
    (&format!("{specs}/other/spec.md"), &spec_of(&[&format!("{rules}/lib.rs")], "- A requirement.")),
  ]);
  assert_eq!(
    check_spec_targets(root.path()),
    [
      format!("{rules}/below.rs: its `//!` header names no spec (openspec/specs/<area>/<name>/spec.md) of the rule"),
      format!("{rules}/missing.rs: its `//!` header names {specs}/gone/spec.md, which does not exist"),
      format!("{specs}/other/spec.md: no target {rules}/unlisted.rs, whose `//!` header names the spec"),
      format!("{rules}/unnamed.rs: its `//!` header names no spec (openspec/specs/<area>/<name>/spec.md) of the rule"),
    ]
  );
}

#[test]
fn every_presenter_of_the_page_is_a_target_of_a_spec_its_tests_aside() {
  let root = repository_of(&[
    ("src/presenters/listed.ts", "export const words = 1\n"),
    ("src/presenters/listed.test.ts", ""),
    ("src/presenters/unlisted.ts", "export const words = 2\n"),
    ("src/presenters/unlisted.test.ts", ""),
    (SPEC, &spec_of(&["src/presenters/listed.ts"], "- A requirement.")),
  ]);
  assert_eq!(
    check_spec_targets(root.path()),
    ["src/presenters/unlisted.ts: a presenter that the `targets:` of no spec list"]
  );
}

/// Every check names a file by its path from the root with `/`, on Windows too, where the walk joins its directories
#[test]
fn every_check_answers_the_paths_with_a_slash() {
  let rules = "crates/domain-policy/src";
  let spec = spec_of(
    &[&format!("{rules}/listed.rs"), "src/presenters/listed.ts"],
    &["- A requirement.", "  [@test] src/example.test.ts (`shows the row`, `hides the row`)"].join("\n"),
  );
  let root = repository_of(&[
    (&format!("{rules}/lib.rs"), "pub mod listed;\n"),
    (&format!("{rules}/listed.rs"), &format!("//! The listed rule ({SPEC}).\n")),
    (&format!("{rules}/unlisted.rs"), &format!("//! A rule the spec doesn't list ({SPEC}).\n")),
    ("src/presenters/listed.ts", ""),
    ("src/presenters/unlisted.ts", ""),
    ("src/example.test.ts", "test(\"shows the row\", () => {})\n"),
    (SPEC, &spec),
  ]);
  assert_eq!(
    check_spec_targets(root.path()),
    [
      format!("{SPEC}: no target {rules}/unlisted.rs, whose `//!` header names the spec"),
      "src/presenters/unlisted.ts: a presenter that the `targets:` of no spec list".to_string()
    ]
  );
  assert_eq!(check_test_links(root.path()), [format!("{SPEC}:9: src/example.test.ts has no test `hides the row`")]);
  let groups = repository_of(&[(SPEC, "### Requirement: A: One\n### Requirement: B: Two\n### Requirement: A: Three\n")]);
  assert_eq!(
    check_requirement_groups(groups.path()),
    [format!("{SPEC}:3: the requirements of the group \"A\" are not together")]
  );
}

/// A spec of OpenSpec whose front matter lists `targets` and whose requirements are `requirements`
fn open_spec_of(targets: &[&str], requirements: &str) -> String {
  let mut lines = vec!["---".to_string(), "name: Example".to_string(), "targets:".to_string()];
  lines.extend(targets.iter().map(|target| format!("  - {target}")));
  lines.extend(
    [
      "---",
      "",
      "# Example",
      "",
      "## Purpose",
      "",
      "What the example capability is for, in a sentence long enough for the strict validation.",
      "",
      "## Requirements",
      "",
      requirements,
      "",
    ]
    .map(str::to_string),
  );
  lines.join("\n")
}

const REQUIREMENT: &str = "### Requirement: Rows: The row shows its name

The row MUST show the name of its branch.

#### Scenario: A branch row
- **WHEN** the table lists a branch
- **THEN** its row shows its name
  [@test] src/example.test.ts (`shows the row`)";

#[test]
fn under_openspec_a_test_path_is_from_the_root_a_path_from_the_file_is_a_problem() {
  let root = repository_of(&[
    ("src/example.test.ts", "test(\"shows the row\", () => {})\n"),
    (
      "openspec/specs/area/example/spec.md",
      &open_spec_of(
        &["src/example.test.ts"],
        &format!("{REQUIREMENT}\n  [@test] ../../../../src/example.test.ts (`shows the row`)"),
      ),
    ),
    (
      "openspec/changes/add-row/specs/area/example/spec.md",
      "## ADDED Requirements\n\n  [@test] src/example.test.ts (`hides the row`)\n",
    ),
    (
      "openspec/changes/archive/2026-11-09-old/specs/area/example/spec.md",
      "  [@test] src/gone.test.ts (`shows the row`)\n",
    ),
  ]);
  assert_eq!(
    check_test_links(root.path()),
    [
      "openspec/changes/add-row/specs/area/example/spec.md:3: src/example.test.ts has no test `hides the row`",
      "openspec/specs/area/example/spec.md:23: the path ../../../../src/example.test.ts is not from the root of the repository, as every path under openspec/",
    ]
  );
}

#[test]
fn the_anchors_of_the_headings_are_those_github_gives_them_a_repeated_one_numbered_none_of_fenced_code() {
  let text = [
    "# Title",
    "### Requirement: `brd push`: Refuses ‹branch›, “quoted”",
    "## Repeated",
    "## Repeated",
    "```md",
    "## Not a heading",
    "```",
  ]
  .join("\n");
  assert_eq!(heading_anchors(&text), ["title", "requirement-brd-push-refuses-branch-quoted", "repeated", "repeated-1"]);
}

#[test]
fn a_link_into_a_spec_names_one_of_its_headings_or_a_group_of_its_requirements() {
  let other = open_spec_of(
    &[],
    &[
      REQUIREMENT.replace("Rows: The row shows its name", "Other: One"),
      "See [rows](../example/spec.md#rows) and [gone](../example/spec.md#gone).".to_string(),
    ]
    .join("\n"),
  );
  let root = repository_of(&[
    ("openspec/specs/area/example/spec.md", &open_spec_of(&[], REQUIREMENT)),
    ("openspec/specs/area/other/spec.md", &other),
    (
      "docs/design/example.md",
      &[
        "[purpose](/openspec/specs/area/example/spec.md#purpose), [requirement](../../openspec/specs/area/example/spec.md#requirement-rows-the-row-shows-its-name)",
        "[missing](../../openspec/specs/area/missing/spec.md), [plain](../../openspec/specs/area/example/spec.md)",
      ]
      .join("\n"),
    ),
    ("openspec/changes/archive/2026-11-09-old/proposal.md", "[gone](../../../specs/area/example/spec.md#gone)\n"),
  ]);
  assert_eq!(
    check_spec_anchors(root.path()),
    [
      "docs/design/example.md:2: no file openspec/specs/area/missing/spec.md",
      "openspec/specs/area/other/spec.md:22: openspec/specs/area/example/spec.md has no heading of the anchor #gone",
    ]
  );
}

#[test]
fn openspec_validate_runs_over_every_spec_and_the_changes_nothing_without_openspec() {
  let root = repository_of(&[
    ("openspec/specs/area/example/spec.md", &open_spec_of(&[], REQUIREMENT)),
    ("openspec/specs/area/weak/spec.md", &open_spec_of(&[], "### Requirement: Rows: Weak\n\nThe row shows.\n")),
  ]);
  assert_eq!(
    check_open_spec(root.path(), openspec_validate),
    [
      "openspec spec area/weak: ERROR requirements.0.scenarios: Requirement must have at least one scenario",
      "openspec spec area/weak: WARNING requirements[0].scenarios: Requirement must have at least one scenario. Scenarios must use level-4 headers. Convert bullet lists into:",
      "openspec spec area/weak: WARNING requirements[0]: Requirement \"Rows: Weak\" should contain SHALL or MUST (RFC 2119 best practice for English specs)",
    ]
  );
  let asked = RefCell::new(Vec::new());
  let validate = |_: &Path, args: &[&str]| -> Result<Value, String> {
    asked.borrow_mut().push(args.join(" "));
    Ok(json!({ "items": [{ "id": "add-row", "type": "change", "valid": false, "issues": [] }] }))
  };
  let changes = repository_of(&[("openspec/changes/add-row/proposal.md", "## Why\n")]);
  assert_eq!(check_open_spec(changes.path(), validate), ["openspec change add-row: invalid"]);
  assert_eq!(*asked.borrow(), ["--changes"]);
  assert_eq!(check_open_spec(repository_of(&[("README.md", "")]).path(), validate), Vec::<String>::new());
  assert_eq!(*asked.borrow(), ["--changes"]);
}

#[test]
fn no_spec_lies_under_spec_and_no_file_names_one_by_its_old_path_a_fixture_of_another_repository_and_the_history_aside() {
  // built of its parts, as this file names no spec of the old layout itself
  let old = ["spec", "branch", "unapply.spec.md"].join("/");
  let files = [
    (old.clone(), String::new()),
    ("crates/a/src/lib.rs".to_string(), format!("//! Unapply ({old}).\n")),
    ("docs/design/a.md".to_string(), format!("x\n[unapply](../../{old}#undo)\n")),
    ("src/fixture.test.ts".to_string(), "const path = \"plugins/air/spec/frame/window.spec.md\"\n".to_string()),
    ("CHANGELOG.md".to_string(), format!("{old}\n")),
    ("openspec/changes/archive/2026-11-09-a/proposal.md".to_string(), format!("{old}\n")),
    ("openspec/specs/branch/unapply/spec.md".to_string(), String::new()),
  ];
  let root = repository_of(&files.iter().map(|(path, text)| (path.as_str(), text.as_str())).collect::<Vec<_>>());
  let paths: Vec<String> = files.iter().map(|(path, _)| path.clone()).collect();
  assert_eq!(
    check_old_layout(root.path(), &paths),
    [
      format!("{old}: a spec of the old layout, which moved to openspec/specs/<area>/<name>/spec.md (docs/contributing/specs.md)"),
      format!("crates/a/src/lib.rs:1: names {old}, a spec of the old layout: openspec/specs/<area>/<name>/spec.md"),
      format!("docs/design/a.md:2: names {old}, a spec of the old layout: openspec/specs/<area>/<name>/spec.md"),
    ]
  );
}

#[test]
fn the_requirements_of_a_group_stand_together_in_a_spec_the_untitled_group_too_a_heading_of_fenced_code_aside() {
  let root = repository_of(&[
    (
      "openspec/specs/area/together/spec.md",
      &[
        "### Requirement: Rows: One",
        "### Requirement: Rows: Two",
        "### Requirement: `brd push`: Refuses: a dirty tree",
        "### Requirement: An untitled one",
        "```md",
        "### Requirement: Rows: Inside a fence",
        "```",
        "### Requirement: Another untitled one",
      ]
      .join("\n"),
    ),
    (
      "openspec/specs/area/apart/spec.md",
      &[
        "### Requirement: Rows: One",
        "### Requirement: Undo: One",
        "### Requirement: Rows: Two",
        "### Requirement: An untitled one",
        "### Requirement: Undo: Two",
        "### Requirement: Another untitled one",
      ]
      .join("\n"),
    ),
  ]);
  assert_eq!(
    check_requirement_groups(root.path()),
    [
      "openspec/specs/area/apart/spec.md:3: the requirements of the group \"Rows\" are not together",
      "openspec/specs/area/apart/spec.md:5: the requirements of the group \"Undo\" are not together",
      "openspec/specs/area/apart/spec.md:6: the requirements of the group \"\" are not together",
    ]
  );
}
