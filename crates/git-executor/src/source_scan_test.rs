//! The production sources of the workspace as the scan tests read them (`worktree_safety_test.rs`,
//! `git_usage_scan_test.rs`): every `.rs` file of `src-tauri` and the crates, without test files, test crates and the
//! inline test module of a file, and the git argument lists in it. An argument list is an array literal whose first
//! element is a string literal (`&["log", "-z", &range]`, `vec!["diff-tree", ...]`); its subcommand is the first element
//! after git's global options and their values (`process_stats::subcommand_index`), so `["-C", dir, "reset"]` is a
//! `reset`. A list built with `push` or `extend` is seen only as far as its literal goes.

use crate::process_stats::subcommand_index;
use std::fs;
use std::path::{Path, PathBuf};

/// Crates that only build fixture repositories for tests
const EXEMPT_CRATES: [&str; 3] = ["test-utils", "sync-test-utils", "integration-detection-tests"];

/// A production source file
pub(crate) struct Source {
  /// The path from the workspace root, `/`-separated (`crates/git-ops/src/ref_transaction.rs`)
  pub(crate) path: String,
  /// The text without the inline test module (`production_part`)
  pub(crate) text: String,
}

impl Source {
  /// The lines that are not comments, with their 1-based numbers
  pub(crate) fn code_lines(&self) -> impl Iterator<Item = (usize, &str)> {
    self
      .text
      .lines()
      .enumerate()
      .map(|(index, line)| (index + 1, line))
      .filter(|(_, line)| !line.trim_start().starts_with("//"))
  }
}

/// Every production source of the workspace
pub(crate) fn production_sources() -> Vec<Source> {
  let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("workspace directory");
  let mut files = Vec::new();
  collect_rust_files(&workspace.join("src-tauri/src"), &mut files);
  for entry in fs::read_dir(workspace.join("crates")).expect("crates directory").flatten() {
    let name = entry.file_name();
    if !EXEMPT_CRATES.iter().any(|exempt| name == *exempt) {
      collect_rust_files(&entry.path().join("src"), &mut files);
    }
  }
  assert!(files.len() > 20, "the scan found too few sources, is the workspace layout different? {}", files.len());
  files
    .iter()
    .map(|file| {
      let text = fs::read_to_string(file).expect("readable source");
      let path = file.strip_prefix(&workspace).unwrap_or(file).to_string_lossy().replace('\\', "/");
      Source {
        path,
        text: production_part(&text).to_string(),
      }
    })
    .collect()
}

fn is_test_file(path: &Path) -> bool {
  let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
  name.ends_with("_test.rs") || name.ends_with("_tests.rs") || name == "tests.rs" || path.components().any(|c| c.as_os_str() == "tests")
}

fn collect_rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
  let Ok(entries) = fs::read_dir(dir) else { return };
  for entry in entries.flatten() {
    let path = entry.path();
    if path.is_dir() {
      collect_rust_files(&path, files);
    } else if path.extension().is_some_and(|e| e == "rs") && !is_test_file(&path) {
      files.push(path);
    }
  }
}

/// Source without inline `#[cfg(test)] mod …` blocks (by convention they are the last item of a file)
pub(crate) fn production_part(source: &str) -> &str {
  let mut offset = 0;
  while let Some(index) = source[offset..].find("#[cfg(test)]") {
    let start = offset + index;
    let rest = source[start + "#[cfg(test)]".len()..].trim_start();
    if rest.starts_with("mod ") {
      return &source[..start];
    }
    offset = start + 1;
  }
  source
}

/// An element of an argument list
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Arg<'a> {
  /// A string literal, its content as written (escapes unresolved)
  Literal(&'a str),
  /// Any other expression, as written (`&range`, `&format!("--onto={onto}")`)
  Expr(&'a str),
}

impl<'a> Arg<'a> {
  const fn text(self) -> &'a str {
    match self {
      Self::Literal(text) | Self::Expr(text) => text,
    }
  }
}

/// A git argument list in a source
#[derive(Debug)]
pub(crate) struct ArgList<'a> {
  /// The 1-based line of its `[`
  pub(crate) line: usize,
  /// The source up to its `[`, to see what it is passed to
  pub(crate) before: &'a str,
  pub(crate) args: Vec<Arg<'a>>,
}

impl<'a> ArgList<'a> {
  /// The subcommand when it is a literal (`["-c", "x=y", "merge-tree"]`); None when the list has none or it is an
  /// expression
  pub(crate) fn subcommand(&self) -> Option<&'a str> {
    let texts: Vec<&str> = self.args.iter().map(|arg| arg.text()).collect();
    match self.args[subcommand_index(&texts)?] {
      Arg::Literal(subcommand) => Some(subcommand),
      Arg::Expr(_) => None,
    }
  }

  /// The literals, in order
  pub(crate) fn literals(&self) -> impl Iterator<Item = &'a str> + '_ {
    self.args.iter().filter_map(|arg| match arg {
      Arg::Literal(literal) => Some(*literal),
      Arg::Expr(_) => None,
    })
  }

  /// Whether one of its literals satisfies `test`
  pub(crate) fn has(&self, test: impl Fn(&str) -> bool) -> bool {
    self.literals().any(test)
  }
}

/// The argument lists of `source`, in order
pub(crate) fn argument_lists(source: &str) -> Vec<ArgList<'_>> {
  let mut lists = Vec::new();
  for (index, _) in source.match_indices('[') {
    let text = &source[index + 1..];
    if !skip_trivia(text).starts_with('"') {
      continue;
    }
    let Some(elements) = elements(text) else { continue };
    let args: Vec<Arg<'_>> = elements.into_iter().filter_map(arg).collect();
    if matches!(args.first(), Some(Arg::Literal(_))) {
      lists.push(ArgList {
        line: source[..index].bytes().filter(|&b| b == b'\n').count() + 1,
        before: &source[..index],
        args,
      });
    }
  }
  lists
}

/// `text` without its leading whitespace and comments
fn skip_trivia(mut text: &str) -> &str {
  loop {
    let trimmed = text.trim_start();
    if let Some(comment) = trimmed.strip_prefix("//") {
      text = comment.find('\n').map_or("", |end| &comment[end..]);
    } else if let Some(comment) = trimmed.strip_prefix("/*") {
      text = comment.find("*/").map_or("", |end| &comment[end + 2..]);
    } else {
      return trimmed;
    }
  }
}

/// The element of an array literal as an argument; None for an empty one (after a trailing comma)
fn arg(element: &str) -> Option<Arg<'_>> {
  let element = skip_trivia(element);
  if element.is_empty() {
    return None;
  }
  if let Some(content) = element.strip_prefix('"')
    && let Some(end) = string_end(content.as_bytes(), 0)
    && skip_trivia(&content[end..]).is_empty()
  {
    return Some(Arg::Literal(&content[..end - 1]));
  }
  Some(Arg::Expr(element.trim_end()))
}

/// The elements of the array literal that starts at `text` (just after its `[`); None when it does not close
fn elements(text: &str) -> Option<Vec<&str>> {
  let bytes = text.as_bytes();
  let mut elements = Vec::new();
  let mut depth = 0usize;
  let mut start = 0;
  let mut index = 0;
  while index < bytes.len() {
    match bytes[index] {
      b'"' => index = string_end(bytes, index + 1)?,
      b'\'' => index = char_end(bytes, index),
      b'/' if bytes.get(index + 1) == Some(&b'/') => index = text[index..].find('\n').map_or(bytes.len(), |end| index + end),
      b'/' if bytes.get(index + 1) == Some(&b'*') => index = index + 2 + text[index + 2..].find("*/")? + 2,
      b'(' | b'[' | b'{' => {
        depth += 1;
        index += 1;
      }
      b']' if depth == 0 => {
        elements.push(&text[start..index]);
        return Some(elements);
      }
      b')' | b']' | b'}' => {
        depth = depth.checked_sub(1)?;
        index += 1;
      }
      b',' if depth == 0 => {
        elements.push(&text[start..index]);
        index += 1;
        start = index;
      }
      _ => index += 1,
    }
  }
  None
}

/// The index after the closing quote of a string whose content starts at `start`
fn string_end(bytes: &[u8], start: usize) -> Option<usize> {
  let mut index = start;
  while index < bytes.len() {
    match bytes[index] {
      b'\\' => index += 2,
      b'"' => return Some(index + 1),
      _ => index += 1,
    }
  }
  None
}

/// The index after a char literal (`'a'`, `'\n'`) or a lifetime (`'a`) that starts at `start`
fn char_end(bytes: &[u8], start: usize) -> usize {
  if bytes.get(start + 1) == Some(&b'\\') {
    return bytes[start + 2..].iter().position(|&b| b == b'\'').map_or(bytes.len(), |end| start + 2 + end + 1);
  }
  if bytes.get(start + 2) == Some(&b'\'') { start + 3 } else { start + 1 }
}

#[test]
fn an_argument_list_is_an_array_literal_that_starts_with_a_string() {
  let lists = argument_lists("#[derive(Debug)]\nfn f() {\n  run(&[\"log\", \"-z\", &format!(\"{a}, {b}]\"), x[0]]);\n}");
  assert_eq!(lists.len(), 1);
  assert_eq!(lists[0].line, 3);
  assert_eq!(
    lists[0].args,
    [Arg::Literal("log"), Arg::Literal("-z"), Arg::Expr("&format!(\"{a}, {b}]\")"), Arg::Expr("x[0]")]
  );
  assert_eq!(lists[0].before, "#[derive(Debug)]\nfn f() {\n  run(&");
  // comments inside a list, a trailing comma, and an escaped quote
  let lists = argument_lists("vec![\n  // the subcommand\n  \"diff-tree\",\n  \"--format=\\\"%H\\\"\", // quoted\n  '[',\n]");
  assert_eq!(lists[0].args, [Arg::Literal("diff-tree"), Arg::Literal("--format=\\\"%H\\\""), Arg::Expr("'['")]);
  assert!(argument_lists("let x: [&'static str; 2] = y; let z = &s[..n];").is_empty());
}

#[test]
fn the_subcommand_follows_the_global_options_and_their_values() {
  fn subcommand(source: &str) -> Option<&str> {
    argument_lists(source)[0].subcommand()
  }
  assert_eq!(subcommand(r#"&["-c", "x=y", "rebase", "--root"]"#), Some("rebase"));
  assert_eq!(subcommand(r#"&["--no-pager", "reset"]"#), Some("reset"));
  assert_eq!(subcommand(r#"&["-C", dir, "reset"]"#), Some("reset"));
  assert_eq!(
    subcommand(r#"&["--git-dir", git_dir, "--literal-pathspecs", "--work-tree=.", "checkout"]"#),
    Some("checkout")
  );
  assert_eq!(subcommand(r#"&["-c", config, subcommand, "reset"]"#), None);
  assert_eq!(subcommand(r#"&["--raw", "-z"]"#), None);
}
