use pretty_assertions::assert_eq;
use std::fs;

use super::*;

/// A copy of another build in `dir`: its own files with their marker. Answers its build
fn install_other(dir: &Path, files: &[(&str, &str)]) -> String {
  let mut marker = String::new();
  for (path, text) in files {
    write_whole(&dir.join(path), text.as_bytes()).expect("written");
    let _ = writeln!(marker, "{}  {path}", sha256_of(text.as_bytes()));
  }
  write_whole(&dir.join(MARKER), marker.as_bytes()).expect("written");
  build_of(marker.as_bytes())
}

fn text(dir: &Path, path: &str) -> String {
  fs::read_to_string(dir.join(path)).expect("readable")
}

/// A fresh install writes each file and the marker; an install of another build replaces it and removes the files of
/// that build only; a copy with local edits refuses with `skill_edited`, and `--force` replaces it
#[test]
fn skill_install_replaces_a_copy_of_another_build_and_keeps_local_edits() {
  let home = tempfile::tempdir().expect("a temporary directory");
  let dir = home.path().join("skills/bd-host");
  assert_eq!(install(&dir, false), Ok(Installed::Fresh));
  for (path, expected) in FILES {
    assert_eq!(text(&dir, path), expected, "{path}");
  }
  let marker = text(&dir, MARKER);
  assert_eq!(marker.lines().count(), FILES.len(), "{marker}");
  assert!(marker.starts_with(&format!("{}  SKILL.md\n", sha256_of(SKILL.as_bytes()))), "{marker}");
  assert_eq!(install(&dir, false), Ok(Installed::Current));

  let other = home.path().join("other");
  let old = install_other(&other, &[("SKILL.md", "old"), ("references/gone.md", "old reference")]);
  fs::write(other.join("notes.md"), "mine").expect("written");
  assert_ne!(old, build());
  assert_eq!(install(&other, false), Ok(Installed::Replaced(old)));
  assert_eq!(text(&other, "SKILL.md"), SKILL);
  assert!(!other.join("references/gone.md").exists(), "the file of the old build stays");
  assert_eq!(text(&other, "notes.md"), "mine", "a file of the user goes");

  fs::write(dir.join("SKILL.md"), "my own words").expect("written");
  let refusal = install(&dir, false).expect_err("refused");
  assert_eq!(refusal.code, Code::SkillEdited);
  assert!(refusal.message.contains("local edits (SKILL.md)"), "{}", refusal.message);
  assert!(refusal.message.contains("`bd-host skill install --force`"), "{}", refusal.message);
  assert_eq!(text(&dir, "SKILL.md"), "my own words");
  assert_eq!(install(&dir, true), Ok(Installed::ReplacedEdited));
  assert_eq!(text(&dir, "SKILL.md"), SKILL);
}

#[test]
fn a_copy_without_a_marker_or_with_a_missing_file_counts_as_edited() {
  let home = tempfile::tempdir().expect("a temporary directory");
  let dir = home.path();
  fs::write(dir.join("SKILL.md"), "written by hand").expect("written");
  let refusal = install(dir, false).expect_err("refused");
  assert!(refusal.message.contains(&format!("no {MARKER}")), "{}", refusal.message);

  install(dir, true).expect("installed");
  fs::remove_file(dir.join("references/design.md")).expect("removed");
  let refusal = install(dir, false).expect_err("refused");
  assert!(refusal.message.contains("references/design.md is missing"), "{}", refusal.message);
}

#[test]
fn a_marker_path_that_no_skill_file_can_have_counts_as_no_line() {
  let sha256 = "a".repeat(64);
  let paths = [
    "SKILL.md",
    "references/spec.md",
    "../escape.md",
    "/abs.md",
    "C:x.md",
    "references/../x.md",
    "references/.hidden.md",
    "references/a/b.md",
    "references/a\\b.md",
    "references/.md",
    "notes.md",
    "references/spec.txt",
  ];
  let marker: String = paths.iter().map(|path| format!("{sha256}  {path}\n")).chain(["not a line\n".to_owned()]).collect();
  assert_eq!(parse_marker(&marker).into_keys().collect::<Vec<_>>(), ["SKILL.md", "references/spec.md"]);
}

/// The refresh of `bd-host upgrade` replaces only an unedited copy of another build, and installs no missing one
#[test]
fn a_refresh_replaces_only_an_unedited_copy_of_another_build() {
  let home = tempfile::tempdir().expect("a temporary directory");
  let dir = home.path().join("bd-host");
  assert_eq!(refresh(&dir), None);
  assert!(!dir.exists(), "a refresh installs a missing skill");

  let old = install_other(&dir, &[("SKILL.md", "old")]);
  assert_eq!(
    refresh(&dir),
    Some(format!("bd-host: replaced the skill of build {old} with build {} in {}", build(), dir.display()))
  );
  assert_eq!(text(&dir, "SKILL.md"), SKILL);
  assert_eq!(refresh(&dir), None);

  fs::write(dir.join("SKILL.md"), "my own words").expect("written");
  let said = refresh(&dir).expect("a line");
  assert!(said.starts_with("bd-host: the skill in "), "{said}");
  assert!(said.contains("local edits (SKILL.md)"), "{said}");
  assert_eq!(text(&dir, "SKILL.md"), "my own words");
}

/// The frontmatter that an agent harness reads: the name of the directory and a description on one line within the
/// budget of 160 bytes
#[test]
fn the_skill_names_itself_bd_host_with_a_short_description() {
  let skill = SKILL.replace("\r\n", "\n");
  let frontmatter = skill
    .strip_prefix("---\n")
    .and_then(|rest| rest.split_once("\n---\n"))
    .map(|(frontmatter, _)| frontmatter)
    .expect("a frontmatter");
  let field = |name: &str| frontmatter.lines().find_map(|line| line.strip_prefix(name)?.strip_prefix(": ")).unwrap_or_default();
  assert_eq!(field("name"), "bd-host");
  let description = field("description");
  assert!(!description.is_empty() && description.len() <= 160, "{} bytes: {description}", description.len());
}
