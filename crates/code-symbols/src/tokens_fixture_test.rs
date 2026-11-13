//! The tokens of the fixtures of the crate and of its own sources (`code-tokens`): what must hold of the tokens of any
//! file (`code_tokens::checks`), of the many texts of every grammar the tests of the summaries, the highlighting and
//! the navigation read.

use pretty_assertions::assert_eq;

/// Every fixture of the crate and its own sources: the checks of `code_tokens::checks`
#[test]
fn every_fixture_formatted_again_has_the_same_tokens() {
  fn walk(path: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    if path.is_dir() {
      for entry in std::fs::read_dir(path).unwrap() {
        walk(&entry.unwrap().path(), out);
      }
    } else {
      out.push(path.to_path_buf());
    }
  }
  let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
  let mut files = Vec::new();
  for directory in ["testdata", "src", "examples"] {
    walk(&root.join(directory), &mut files);
  }
  files.sort();
  let (mut checked, mut lines, mut failed) = (0, 0, Vec::new());
  for path in files {
    let name = path.strip_prefix(&root).unwrap().to_string_lossy().into_owned();
    let Some(grammar) = code_tokens::grammar_of(&name) else { continue };
    let text = std::fs::read_to_string(&path).unwrap();
    match code_tokens::checks::check(grammar, &name, &text) {
      Ok(Some(found)) => {
        assert!(found.tokens > 0, "{name}");
        checked += 1;
        lines += found.lines;
      }
      Ok(None) => failed.push(format!("{name}: no tokens")),
      Err(what) => failed.push(format!("{name}: {what}")),
    }
  }
  assert_eq!(failed, Vec::<String>::new());
  assert!(checked >= 100 && lines >= 20_000, "{checked} files, {lines} lines");
}
