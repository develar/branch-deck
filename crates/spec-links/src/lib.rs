//! The check of the links of the specs and the design docs to the code (`pnpm lint:specs`, docs/contributing/specs.md):
//! - every `[@test]` link is a line of its own, `[@test] <path> (`name`, `name`)`, its path names a test file and every
//!   name a test in it: a TypeScript test by its title (a string literal), a Rust test by its function. A path that
//!   starts with `./` or `../` is from the file, any other from the root (`spec_syntax::resolve`); under `openspec/`
//!   every path is from the root, as `openspec archive` copies the text of a delta into the main spec two directories up;
//! - every spec (`openspec/specs/<area>/<name>/spec.md`) lists its `targets:` in its front matter, each once, a path
//!   from the root of a file that exists;
//! - every rule of the domain (`crates/domain-policy/src/<rule>.rs`) names its spec in its `//!` header, and every spec
//!   it names lists it as a target;
//! - every presenter of the page (`src/presenters/*.ts`) is a target of a spec;
//! - a link into a spec names one of its headings by the anchor GitHub gives it, or a group of its requirements
//!   (`#refusals` for the requirements `### Requirement: Refusals: …`);
//! - in every spec the requirements of a group (`### Requirement: <Group>: <behaviour>`) stand together;
//! - no file lies under `spec/`, and none names a spec by its old path `spec/<area>/<name>.spec.md`;
//! - `openspec validate --strict` passes for every spec and every change;
//! - AGENTS.md keeps within its budget, which every session of every agent reads whole (Codex reads no more than 32 KiB).
//!
//! A renamed, moved or deleted test or file fails here, not in a reader. Every path is one from the root with `/` on
//! every platform, as the links, the targets and the `//!` headers name a file. A test link and a target may name a
//! file of a crate of the git dependency psi-syntax, `psi-syntax:crates/<crate>/<path>`: the file of that crate at the
//! revision Cargo.lock pins, in the checkout cargo has fetched (`GitSources`).

use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::lease_slot::tracked_under;
use regex::Regex;
use serde_json::Value;
use std::cell::OnceCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use sync_utils::child_process::windowless_command;

/// The rules of the domain, each a `//!` header naming its spec
const RULES: &str = "crates/domain-policy/src/*.rs";
/// The presenters of the page, each a target of a spec
const PRESENTERS: &str = "src/presenters/*.ts";
/// The budget of AGENTS.md, in bytes
pub const AGENTS_BUDGET: u64 = 32 * 1024;
/// The specs
const SPECS: &str = "openspec/specs/**/spec.md";
/// The files of prose whose `[@test]` lines and links are checked; an archived change is history, as CHANGELOG.md
const DOCS: &[&str] = &["openspec/**/*.md", "docs/**/*.md"];
/// The archived changes of OpenSpec
const ARCHIVE: &str = "openspec/changes/archive/";

/// A `[@test]` link, a line of its own: its path and its names
static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*\[@test\] (\S+) \(((?:`[^`]+`(?:, )?)+)\)$").expect("a valid pattern"));
/// A name of a link, in backticks
static NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new("`([^`]+)`").expect("a valid pattern"));
/// A spec a `//!` header names
static SPEC_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bopenspec/specs/(?:[a-z0-9-]+/)+spec\.md\b").expect("a valid pattern"));
/// A markdown link to a spec of OpenSpec, `[text](<path>/spec.md#anchor)`
static SPEC_LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\]\(([^)\s#]*/spec\.md)(?:#([^)\s]*))?\)").expect("a valid pattern"));
/// The heading of a requirement, its name the group
static REQUIREMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new("^### Requirement: (.+)$").expect("a valid pattern"));
/// A heading of Markdown and its text
static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new("^#{1,6} (.+)$").expect("a valid pattern"));
/// The closing run of `#` of a heading
static CLOSING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+#+\s*$").expect("a valid pattern"));
/// A target line of a front matter
static TARGET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s+- (\S+)$").expect("a valid pattern"));
/// A path of a spec of the old layout; `old_layout_at` keeps the ones from the root or after `../`
/// (`plugins/air/spec/x/y.spec.md` is of another repository)
static OLD_LAYOUT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"spec/(?:branch|cli|commit|repository|review)/[a-z0-9-]+\.spec\.md\b").expect("a valid pattern"));

/// The prefix of a path into the git dependency psi-syntax, `psi-syntax:crates/<crate>/<path>`
const PSI_SYNTAX: &str = "psi-syntax:";
/// The repository of the git dependency psi-syntax, its public mirror, as Cargo.lock names its source
const PSI_SYNTAX_URL: &str = "https://github.com/develar/psi-syntax";

/// The directory of each crate of psi-syntax by its revision (the commit of Cargo.lock), by the name of its directory
type Crates = BTreeMap<String, BTreeMap<String, PathBuf>>;

/// The sources of the crates of the git dependency psi-syntax, for a path `psi-syntax:crates/<crate>/<path>`: the
/// directory of each crate the workspace depends on, at the revision Cargo.lock pins, from the packages `metadata`
/// answers (`cargo_metadata`). It is asked once, at the first such path; a check of none runs no cargo
pub struct GitSources<'a> {
  metadata: Box<dyn Fn() -> Result<Value, String> + 'a>,
  /// The crates of psi-syntax, or why there are none
  crates: OnceCell<Result<Crates, String>>,
}

impl<'a> GitSources<'a> {
  /// The sources of the packages `metadata` answers, `cargo metadata --format-version 1` of the workspace or why not
  #[must_use]
  pub fn new(metadata: impl Fn() -> Result<Value, String> + 'a) -> Self {
    Self {
      metadata: Box::new(metadata),
      crates: OnceCell::new(),
    }
  }

  /// The file at `path` of `root`: of a path `psi-syntax:crates/<crate>/<path>` in the directory of the crate, of any
  /// other from `root`
  ///
  /// # Errors
  /// The path into psi-syntax names no file of a crate, its sources are not fetched, the workspace depends on no crate of
  /// that name, or on two revisions of it.
  pub fn file_of(&self, root: &Path, path: &str) -> Result<PathBuf, String> {
    let Some(inside) = path.strip_prefix(PSI_SYNTAX) else {
      return Ok(root.join(path));
    };
    let parts: Vec<&str> = inside.split('/').collect();
    if parts.len() < 3 || parts[0] != "crates" || parts.iter().any(|part| matches!(*part, "" | "." | "..")) {
      return Err(format!("{path} is no file of a crate of psi-syntax, {PSI_SYNTAX}crates/<crate>/<path>"));
    }
    let name = parts[1];
    let crates = self.crates.get_or_init(|| (self.metadata)().map(|metadata| psi_syntax_crates(&metadata)));
    let crates = crates
      .as_ref()
      .map_err(|error| format!("the sources of psi-syntax are not fetched (`cargo fetch` fetches them): {error}"))?;
    let Some(revisions) = crates.get(name) else {
      let names: Vec<&str> = crates.keys().map(String::as_str).collect();
      let only = if names.is_empty() {
        "on none".to_string()
      } else {
        format!("only on {}", names.join(", "))
      };
      return Err(format!("the workspace depends on no crate {name} of psi-syntax (Cargo.lock), {only}"));
    };
    match revisions.values().collect::<Vec<_>>()[..] {
      [dir] => Ok(parts[2..].iter().fold(dir.clone(), |file, part| file.join(part))),
      _ => Err(format!(
        "the workspace depends on the crate {name} of psi-syntax at {} revisions ({}, Cargo.lock), so the link names no one file: pin one",
        revisions.len(),
        revisions.keys().map(|commit| commit.get(..8).unwrap_or(commit)).collect::<Vec<_>>().join(", ")
      )),
    }
  }

  /// The file at `path` of `root` that exists (`file_of`)
  ///
  /// # Errors
  /// The problem of `file_of`, or no file at the path.
  pub fn existing_file(&self, root: &Path, path: &str) -> Result<PathBuf, String> {
    let file = self.file_of(root, path)?;
    if file.exists() { Ok(file) } else { Err(format!("no file {path}")) }
  }
}

/// The directory of each crate of psi-syntax among the packages of `metadata` (`cargo metadata`), by its revision, by
/// the name of its directory under `crates/` of the checkout
fn psi_syntax_crates(metadata: &Value) -> Crates {
  let mut crates = Crates::new();
  for package in metadata["packages"].as_array().into_iter().flatten() {
    // `git+<url>?rev=<rev>#<commit>`
    let Some(source) = package["source"].as_str().and_then(|source| source.strip_prefix("git+")) else {
      continue;
    };
    let (url, commit) = source.split_once('#').unwrap_or((source, ""));
    let url = url.split_once('?').map_or(url, |(url, _)| url);
    if url.trim_end_matches(".git") != PSI_SYNTAX_URL {
      continue;
    }
    let Some(dir) = package["manifest_path"].as_str().and_then(|manifest| Path::new(manifest).parent()) else {
      continue;
    };
    if dir.parent().and_then(Path::file_name).is_some_and(|parent| parent == "crates")
      && let Some(name) = dir.file_name()
    {
      crates.entry(name.to_string_lossy().into_owned()).or_default().insert(commit.to_string(), dir.to_path_buf());
    }
  }
  crates
}

/// What `cargo metadata --format-version 1 --offline --locked` answers of the workspace at `root`: its packages at the
/// revisions Cargo.lock pins, from the sources cargo has fetched, never from the network and never a change of
/// Cargo.lock. The cargo that runs this one (`CARGO`), else the one that built it
///
/// # Errors
/// Cargo does not run, has not fetched a package of the workspace or Cargo.lock is not up to date; cargo's error, its
/// lines joined from the first error.
pub fn cargo_metadata(root: &Path) -> Result<Value, String> {
  let cargo = std::env::var_os("CARGO").unwrap_or_else(|| env!("CARGO").into());
  let output = windowless_command(&cargo)
    .args(["metadata", "--format-version", "1", "--offline", "--locked", "--manifest-path"])
    .arg(root.join("Cargo.toml"))
    .output()
    .map_err(|error| format!("cargo metadata --offline: {error}"))?;
  if !output.status.success() {
    // from its first error, without the notes before it ("Blocking waiting for file lock")
    let stderr = String::from_utf8_lossy(&output.stderr);
    let lines: Vec<&str> = stderr.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
    let first = lines.iter().position(|line| line.starts_with("error")).unwrap_or(0);
    return Err(format!("cargo metadata --offline: {}", lines[first..].join(" ")));
  }
  serde_json::from_slice(&output.stdout).map_err(|error| format!("cargo metadata --offline: {error}"))
}

/// Every problem of the specs of the repository at `root`, in the order of the checks
#[must_use]
pub fn check_all(root: &Path) -> Vec<String> {
  let sources = GitSources::new(|| cargo_metadata(root));
  let mut problems = check_test_links(root, &sources);
  problems.extend(check_spec_targets(root, &sources));
  problems.extend(check_spec_anchors(root));
  problems.extend(check_requirement_groups(root));
  problems.extend(check_old_layout(root, &tracked_files(root)));
  problems.extend(check_open_spec(root, openspec_validate));
  problems.extend(check_agents_budget(root));
  problems
}

/// The files under `root` that `patterns` match, by their paths from `root` with `/`, sorted: a walk of the directory
/// before each pattern's first wildcard, without the hidden directories, `node_modules` and `target`
fn files_of(root: &Path, patterns: &[&str]) -> Vec<String> {
  let mut files = BTreeSet::new();
  for pattern in patterns {
    let fixed = pattern.find(['*', '?']).map_or(*pattern, |at| &pattern[..at]);
    let start = fixed.rfind('/').map_or("", |at| &fixed[..at]);
    if !pattern.contains(['*', '?']) {
      if root.join(pattern).is_file() {
        files.insert((*pattern).to_string());
      }
      continue;
    }
    walk(root, start, &mut |path| {
      if spec_syntax::glob_matches(pattern, path) {
        files.insert(path.to_string());
      }
    });
  }
  files.into_iter().collect()
}

/// Each file below the directory `dir` of `root` (a path from it, `""` for the root), by its path from `root`
fn walk(root: &Path, dir: &str, visit: &mut dyn FnMut(&str)) {
  let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
    return;
  };
  let mut entries: Vec<_> = entries.flatten().collect();
  entries.sort_by_key(std::fs::DirEntry::file_name);
  for entry in entries {
    let name = entry.file_name().to_string_lossy().into_owned();
    let path = if dir.is_empty() { name.clone() } else { format!("{dir}/{name}") };
    let Ok(kind) = entry.file_type() else {
      continue;
    };
    if kind.is_dir() {
      if !name.starts_with('.') && name != "node_modules" && name != "target" {
        walk(root, &path, visit);
      }
    } else if !kind.is_dir() {
      visit(&path);
    }
  }
}

/// The files of prose under `root` whose links are checked, without the archived changes
fn docs_of(root: &Path, patterns: &[&str]) -> Vec<String> {
  files_of(root, patterns).into_iter().filter(|doc| !doc.starts_with(ARCHIVE)).collect()
}

/// The text of the file at `path` of `root`, lossy where it is no UTF-8; `None` for no file
fn read(root: &Path, path: &str) -> Option<String> {
  read_file(&root.join(path))
}

/// The text of the file `file`, lossy where it is no UTF-8; `None` for no file
fn read_file(file: &Path) -> Option<String> {
  std::fs::read(file).ok().map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

/// The lines of `text`, split at `\n` with a `\r` before it dropped
fn lines_of(text: &str) -> Vec<&str> {
  text.split('\n').map(|line| line.strip_suffix('\r').unwrap_or(line)).collect()
}

/// Whether the path is from the file it is written in: it starts `./` or `../`
fn from_the_file(path: &str) -> bool {
  path.starts_with("./") || path.starts_with("../")
}

/// The file a link of `doc` names by `path`, from the root with `/`: `./` and `../` from `doc`, any other from the root
/// (`spec_syntax::resolve`, the rule of OpenSpec and of any file that is no IntelliJ spec); a path above the root as
/// written
fn linked_file(doc: &str, path: &str) -> String {
  spec_syntax::resolve(doc, spec_syntax::Format::OpenSpec, path).unwrap_or_else(|| path.to_string())
}

/// Whether the file declares the test: a Rust function (`\bfn <name>\s*[(<]`), else a title in any string quote,
/// escaped as the source writes it
fn declares(path: &str, source: &str, name: &str) -> bool {
  if Path::new(path).extension().is_some_and(|extension| extension == "rs") {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let needle = format!("fn {name}");
    return source.match_indices(&needle).any(|(at, _)| {
      let before = source[..at].chars().next_back();
      let after = source[at + needle.len()..].trim_start();
      !before.is_some_and(word) && (after.starts_with('(') || after.starts_with('<'))
    });
  }
  ['"', '\'', '`']
    .iter()
    .any(|quote| source.contains(&format!("{quote}{}{quote}", name.replace(*quote, &format!("\\{quote}")))))
}

/// The problems of the `[@test]` links of the specs and the design docs under `root`; a link into psi-syntax names a
/// test file of `sources`
#[must_use]
pub fn check_test_links(root: &Path, sources: &GitSources) -> Vec<String> {
  let mut texts: HashMap<String, Option<String>> = HashMap::new();
  let mut problems = Vec::new();
  for doc in docs_of(root, DOCS) {
    let Some(text) = read(root, &doc) else {
      continue;
    };
    for (index, line) in lines_of(&text).into_iter().enumerate() {
      if !line.contains("[@test]") {
        continue;
      }
      let at = format!("{doc}:{}", index + 1);
      let Some(link) = LINK.captures(line) else {
        problems.push(format!(
          "{at}: not a link of its own line, `[@test] <path> (`name`, …)`: {}",
          line.trim().chars().take(120).collect::<String>()
        ));
        continue;
      };
      let path = &link[1];
      if doc.starts_with("openspec/") && from_the_file(path) {
        problems.push(format!("{at}: the path {path} is not from the root of the repository, as every path under openspec/"));
        continue;
      }
      let file = if path.starts_with(PSI_SYNTAX) { path.to_string() } else { linked_file(&doc, path) };
      let Ok(full) = sources.existing_file(root, &file).inspect_err(|problem| problems.push(format!("{at}: {problem}"))) else {
        continue;
      };
      let source = texts.entry(file.clone()).or_insert_with(|| read_file(&full)).clone().unwrap_or_default();
      for name in NAME.captures_iter(&link[2]) {
        if !declares(&file, &source, &name[1]) {
          problems.push(format!("{at}: {file} has no test `{}`", &name[1]));
        }
      }
    }
  }
  problems
}

/// A `targets:` item of a spec: its path as written and its line
struct Target {
  path: String,
  line: usize,
}

/// The `targets:` of a spec's front matter, or the problem of a spec that has none; the problems of its lines beside
fn targets_of(spec: &str, text: &str) -> Result<(Vec<Target>, Vec<String>), String> {
  let lines = lines_of(text);
  let end = (lines.first() == Some(&"---"))
    .then(|| lines.iter().skip(1).position(|line| *line == "---").map(|at| at + 1))
    .flatten();
  let Some(end) = end else {
    return Err(format!("{spec}: no front matter (`---` … `---`)"));
  };
  let Some(start) = lines[..end].iter().position(|line| *line == "targets:") else {
    return Err(format!("{spec}: its front matter lists no `targets:`"));
  };
  let (mut targets, mut problems) = (Vec::new(), Vec::new());
  for (index, line) in lines.iter().enumerate().take(end).skip(start + 1) {
    if !line.starts_with(char::is_whitespace) {
      break;
    }
    match TARGET.captures(line) {
      Some(item) => targets.push(Target {
        path: item[1].to_string(),
        line: index + 1,
      }),
      None => problems.push(format!("{spec}:{}: not a target line, `  - <path>`: {}", index + 1, line.trim())),
    }
  }
  if targets.is_empty() && problems.is_empty() {
    problems.push(format!("{spec}: its `targets:` list no file"));
  }
  Ok((targets, problems))
}

/// The leading `//!` lines of a Rust file
fn module_header(source: &str) -> String {
  lines_of(source).into_iter().take_while(|line| line.starts_with("//!")).collect::<Vec<_>>().join("\n")
}

/// The path `path` normalized as `posix.normalize` does: `.` and `x/..` gone, a leading `..` kept
fn normalize(path: &str) -> String {
  let mut parts: Vec<&str> = Vec::new();
  for part in path.split('/') {
    match part {
      "" | "." => {}
      ".." if parts.last().is_some_and(|last| *last != "..") => {
        parts.pop();
      }
      part => parts.push(part),
    }
  }
  parts.join("/")
}

/// The problems of the `targets:` of the specs under `root`: a spec without them, a target that is no path from the
/// root of a file, one listed twice; a rule of `crates/domain-policy/src/` whose `//!` header names no spec, or a spec
/// that lists it not; a presenter of `src/presenters/` that no spec lists. A target into psi-syntax names a file of
/// `sources`
#[must_use]
pub fn check_spec_targets(root: &Path, sources: &GitSources) -> Vec<String> {
  let mut problems = Vec::new();
  // the specs that list a file, by its path from `root`
  let mut listed_by: HashMap<String, BTreeSet<String>> = HashMap::new();
  for spec in files_of(root, &[SPECS]) {
    let (targets, found) = match targets_of(&spec, &read(root, &spec).unwrap_or_default()) {
      Ok(found) => found,
      Err(problem) => {
        problems.push(problem);
        continue;
      }
    };
    problems.extend(found);
    let mut seen = BTreeSet::new();
    for Target { path, line } in targets {
      let at = format!("{spec}:{line}");
      if from_the_file(&path) {
        problems.push(format!("{at}: the target {path} is not from the root of the repository, as every path under openspec/"));
        continue;
      }
      let file = normalize(&path);
      if !seen.insert(file.clone()) {
        problems.push(format!("{at}: lists {path} twice"));
        continue;
      }
      let Ok(full) = sources.existing_file(root, &file).inspect_err(|problem| problems.push(format!("{at}: {problem}"))) else {
        continue;
      };
      if !full.is_file() {
        problems.push(format!("{at}: {file} is no file"));
        continue;
      }
      listed_by.entry(file).or_default().insert(spec.clone());
    }
  }
  for rule in files_of(root, &[RULES]) {
    if rule.ends_with("/lib.rs") || rule.ends_with("_test.rs") {
      continue;
    }
    let header = module_header(&read(root, &rule).unwrap_or_default());
    // in the order the header names them, each once
    let mut named: Vec<&str> = Vec::new();
    for found in SPEC_NAME.find_iter(&header) {
      if !named.contains(&found.as_str()) {
        named.push(found.as_str());
      }
    }
    if named.is_empty() {
      problems.push(format!("{rule}: its `//!` header names no spec (openspec/specs/<area>/<name>/spec.md) of the rule"));
    }
    for spec in named {
      if !root.join(spec).exists() {
        problems.push(format!("{rule}: its `//!` header names {spec}, which does not exist"));
      } else if !listed_by.get(&rule).is_some_and(|specs| specs.contains(spec)) {
        problems.push(format!("{spec}: no target {rule}, whose `//!` header names the spec"));
      }
    }
  }
  for presenter in files_of(root, &[PRESENTERS]) {
    if !presenter.ends_with(".test.ts") && !listed_by.contains_key(&presenter) {
      problems.push(format!("{presenter}: a presenter that the `targets:` of no spec list"));
    }
  }
  problems
}

/// The anchor GitHub gives a heading (github-slugger): lowercase, every character but a letter, a mark, a number,
/// connector punctuation, `-` and a space dropped, every space a `-`
#[must_use]
pub fn heading_slug(heading: &str) -> String {
  use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory};
  heading
    .trim()
    .to_lowercase()
    .chars()
    .filter(|c| {
      *c == '-'
        || *c == ' '
        || matches!(
          c.general_category_group(),
          GeneralCategoryGroup::Letter | GeneralCategoryGroup::Mark | GeneralCategoryGroup::Number
        )
        || c.general_category() == unicode_properties::GeneralCategory::ConnectorPunctuation
    })
    .map(|c| if c == ' ' { '-' } else { c })
    .collect()
}

/// The lines of `text` outside its fenced code, each with its number; a fence line is none
fn prose_lines(text: &str) -> Vec<(&str, usize)> {
  let mut lines = Vec::new();
  let mut fence: Option<&str> = None;
  for (index, line) in lines_of(text).into_iter().enumerate() {
    let trimmed = line.trim_start();
    let run = |marker: char| trimmed.chars().take_while(|c| *c == marker).count();
    let marker = ['`', '~']
      .into_iter()
      .map(|marker| (marker, run(marker)))
      .find(|(_, count)| *count >= 3)
      .map(|(marker, count)| &trimmed[..count * marker.len_utf8()]);
    match (marker, fence) {
      (None, None) => lines.push((line, index + 1)),
      (None, Some(_)) => {}
      (Some(marker), None) => fence = Some(marker),
      (Some(marker), Some(open)) => {
        if marker.starts_with(&open[..1]) && marker.len() >= open.len() {
          fence = None;
        }
      }
    }
  }
  lines
}

/// The anchors of the headings of `text` outside its fenced code, a repeated one with the `-1`, `-2` GitHub gives it,
/// in the order of the headings
#[must_use]
pub fn heading_anchors(text: &str) -> Vec<String> {
  let mut anchors: Vec<String> = Vec::new();
  let mut counts: HashMap<String, usize> = HashMap::new();
  for (line, _) in prose_lines(text) {
    let Some(heading) = HEADING.captures(line) else {
      continue;
    };
    let slug = heading_slug(&CLOSING.replace(&heading[1], ""));
    let count = counts.entry(slug.clone()).or_insert(0);
    let anchor = if *count == 0 { slug } else { format!("{slug}-{count}") };
    *count += 1;
    if !anchors.contains(&anchor) {
      anchors.push(anchor);
    }
  }
  anchors
}

/// Whether `anchor` names one of `anchors` or a group of requirements: `refusals` for `requirement-refusals-…`
#[must_use]
pub fn names_heading(anchors: &[String], anchor: &str) -> bool {
  let group = format!("requirement-{anchor}-");
  anchors.iter().any(|slug| slug == anchor || slug.starts_with(&group))
}

/// The problems of the links of the files of prose under `root` (the specs, `docs/`, AGENTS.md, the READMEs) into a spec:
/// a file that is gone, an anchor that names none of its headings and no group of its requirements
#[must_use]
pub fn check_spec_anchors(root: &Path) -> Vec<String> {
  let mut anchors_of: HashMap<String, Vec<String>> = HashMap::new();
  let mut problems = Vec::new();
  let patterns: Vec<&str> = DOCS
    .iter()
    .copied()
    .chain(["AGENTS.md", "README.md", "crates/*/README.md", "crates/*/cases/README.md"])
    .collect();
  for doc in docs_of(root, &patterns) {
    let Some(text) = read(root, &doc) else {
      continue;
    };
    for (index, line) in lines_of(&text).into_iter().enumerate() {
      for link in SPEC_LINK.captures_iter(line) {
        let path = &link[1];
        let file = match path.strip_prefix('/') {
          Some(from_root) => from_root.to_string(),
          None => linked_file(&doc, &if from_the_file(path) { path.to_string() } else { format!("./{path}") }),
        };
        if !file.starts_with("openspec/specs/") {
          continue;
        }
        let at = format!("{doc}:{}", index + 1);
        if !root.join(&file).exists() {
          problems.push(format!("{at}: no file {file}"));
          continue;
        }
        let anchors = anchors_of.entry(file.clone()).or_insert_with(|| heading_anchors(&read(root, &file).unwrap_or_default()));
        if let Some(anchor) = link.get(2).map(|anchor| anchor.as_str())
          && !names_heading(anchors, anchor)
        {
          problems.push(format!("{at}: {file} has no heading of the anchor #{anchor}"));
        }
      }
    }
  }
  problems
}

/// The problems of the order of the requirements of the specs under `root`: the requirements of a group stand together.
/// The group of `### Requirement: <Group>: <behaviour>` is the text before its first `: `, of a name without one the
/// untitled group; a group of two runs fails at the first requirement of its second
#[must_use]
pub fn check_requirement_groups(root: &Path) -> Vec<String> {
  let mut problems = Vec::new();
  for spec in files_of(root, &[SPECS]) {
    let text = read(root, &spec).unwrap_or_default();
    let mut ended: BTreeSet<&str> = BTreeSet::new();
    let mut current: Option<&str> = None;
    for (line, number) in prose_lines(&text) {
      let Some(name) = REQUIREMENT.captures(line).and_then(|requirement| requirement.get(1)) else {
        continue;
      };
      let name = name.as_str();
      let group = name.find(": ").map_or("", |at| &name[..at]);
      if current == Some(group) {
        continue;
      }
      if ended.contains(group) {
        problems.push(format!("{spec}:{number}: the requirements of the group \"{group}\" are not together"));
      }
      if let Some(current) = current {
        ended.insert(current);
      }
      current = Some(group);
    }
  }
  problems
}

/// The tracked files of the repository at `root`, by their paths from it: one `ls-files -z` of `.` through the executor
/// (`git_ops::lease_slot::tracked_under`); none where git does not run there
fn tracked_files(root: &Path) -> Vec<String> {
  let Some(root) = root.to_str() else {
    return Vec::new();
  };
  tracked_under(&GitCommandExecutor::new(), root, &["."]).unwrap_or_default()
}

/// The first path of a spec of the old layout in `text` and the byte it starts at: one from the root or after `../`,
/// never after a word character, a `-` or such a character and a `/`
fn old_layout_at(text: &str) -> Option<(usize, &str)> {
  let word = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-';
  let bytes = text.as_bytes();
  OLD_LAYOUT.find_iter(text).find_map(|found| {
    let at = found.start();
    let after_word = at >= 1 && word(bytes[at - 1]);
    let after_word_slash = at >= 2 && bytes[at - 1] == b'/' && word(bytes[at - 2]);
    (!after_word && !after_word_slash).then(|| (at, found.as_str()))
  })
}

/// Whether the path is of a binary file the old layout check reads not
fn is_binary(path: &str) -> bool {
  let lower = path.to_ascii_lowercase();
  ["png", "jpg", "jpeg", "gif", "webp", "ico", "icns", "woff", "woff2", "ttf", "otf", "zip", "gz", "pdf", "car"]
    .iter()
    .any(|extension| lower.ends_with(&format!(".{extension}")))
}

/// The problems of the old layout of the specs, which moved to `openspec/specs/<area>/<name>/spec.md`: a file under
/// `spec/`, a file that names a spec by its old path (CHANGELOG.md and the archived changes are history). A lease that
/// picks a commit made before the move lands it without a conflict; this fails it. `files` are the files of `root`
#[must_use]
pub fn check_old_layout(root: &Path, files: &[String]) -> Vec<String> {
  let mut problems = Vec::new();
  for file in files {
    if file.starts_with("spec/") {
      problems.push(format!(
        "{file}: a spec of the old layout, which moved to openspec/specs/<area>/<name>/spec.md (docs/contributing/specs.md)"
      ));
      continue;
    }
    if file == "CHANGELOG.md" || file.starts_with(ARCHIVE) || is_binary(file) {
      continue;
    }
    let Some(text) = read(root, file) else {
      continue;
    };
    if let Some((at, old)) = old_layout_at(&text) {
      let line = text[..at].matches('\n').count() + 1;
      problems.push(format!("{file}:{line}: names {old}, a spec of the old layout: openspec/specs/<area>/<name>/spec.md"));
    }
  }
  problems
}

/// The problem of an AGENTS.md under `root` over its budget, or none
#[must_use]
pub fn check_agents_budget(root: &Path) -> Vec<String> {
  let bytes = std::fs::metadata(root.join("AGENTS.md")).map_or(0, |metadata| metadata.len());
  if bytes > AGENTS_BUDGET {
    vec![format!(
      "AGENTS.md: {bytes} bytes, over its budget of {AGENTS_BUDGET}: one line per rule there, the detail in the doc it links"
    )]
  } else {
    Vec::new()
  }
}

/// The report of `openspec validate <args> --strict --json` in `root`, by the binary of the devDependency of this
/// repository; an invalid item exits 1 with its report on stdout
///
/// # Errors
/// The binary does not run, or prints no report.
pub fn openspec_validate(root: &Path, args: &[&str]) -> Result<Value, String> {
  let windows = cfg!(windows);
  // the root of this repository, two directories above the crate, by its components: cmd.exe, which runs a `.cmd`,
  // reads no `/` and no `..` of a path
  let repository = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).unwrap_or_else(|| Path::new("."));
  let binary = repository.join("node_modules").join(".bin").join(if windows { "openspec.cmd" } else { "openspec" });
  let output = windowless_command(&binary)
    .arg("validate")
    .args(args)
    .args(["--strict", "--json", "--no-interactive"])
    .current_dir(root)
    .env("OPENSPEC_TELEMETRY", "0")
    .env("OPENSPEC_NO_UPDATE_CHECK", "1")
    .output()
    .map_err(|error| format!("{}: {error}", binary.display()))?;
  let stdout = String::from_utf8_lossy(&output.stdout);
  if stdout.trim().is_empty() {
    return Err(format!(
      "openspec validate {}: no report: {}",
      args.join(" "),
      String::from_utf8_lossy(&output.stderr).trim()
    ));
  }
  serde_json::from_str(&stdout).map_err(|error| format!("openspec validate {}: {error}", args.join(" ")))
}

/// The problems `openspec validate --strict` reports of every spec under `openspec/specs/`, of the changes under
/// `openspec/changes/` and of the archived ones (a task not done). `validate` answers the report of its arguments
#[must_use]
pub fn check_open_spec(root: &Path, validate: impl Fn(&Path, &[&str]) -> Result<Value, String>) -> Vec<String> {
  if !root.join("openspec").exists() {
    return Vec::new();
  }
  let mut problems = Vec::new();
  let mut report = |answer: Result<Value, String>| {
    let answer = match answer {
      Ok(answer) => answer,
      Err(error) => {
        problems.push(error);
        return;
      }
    };
    for item in answer["items"].as_array().into_iter().flatten() {
      let text = |key: &str| item[key].as_str().unwrap_or_default().to_string();
      let issues = item["issues"].as_array().cloned().unwrap_or_default();
      for issue in &issues {
        let line = issue["line"].as_u64().map_or_else(String::new, |line| format!(":{line}"));
        let message = issue["message"].as_str().unwrap_or_default().lines().next().unwrap_or_default();
        problems.push(format!(
          "openspec {} {}: {} {}{line}: {message}",
          text("type"),
          text("id"),
          issue["level"].as_str().unwrap_or_default(),
          issue["path"].as_str().unwrap_or_default()
        ));
      }
      if !item["valid"].as_bool().unwrap_or(true) && issues.is_empty() {
        problems.push(format!("openspec {} {}: invalid", text("type"), text("id")));
      }
    }
  };
  if !files_of(root, &[SPECS]).is_empty() {
    report(validate(root, &["--specs"]));
  }
  if !files_of(root, &["openspec/changes/*/proposal.md"]).is_empty() {
    report(validate(root, &["--changes"]));
  }
  if !files_of(root, &[&format!("{ARCHIVE}*/proposal.md")]).is_empty() {
    report(validate(root, &["--archived"]));
  }
  problems
}

#[cfg(test)]
mod lib_test;
