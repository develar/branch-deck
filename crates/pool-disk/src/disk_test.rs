use crate::build_dirs;

#[test]
fn the_build_dirs_of_a_slot_are_the_outermost_dirs_its_clean_rules_name() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(
    dir.path().join(".branchdeck.toml"),
    r#"[lease]
clean = [
  { cargo = "target", keep = 8, age = "1h" },
  { delete = "out", except = ["dev-run"] },
  { delete = "out/dev-run", except = ["jar-cache"] },
  { delete = "tests/results" },
]
"#,
  )
  .unwrap();
  assert_eq!(build_dirs(dir.path()), ["out", "target", "tests/results"]);
  assert_eq!(build_dirs(&dir.path().join("none")), Vec::<String>::new());
}
