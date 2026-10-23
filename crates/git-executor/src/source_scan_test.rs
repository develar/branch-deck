//! The production sources of the workspace as the scan tests read them (`worktree_safety_test.rs`,
//! `git_usage_scan_test.rs`): every `.rs` file of `src-tauri` and the crates, without test files, test crates and the
//! inline test modules of a file (only their blocks: the code after one is scanned), and the git argument lists in it.
//! An argument list is an array literal whose first element is a string literal (`&["log", "-z", &range]`,
//! `vec!["diff-tree", ...]`); its subcommand is the first element after git's global options and their values
//! (`process_stats::subcommand_index`), so `["-C", dir, "reset"]` is a `reset`. A list built with `push` or `extend` is
//! seen only as far as its literal goes.

use crate::process_stats::subcommand_index;
use std::fs;
use std::path::{Path, PathBuf};

/// Crates that only build fixture repositories for tests
const EXEMPT_CRATES: [&str; 1] = ["test-utils"];

/// A production source file
pub(crate) struct Source {
  /// The path from the workspace root, `/`-separated (`crates/git-ops/src/ref_transaction.rs`)
  pub(crate) path: String,
  /// The text without its inline test modules (`production_part`), on the lines of the file
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

/// A file a scan rule allows, with its number of hits and why
pub(crate) struct Exemption {
  pub(crate) path: &'static str,
  pub(crate) hits: usize,
  pub(crate) reason: &'static str,
}

/// Fails when `find` hits a line outside `exemptions`, or an exempt file has another number of hits than its exemption
pub(crate) fn check(rule: &str, instead: &str, exemptions: &[Exemption], find: impl Fn(&Source) -> Vec<usize>) {
  let mut report = Vec::new();
  let mut exempt_hits = vec![0; exemptions.len()];
  for source in production_sources() {
    let lines = find(&source);
    match exemptions.iter().position(|exemption| exemption.path == source.path) {
      Some(index) => exempt_hits[index] += lines.len(),
      None => report.extend(lines.iter().map(|line| format!("  {}:{line}", source.path))),
    }
  }
  for (exemption, hits) in exemptions.iter().zip(exempt_hits) {
    if hits != exemption.hits {
      report.push(format!(
        "  {}: {hits} hits, but its exemption says {} ({}); a new hit there needs a review, a gone one lowers the count",
        exemption.path, exemption.hits, exemption.reason
      ));
    }
  }
  assert!(report.is_empty(), "{rule}:\n{}\nInstead: {instead}", report.join("\n"));
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
        text: production_part(&text),
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

/// Source without its inline test modules (`#[cfg(test)] mod tests { … }`), each replaced by its line breaks, so the code
/// before and after it keeps its line numbers; a test module of a file of its own (`mod x_test;`) has nothing to leave
/// out
pub(crate) fn production_part(source: &str) -> String {
  const CFG_TEST: &str = "#[cfg(test)]";
  let mut text = String::with_capacity(source.len());
  let mut copied = 0;
  let mut offset = 0;
  while let Some(index) = source[offset..].find(CFG_TEST) {
    let start = offset + index;
    offset = start + CFG_TEST.len();
    let Some(length) = test_module_length(&source[offset..]) else { continue };
    offset += length;
    text.push_str(&source[copied..start]);
    text.extend(source[start..offset].matches('\n').map(|_| '\n'));
    copied = offset;
  }
  text.push_str(&source[copied..]);
  text
}

/// The length of the inline module that `rest`, the text after a `#[cfg(test)]`, starts, to its closing brace: more
/// attributes, a visibility, `mod <name> { … }`; None for another item and for a module declared without a body
fn test_module_length(rest: &str) -> Option<usize> {
  let mut item = skip_trivia(rest);
  while item.starts_with("#[") {
    item = skip_trivia(&item[block_end(item, 2)? + 1..]);
  }
  for visibility in ["pub(crate) ", "pub(super) ", "pub "] {
    item = item.strip_prefix(visibility).unwrap_or(item);
  }
  let body = item.strip_prefix("mod ")?;
  let open = body.find(['{', ';'])?;
  if body.as_bytes()[open] != b'{' || !body[..open].trim().chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
    return None;
  }
  // every slice above is a suffix of `rest`
  Some(rest.len() - body.len() + block_end(body, open + 1)? + 1)
}

/// The index of the bracket that closes the one before `start` in `text`, past nested brackets, strings, char literals
/// and comments; None when it does not close
fn block_end(text: &str, start: usize) -> Option<usize> {
  let bytes = text.as_bytes();
  let mut depth = 0usize;
  let mut index = start;
  while index < bytes.len() {
    if let Some(end) = literal_end(text, index) {
      index = end;
      continue;
    }
    match bytes[index] {
      b'(' | b'[' | b'{' => depth += 1,
      b')' | b']' | b'}' if depth == 0 => return Some(index),
      b')' | b']' | b'}' => depth -= 1,
      _ => {}
    }
    index += 1;
  }
  None
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
  /// The source after its `]`, to see what is added to it
  pub(crate) after: &'a str,
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
    let Some((elements, end)) = elements(text) else { continue };
    let args: Vec<Arg<'_>> = elements.into_iter().filter_map(arg).collect();
    if matches!(args.first(), Some(Arg::Literal(_))) {
      lists.push(ArgList {
        line: line_at(source, index),
        before: &source[..index],
        after: &text[end + 1..],
        args,
      });
    }
  }
  lists
}

/// The 1-based line of the byte at `offset` of `text`
pub(crate) fn line_at(text: &str, offset: usize) -> usize {
  text[..offset].bytes().filter(|&b| b == b'\n').count() + 1
}

/// `text` without its leading whitespace and comments
pub(crate) fn skip_trivia(mut text: &str) -> &str {
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

/// The elements of the array literal that starts at `text` (just after its `[`) and the index of its `]`; None when it
/// does not close
fn elements(text: &str) -> Option<(Vec<&str>, usize)> {
  let bytes = text.as_bytes();
  let mut elements = Vec::new();
  let mut depth = 0usize;
  let mut start = 0;
  let mut index = 0;
  while index < bytes.len() {
    if let Some(end) = literal_end(text, index) {
      index = end;
      continue;
    }
    match bytes[index] {
      b'(' | b'[' | b'{' => {
        depth += 1;
        index += 1;
      }
      b']' if depth == 0 => {
        elements.push(&text[start..index]);
        return Some((elements, index));
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

/// The index after the string, raw string, char literal or comment that starts at `index` of `text` (the end of `text`
/// when it does not close); None when none starts there
fn literal_end(text: &str, index: usize) -> Option<usize> {
  let bytes = text.as_bytes();
  Some(match bytes[index] {
    b'"' => string_end(bytes, index + 1).unwrap_or(bytes.len()),
    b'\'' => char_end(bytes, index),
    b'/' if bytes.get(index + 1) == Some(&b'/') => text[index..].find('\n').map_or(bytes.len(), |end| index + end),
    b'/' if bytes.get(index + 1) == Some(&b'*') => text[index + 2..].find("*/").map_or(bytes.len(), |end| index + 2 + end + 2),
    _ => return raw_string_end(text, index),
  })
}

/// The index after a raw string (`r"…"`, `r#"…"#`, `br"…"`) that starts at `index` of `text` (the end of `text` when it
/// does not close); None when none starts there
fn raw_string_end(text: &str, index: usize) -> Option<usize> {
  if index > 0 && (text.as_bytes()[index - 1].is_ascii_alphanumeric() || text.as_bytes()[index - 1] == b'_') {
    return None;
  }
  let token = &text[index..];
  let rest = token.strip_prefix('b').unwrap_or(token).strip_prefix('r')?;
  let hashes = rest.len() - rest.trim_start_matches('#').len();
  let content = rest[hashes..].strip_prefix('"')?;
  let close = format!("\"{}", "#".repeat(hashes));
  let content_start = text.len() - content.len();
  Some(content.find(&close).map_or(text.len(), |end| content_start + end + close.len()))
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

/// The index after a char literal (`'a'`, `'\n'`, `'\''`) or a lifetime (`'a`) that starts at `start`
fn char_end(bytes: &[u8], start: usize) -> usize {
  if bytes.get(start + 1) == Some(&b'\\') {
    // past the escaped character, which may be a quote
    let content = (start + 3).min(bytes.len());
    return bytes[content..].iter().position(|&b| b == b'\'').map_or(bytes.len(), |end| content + end + 1);
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
  assert_eq!(lists[0].after, ");\n}");
  // comments inside a list, a trailing comma, and an escaped quote
  let lists = argument_lists("vec![\n  // the subcommand\n  \"diff-tree\",\n  \"--format=\\\"%H\\\"\", // quoted\n  '[',\n]");
  assert_eq!(lists[0].args, [Arg::Literal("diff-tree"), Arg::Literal("--format=\\\"%H\\\""), Arg::Expr("'['")]);
  assert!(argument_lists("let x: [&'static str; 2] = y; let z = &s[..n];").is_empty());
}

#[test]
fn only_the_test_modules_are_left_out() {
  let source = concat!(
    "fn a() {}\n",
    "#[cfg(test)]\n",
    "#[path_free]\n",
    "mod tests {\n",
    "  const S: &str = \"}\";\n",
    "  const R: &str = r#\"{\"a\": \"}\"}\"#;\n",
    "  // }\n",
    "  fn t<'a>(x: &'a str) { let (c, q) = ('}', '\\''); }\n",
    "}\n",
    "fn b() { run(&[\"log\"]); }\n",
    "#[cfg(test)]\n",
    "mod b_test;\n",
    "fn c() { run(&[\"diff\"]); }\n",
  );
  let text = production_part(source);
  assert_eq!(text.lines().count(), source.lines().count());
  assert_eq!(
    text.lines().filter(|line| !line.is_empty()).collect::<Vec<_>>(),
    ["fn a() {}", "fn b() { run(&[\"log\"]); }", "#[cfg(test)]", "mod b_test;", "fn c() { run(&[\"diff\"]); }"]
  );
  let lines: Vec<(usize, Option<&str>)> = argument_lists(&text).iter().map(|list| (list.line, list.subcommand())).collect();
  assert_eq!(lines, [(10, Some("log")), (13, Some("diff"))]);
  // a test function stays, and so does a module whose block does not close
  let kept = "#[cfg(test)]\nfn helper() {}\n#[cfg(test)]\nmod open {\n";
  assert_eq!(production_part(kept), kept);
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
