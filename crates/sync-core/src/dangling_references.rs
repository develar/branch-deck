//! Dangling references: a branch removes a declaration that its virtual tree still uses, because the commit that
//! changes the use is in another branch. Git merges such a branch cleanly, and it fails only when it is compiled
//! (`bd check`, `bd suggest`, `bd safe-push`).
//!
//! - **Declarations by the lines.** A removed line of a commit is a declaration when it declares a name that other files
//!   can use: a top-level type or function, a static member (used as `Stem.NAME`, the file's stem). Instance members and
//!   locals are not searched. Language by extension: JVM (Java, Kotlin, Groovy, Scala), Rust, TypeScript and JavaScript,
//!   Python, Go.
//! - **Only where the trees differ.** A use that the virtual tree of a branch has and HEAD's tree lacks can only be in a
//!   path where the two trees differ: the paths of the other branches' commits, a few hundred, not the whole tree. One
//!   `diff-tree --stdin` for all branches, one `cat-file --batch` for the blobs; the search runs in Rust.
//! - **Counts, not lines.** A path dangles when the virtual tree has more lines that use the name than HEAD's tree has,
//!   so a use another commit only reformatted is no dangling reference.
//! - **Evidence.** `fixed_by` is the commit outside the branch whose patch removes the use; without one the reference is
//!   only possible. The uses that landed on the baseline after the base (`<base> <baseline tip>`) are possible too:
//!   Patronus merges onto the baseline tip.
//! - The changed lines of each commit come from one `log -z --raw -p -U0 --no-renames`, by the paths of the raw
//!   entries, and are cached by commit id (`commit_lines`).
//! - **By the summaries** (`declaration_checks`, with `branchdeck.symbols on`): an instance or static member a branch
//!   removes, a form it changes and a declaration it adds again, on the same `diff-tree` and `cat-file` (`TreeSearch`),
//!   each a reference of its `DanglingKind`.

use crate::check::{BranchCheck, CheckOutcome};
use crate::diff_scan::{HUNK_OPTIONS, split_records};
use crate::symbol_dependencies::declaration_checks::{DeclarationFacts, DeclarationSearch, Fit, TreeForm, searches_of};
use anyhow::{Result, anyhow};
use code_symbols::extract::summarize;
use code_symbols::language::language_of as symbols_language_of;
use code_symbols::summary::{FileSummary, Role};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::object_texts::read_texts;
use object_cache::cache::ObjectCache;
use object_cache::commit_lines::FileLines;
use regex::Regex;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex};
use tracing::{debug, instrument};

/// What a branch does to the declaration of a dangling reference
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DanglingKind {
  /// It removes the declaration
  Removed,
  /// It changes the form of the declaration (`declaration_checks`): its header, its parameters, its reach or the
  /// constructors of its type, and the use may not fit the new form
  Changed,
  /// It declares the key again (`declaration_checks`): the use is the other declaration, in a file its tree has and
  /// HEAD's lacks
  Duplicate,
}

/// A use of a name that a branch removes or changes, or a declaration of a name it adds again, in its virtual tree but
/// not in HEAD's tree (or on the baseline tip)
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(clippy::struct_excessive_bools, reason = "independent facts of a reference")]
pub struct DanglingReference {
  pub kind: DanglingKind,
  pub identifier: String,
  /// The path of the use
  pub file: String,
  /// The first line of the use in the tree that has it, 1-based
  pub line: u32,
  /// The commit of the branch that removes the declaration, changes it or declares it again
  pub removed_by: String,
  /// The commit outside the branch whose patch removes the use; `None` makes the reference only possible
  pub fixed_by: Option<String>,
  /// The use is on the baseline tip, which got it after the base of the branch
  pub on_baseline: bool,
  /// The summary can't tell that the use breaks (`declaration_checks::Fit::Unknown`: a changed return type or
  /// nullability, new type parameters, a suspend call in the lambda of an unknown function): only possible, with
  /// `fixed_by` too
  pub possible: bool,
  /// The lines `fixed_by` adds in `file` use a name the branch declares (the fix needs the branch): stacking one
  /// branch on the other can't help, moving the fix into `removed_by` does
  pub mutual: bool,
  /// The syntax trees of both sides confirmed the uses (`code_symbols`); `false` where the lines alone found them: a
  /// language without a grammar, or a file the parser can't read well
  pub by_syntax: bool,
}

/// The syntax summaries of the blobs the checks of a session read, by their `cat-file` name, parsed once per session
/// and never written to the object cache: the blobs of a virtual tree are new in every trial
#[derive(Debug, Default)]
pub struct SyntaxMemo {
  summaries: Mutex<HashMap<String, Option<Arc<FileSummary>>>>,
}

impl SyntaxMemo {
  /// The summary of `text`, the blob `name` of `path`; `None` for a language without a grammar and a file the parser
  /// can't read well (a twentieth of its bytes in errors, or cut by the budget)
  fn summary(&self, name: &str, path: &str, text: &str) -> Option<Arc<FileSummary>> {
    if let Some(known) = self.summaries.lock().ok()?.get(name) {
      return known.clone();
    }
    let summary = symbols_language_of(path)
      .map(|language| summarize(language, text.as_bytes()))
      .filter(|summary| !summary.is_degraded() && !summary.truncated)
      .map(Arc::new);
    self.summaries.lock().ok()?.insert(name.to_string(), summary.clone());
    summary
  }
}

/// The uses of a path on the side with the uses and on the other side, as `Search::uses` gives them, and whether the
/// syntax trees filtered them
type SidesUses<'a> = (Vec<(usize, &'a str)>, Vec<(usize, &'a str)>, bool);

/// The candidate lines of `uses` on which the syntax tree has a use of `name`: a comment, a string, a local of the
/// same name, a key or a named argument and an `override` member are none. A subtraction only: what the tree can't
/// place stays out, what the lines missed is not added
fn on_syntax<'a>(uses: Vec<(usize, &'a str)>, summary: &FileSummary, name: &str) -> Vec<(usize, &'a str)> {
  // `override fun name` declares a member: the line search takes it for a use, the syntax tree must not confirm it
  let lines: HashSet<usize> = summary
    .references_to(name)
    .filter(|reference| reference.role != Role::Override)
    .filter_map(|reference| usize::try_from(reference.line).ok())
    .collect();
  uses.into_iter().filter(|(index, _)| lines.contains(&(index + 1))).collect()
}

impl DanglingReference {
  /// Whether a commit proves it: one outside the branch changed the use
  pub const fn is_confirmed(&self) -> bool {
    self.fixed_by.is_some() && !self.on_baseline && !self.possible
  }
}

/// The source languages whose declarations are read; a use is searched in the files of the same language
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Language {
  Jvm,
  Rust,
  Script,
  Python,
  Go,
}

fn language_of(path: &str) -> Option<Language> {
  let extension = path.rsplit_once('.')?.1;
  Some(match extension {
    "java" | "kt" | "kts" | "groovy" | "scala" => Language::Jvm,
    "rs" => Language::Rust,
    "ts" | "tsx" | "mts" | "cts" | "js" | "jsx" | "mjs" | "cjs" => Language::Script,
    "py" => Language::Python,
    "go" => Language::Go,
    _ => return None,
  })
}

/// How other files use a declared name
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reach {
  /// A top-level type, function or constant: by its name
  TopLevel,
  /// A static member or a nested type: qualified by the stem of its file (`AcpScenarioAgent.CONTROL_URL_ENV`)
  Member,
  /// Visible in its own file only
  Private,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Declaration {
  name: String,
  reach: Reach,
}

/// An annotation before a declaration (`@Deprecated`, `@JvmStatic`, `#[inline]`)
const ANNOTATIONS: &str = r"(?:@[\w.]+(?:\([^)]*\))?\s+)*";

static JVM_TYPE: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(&format!(
    r"^(?P<indent>\s*){ANNOTATIONS}(?P<mods>(?:(?:public|protected|private|internal|static|final|abstract|sealed|non-sealed|open|data|value|inline|enum|annotation|inner|expect|actual|strictfp|fun)\s+)*)(?:class|interface|enum|record|object|@interface|typealias)\s+(?P<name>[A-Za-z_$][\w$]*)"
  ))
  .expect("valid regex")
});
static KOTLIN_FUN: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(&format!(
    r"^(?P<indent>\s*){ANNOTATIONS}(?P<mods>(?:(?:public|protected|private|internal|open|override|abstract|final|inline|suspend|tailrec|operator|infix|external|expect|actual)\s+)*)fun\s+(?:<[^>]*>\s*)?(?:[\w$.<>?, ]+\.)?(?P<name>[A-Za-z_$][\w$]*)\s*\("
  ))
  .expect("valid regex")
});
static KOTLIN_PROPERTY: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(&format!(
    r"^(?P<indent>\s*){ANNOTATIONS}(?P<mods>(?:(?:public|protected|private|internal|const|lateinit|override|open|expect|actual)\s+)*)(?:val|var)\s+(?:[\w$.<>?, ]+\.)?(?P<name>[A-Za-z_$][\w$]*)\b"
  ))
  .expect("valid regex")
});
static JAVA_MEMBER: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(&format!(
    r"^(?P<indent>\s*){ANNOTATIONS}(?P<mods>(?:(?:public|protected|private|static|final|abstract|synchronized|native|transient|volatile|default)\s+)+)(?:<[^>]*>\s*)?[\w$.<>\[\], ?]+?\s+(?P<name>[A-Za-z_$][\w$]*)\s*[=;(]"
  ))
  .expect("valid regex")
});
/// A constant of an interface, implicitly static: `String CONTROL_URL_ENV = "…";`
static JAVA_INTERFACE_CONSTANT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?P<indent>\s+)[\w$.<>\[\]]+\s+(?P<name>[A-Z][A-Z0-9_]+)\s*=").expect("valid regex"));
static RUST_ITEM: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(
    r#"^(?P<indent>\s*)(?P<pub>pub(?:\([^)]*\))?\s+)?(?:(?:async|const|unsafe|extern\s+"[^"]*")\s+)*(?:fn|struct|enum|trait|const|static|type|union|mod)\s+(?P<name>[A-Za-z_]\w*)"#,
  )
  .expect("valid regex")
});
static RUST_MACRO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?P<indent>\s*)macro_rules!\s+(?P<name>[A-Za-z_]\w*)").expect("valid regex"));
static SCRIPT_EXPORT: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"^(?P<indent>\s*)(?P<export>export\s+(?:default\s+)?)?(?:declare\s+)?(?:abstract\s+)?(?:async\s+)?(?:function\*?|class|const|let|var|interface|type|enum|namespace)\s+(?P<name>[A-Za-z_$][\w$]*)").expect("valid regex")
});
static PYTHON_DEFINITION: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^(?:(?:async\s+)?def\s+(?P<function>\w+)\s*\(|class\s+(?P<class>\w+)|(?P<constant>[A-Z][A-Z0-9_]+)\s*(?::[^=]*)?=)").expect("valid regex"));
static GO_DEFINITION: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"^(?:func\s+(?P<function>\w+)\s*[\[(]|type\s+(?P<type>\w+)|(?:var|const)\s+(?P<value>\w+))").expect("valid regex"));
static JVM_PACKAGE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*package\s+(?P<name>[\w.]+)").expect("valid regex"));

/// The shortest name searched: shorter ones are everywhere
const MIN_NAME_LENGTH: usize = 3;
/// The largest blob searched
const MAX_BLOB_BYTES: usize = 1 << 20;
/// The most paths of the baseline after the base that are searched; a baseline far ahead is left out
const MAX_BASELINE_PATHS: usize = 2000;

/// The declaration a line of a file in `language` makes, if other files (or its own) can use it
fn declaration_of(language: Language, line: &str) -> Option<Declaration> {
  let indented = |captures: &regex::Captures| captures.name("indent").is_some_and(|indent| !indent.as_str().is_empty());
  let has_mod = |captures: &regex::Captures, modifier: &str| captures.name("mods").is_some_and(|mods| mods.as_str().split_whitespace().any(|m| m == modifier));
  let declaration = |name: &str, reach: Reach| (name.len() >= MIN_NAME_LENGTH).then(|| Declaration { name: name.to_string(), reach });
  match language {
    Language::Jvm => {
      if let Some(captures) = JVM_TYPE.captures(line) {
        let reach = if has_mod(&captures, "private") {
          Reach::Private
        } else if indented(&captures) {
          Reach::Member
        } else {
          Reach::TopLevel
        };
        return declaration(&captures["name"], reach);
      }
      for regex in [&*KOTLIN_FUN, &*KOTLIN_PROPERTY] {
        if let Some(captures) = regex.captures(line) {
          return match (has_mod(&captures, "private"), indented(&captures), has_mod(&captures, "const")) {
            (true, _, _) => declaration(&captures["name"], Reach::Private),
            (false, false, _) => declaration(&captures["name"], Reach::TopLevel),
            // a constant of an object or a companion; other members are instance members
            (false, true, true) => declaration(&captures["name"], Reach::Member),
            (false, true, false) => None,
          };
        }
      }
      if let Some(captures) = JAVA_MEMBER.captures(line) {
        return match (has_mod(&captures, "private"), has_mod(&captures, "static")) {
          (true, _) => declaration(&captures["name"], Reach::Private),
          (false, true) => declaration(&captures["name"], Reach::Member),
          (false, false) => None,
        };
      }
      let captures = JAVA_INTERFACE_CONSTANT.captures(line)?;
      declaration(&captures["name"], Reach::Member)
    }
    Language::Rust => {
      let captures = RUST_ITEM.captures(line).or_else(|| RUST_MACRO.captures(line))?;
      if indented(&captures) {
        return None;
      }
      let reach = if captures.name("pub").is_some() || captures.get(0).is_some_and(|m| m.as_str().contains("macro_rules!")) {
        Reach::TopLevel
      } else {
        Reach::Private
      };
      declaration(&captures["name"], reach)
    }
    Language::Script => {
      let captures = SCRIPT_EXPORT.captures(line)?;
      if indented(&captures) {
        return None;
      }
      let reach = if captures.name("export").is_some() { Reach::TopLevel } else { Reach::Private };
      declaration(&captures["name"], reach)
    }
    Language::Python => {
      let captures = PYTHON_DEFINITION.captures(line)?;
      let name = ["function", "class", "constant"].iter().find_map(|group| captures.name(group))?.as_str();
      declaration(name, if name.starts_with('_') { Reach::Private } else { Reach::TopLevel })
    }
    Language::Go => {
      let captures = GO_DEFINITION.captures(line)?;
      let name = ["function", "type", "value"].iter().find_map(|group| captures.name(group))?.as_str();
      declaration(name, if name.starts_with(char::is_uppercase) { Reach::TopLevel } else { Reach::Private })
    }
  }
}

/// The stem of a path: `AcpScenarioAgent` of `src/acp/AcpScenarioAgent.java`
fn stem(path: &str) -> &str {
  let name = path.rsplit_once('/').map_or(path, |(_, name)| name);
  name.split_once('.').map_or(name, |(stem, _)| stem)
}

/// `pattern` as a whole word: no identifier character (a JVM name may have `$`) before or after it
fn whole_word(pattern: &str) -> Regex {
  Regex::new(&format!("(?:^|[^A-Za-z0-9_$]){pattern}(?:$|[^A-Za-z0-9_$])")).expect("an escaped pattern is a valid regex")
}

/// How a removed declaration is searched
struct Search {
  name: String,
  /// The file it was removed from
  file: String,
  language: Language,
  removed_by: String,
  /// The uses in the files of `language`; `None` searches only `file`
  pattern: Option<Regex>,
  /// The unqualified uses in `file` itself, for a private declaration or a member: a top-level one is found by
  /// `pattern`, and one moved within its package still resolves there
  local_pattern: Option<Regex>,
  /// A top-level JVM declaration moved to other files: searched as `<old package>.<name>` once the packages are read,
  /// when they differ
  moved: Option<Moved>,
}

/// The `cat-file` names of the old file of a moved declaration and of the files it moved to
struct Moved {
  old_file: String,
  new_files: Vec<String>,
}

impl Search {
  /// The regex for a use in `path`, or `None` when the path isn't searched
  fn pattern_for(&self, path: &str) -> Option<&Regex> {
    if path == self.file
      && let Some(local) = &self.local_pattern
    {
      return Some(local);
    }
    self.pattern.as_ref().filter(|_| language_of(path) == Some(self.language))
  }

  /// The lines of `text` that use the name, without the lines that declare it
  fn uses<'a>(&self, path: &str, text: &'a str) -> Vec<(usize, &'a str)> {
    // most blobs don't name it at all: one substring search instead of one per line
    if !text.contains(self.name.as_str()) {
      return Vec::new();
    }
    let Some(pattern) = self.pattern_for(path) else { return Vec::new() };
    let language = language_of(path);
    text
      .lines()
      .enumerate()
      .filter(|(_, line)| line.contains(self.name.as_str()) && pattern.is_match(line))
      .filter(|(_, line)| {
        language
          .and_then(|language| declaration_of(language, line))
          .is_none_or(|declaration| declaration.name != self.name)
      })
      .collect()
  }

  /// How many of `lines` use the name
  fn count_in<'a>(&self, path: &str, lines: impl IntoIterator<Item = &'a String>) -> usize {
    let Some(pattern) = self.pattern_for(path) else { return 0 };
    lines.into_iter().filter(|line| line.contains(self.name.as_str()) && pattern.is_match(line)).count()
  }
}

/// Whether an added line of `remaining` uses a name that the added lines of `moved` declare for other files (a top-level
/// declaration by its name, a member as `<stem>.<name>`): a move of `moved` alone would leave that use without its
/// declaration. A heuristic by the lines, as the dangling references are; a declaration line itself is no use
pub(crate) fn uses_added_declarations<'a>(moved: impl IntoIterator<Item = &'a FileLines>, remaining: impl IntoIterator<Item = &'a FileLines>) -> bool {
  let mut searches: Vec<(Language, String, Regex)> = Vec::new();
  for file in moved {
    let Some(language) = language_of(&file.path) else { continue };
    for line in &file.added {
      match declaration_of(language, line) {
        Some(Declaration { name, reach: Reach::TopLevel }) => searches.push((language, name.clone(), whole_word(&regex::escape(&name)))),
        Some(Declaration { name, reach: Reach::Member }) => {
          let pattern = whole_word(&format!(r"{}\s*\.\s*{}", regex::escape(stem(&file.path)), regex::escape(&name)));
          searches.push((language, name, pattern));
        }
        _ => {}
      }
    }
  }
  if searches.is_empty() {
    return false;
  }
  remaining.into_iter().any(|file| {
    let Some(language) = language_of(&file.path) else { return false };
    file.added.iter().any(|line| {
      searches.iter().any(|(searched, name, pattern)| {
        *searched == language && line.contains(name.as_str()) && pattern.is_match(line) && declaration_of(language, line).is_none_or(|declaration| declaration.name != *name)
      })
    })
  })
}

/// The changed lines of `ids` in the files of the known languages, from the cache, then one `git log` for the rest
#[instrument(skip(git, ids, cache), fields(commits = ids.len()))]
pub fn read_commit_lines(git: &GitCommandExecutor, repository_path: &str, ids: &[String], cache: &ObjectCache) -> Result<HashMap<String, Vec<FileLines>>> {
  let mut lines = cache.get_commit_lines(ids);
  let missing: Vec<&String> = ids.iter().filter(|id| !lines.contains_key(*id)).collect();
  if missing.is_empty() {
    return Ok(lines);
  }
  let input = missing.iter().fold(String::new(), |mut input, id| {
    input.push_str(id);
    input.push('\n');
    input
  });
  let mut args = vec![
    "log",
    "--no-walk=unsorted",
    "--stdin",
    "--format=%x1e%H",
    "-z",
    "--raw",
    "-p",
    "-U0",
    "--no-renames",
    "--no-ext-diff",
    "--no-textconv",
    "--src-prefix=a/",
    "--dst-prefix=b/",
  ];
  // the lines are cached: which ones a hunk holds, and in which order, must not depend on the user's diff config
  args.extend(HUNK_OPTIONS);
  let output = git.cmd(&args, repository_path).stdin(input.as_bytes()).bytes()?;
  let read = parse_commit_lines(&output);
  cache.put_commit_lines(read.iter().map(|(id, files)| (id.as_str(), files)));
  lines.extend(read);
  Ok(lines)
}

/// The most lines kept per file and side: a generated file of thousands of lines declares nothing anyone looks for
const MAX_LINES_PER_FILE: usize = 5000;

/// Parse `log --format=%x1e%H -z --raw -p -U0` into the lines of the files of the known languages, by commit. The paths
/// are those of the raw entries, NUL-separated and verbatim: the `---`/`+++` lines of the patch end a name with a space
/// with a tab and C-quote one with a quote, a backslash or a control character. The file sections of the patch follow
/// the raw entries in their order; a type change has two, the old file deleted and the new one added, whose lines are
/// one file's
fn parse_commit_lines(output: &[u8]) -> HashMap<String, Vec<FileLines>> {
  let mut commits = HashMap::new();
  for record in split_records(output) {
    let Some(RawRecord { id, paths, patch }) = split_raw_part(record) else { continue };
    let mut files: Vec<Option<FileLines>> = paths.iter().map(|_| None).collect();
    // the entry of the current section, the sections it still has, and whether its lines are read
    let (mut current, mut sections_left, mut in_hunks) = (None::<usize>, 0, false);
    for line in patch.split(|&b| b == b'\n') {
      let line = line.strip_suffix(b"\r").unwrap_or(line);
      if line.starts_with(b"diff --git ") {
        in_hunks = false;
        if sections_left == 0 {
          let next = current.map_or(0, |index| index + 1);
          if next >= paths.len() {
            debug!(commit = %id, "more patch sections than raw entries");
            break;
          }
          current = Some(next);
          sections_left = if paths[next].1 == b'T' { 2 } else { 1 };
        }
        sections_left -= 1;
        continue;
      }
      let Some(index) = current.filter(|index| language_of(&paths[*index].0).is_some()) else {
        continue;
      };
      if line.starts_with(b"@@") {
        in_hunks = true;
        files[index].get_or_insert_with(|| FileLines {
          path: paths[index].0.clone(),
          removed: Vec::new(),
          added: Vec::new(),
        });
        continue;
      }
      let Some(file) = files[index].as_mut().filter(|_| in_hunks) else { continue };
      let (side, text) = match line.split_first() {
        Some((b'-', text)) => (&mut file.removed, text),
        Some((b'+', text)) => (&mut file.added, text),
        _ => continue,
      };
      if side.len() < MAX_LINES_PER_FILE {
        side.push(String::from_utf8_lossy(text).into_owned());
      }
    }
    commits.insert(id, files.into_iter().flatten().collect());
  }
  commits
}

/// A record of `log -z --raw -p`
struct RawRecord<'a> {
  id: String,
  /// The path and the status letter of each raw entry
  paths: Vec<(String, u8)>,
  patch: &'a [u8],
}

/// The id of a record of `log -z --raw -p`, its raw entries (`:<modes> <ids> <status>` NUL `<path>` NUL) and the patch
/// after the NUL that ends them
fn split_raw_part(record: &[u8]) -> Option<RawRecord<'_>> {
  fn field<'a>(rest: &mut &'a [u8]) -> Option<&'a [u8]> {
    let end = rest.iter().position(|&b| b == 0)?;
    let field = &rest[..end];
    *rest = &rest[end + 1..];
    Some(field)
  }
  let mut rest = record;
  let id = String::from_utf8_lossy(field(&mut rest)?).trim().to_string();
  rest = rest.strip_prefix(b"\n").unwrap_or(rest);
  let mut paths = Vec::new();
  while rest.first() == Some(&b':') {
    let status = field(&mut rest)?.rsplit(|&b| b == b' ').next().and_then(|status| status.first().copied()).unwrap_or(b'M');
    let first = field(&mut rest)?;
    // a rename or a copy names the old path first
    let path = if matches!(status, b'R' | b'C') { field(&mut rest)? } else { first };
    paths.push((String::from_utf8_lossy(path).into_owned(), status));
  }
  Some(RawRecord {
    id,
    paths,
    patch: rest.strip_prefix(b"\0").unwrap_or(rest),
  })
}

/// What a dangling-reference check needs besides the trial builds
pub struct DanglingInput<'a> {
  /// HEAD, whose tree has every commit
  pub head: &'a str,
  /// The commit the root branches are built on
  pub base: Option<&'a str>,
  /// The baseline tip, when it is ahead of `base`
  pub baseline_tip: Option<&'a str>,
  /// Every commit ahead of the baseline, oldest first
  pub commits: &'a [String],
  /// The changed lines of `commits`
  pub lines: &'a HashMap<String, Vec<FileLines>>,
  /// Only the references of these branches; `None` for every built branch
  pub of: Option<&'a [String]>,
  /// The syntax trees that filter the uses the lines find; `None` with `branchdeck.symbols` off
  pub syntax: Option<&'a SyntaxMemo>,
  /// The summaries and changes of `commits` for the checks by the declarations (`declaration_checks`); `None` unless
  /// `branchdeck.symbols` is `on` and a commit changes a source file of a grammar
  pub declarations: Option<DeclarationFacts<'a>>,
}

/// One pair of trees that `diff-tree` compares: `(the side with the uses, the other side)`, and per path the blobs
struct TreeDiff {
  /// path -> (blob of the side with the uses, blob of the other side); `None` for a side without the path
  paths: BTreeMap<String, (Option<String>, Option<String>)>,
}

/// The dangling references of each built branch of a check, by branch
#[instrument(skip_all, fields(branches = branches.len()))]
pub fn find_dangling_references(
  git: &GitCommandExecutor,
  repository_path: &str,
  input: &DanglingInput<'_>,
  branches: &[BranchCheck],
) -> Result<BTreeMap<String, Vec<DanglingReference>>> {
  let position: HashMap<&str, usize> = input.commits.iter().enumerate().map(|(index, id)| (id.as_str(), index)).collect();
  let by_name: HashMap<&str, &BranchCheck> = branches.iter().map(|branch| (branch.name.as_str(), branch)).collect();

  // per built branch: the searches of the declarations it removes, and what its virtual tree holds
  struct Built<'a> {
    name: &'a str,
    head: &'a str,
    searches: Vec<Search>,
    /// The commits in the virtual tree: the branch's and its ancestors'
    members: HashSet<&'a str>,
    /// The names its commits and its ancestors' declare
    declared: HashSet<String>,
    /// What its commits remove, change or declare again, searched by the syntax trees
    declarations: Vec<DeclarationSearch<'a>>,
  }
  let mut built: Vec<Built> = Vec::new();
  let mut packages_needed: Vec<String> = Vec::new();
  for branch in branches {
    let CheckOutcome::Ok { head } = &branch.outcome else { continue };
    if input.of.is_some_and(|of| !of.contains(&branch.name)) {
      continue;
    }
    let mut members: HashSet<&str> = branch.commits.iter().map(String::as_str).collect();
    let mut ancestor = branch.built_on.as_deref();
    // bounded: a broken stack can't loop
    for _ in 0..branches.len() {
      let Some(name) = ancestor else { break };
      let Some(parent) = by_name.get(name) else { break };
      members.extend(parent.commits.iter().map(String::as_str));
      ancestor = parent.built_on.as_deref();
    }

    // the declarations the members add, by name -> files
    let mut added: HashMap<String, HashSet<String>> = HashMap::new();
    for id in &members {
      for file in input.lines.get(*id).into_iter().flatten() {
        let Some(language) = language_of(&file.path) else { continue };
        for line in &file.added {
          if let Some(declaration) = declaration_of(language, line) {
            added.entry(declaration.name).or_default().insert(file.path.clone());
          }
        }
      }
    }

    let mut removed: BTreeMap<(String, String), (Declaration, &str)> = BTreeMap::new();
    let mut own: Vec<&String> = branch.commits.iter().collect();
    own.sort_by_key(|id| position.get(id.as_str()).copied().unwrap_or(usize::MAX));
    let declarations = input.declarations.map_or_else(Vec::new, |facts| {
      let own: Vec<&str> = own.iter().map(|id| id.as_str()).collect();
      searches_of(facts, &own, &members)
    });
    for id in own {
      for file in input.lines.get(id).into_iter().flatten() {
        let Some(language) = language_of(&file.path) else { continue };
        for line in &file.removed {
          let Some(declaration) = declaration_of(language, line) else { continue };
          removed.insert((declaration.name.clone(), file.path.clone()), (declaration, id.as_str()));
        }
      }
    }

    let mut searches = Vec::new();
    for ((name, file), (declaration, removed_by)) in removed {
      let language = language_of(&file).expect("a file with a declaration has a language");
      let moved_to: Vec<&String> = added.get(&name).map_or_default(|files| files.iter().filter(|other| **other != file).collect());
      let escaped = regex::escape(&name);
      let (pattern, moved) = match declaration.reach {
        // a private declaration made visible elsewhere: its file imports it from there
        Reach::Private if !moved_to.is_empty() => continue,
        Reach::Private => (None, None),
        // a member moved with its class (a file of the same stem elsewhere) keeps its qualified name
        Reach::Member if moved_to.iter().any(|other| stem(other) == stem(&file)) => continue,
        Reach::Member => (Some(whole_word(&format!(r"{}\s*\.\s*(?:Companion\s*\.\s*)?{escaped}", regex::escape(stem(&file))))), None),
        Reach::TopLevel if moved_to.is_empty() => (Some(whole_word(&escaped)), None),
        // a moved top-level declaration breaks the imports of its old package only
        Reach::TopLevel if language == Language::Jvm => {
          let moved = Moved {
            old_file: format!("{removed_by}^:{file}"),
            new_files: moved_to.iter().map(|other| format!("{head}:{other}")).collect(),
          };
          packages_needed.push(moved.old_file.clone());
          packages_needed.extend(moved.new_files.iter().cloned());
          (None, Some(moved))
        }
        Reach::TopLevel => continue,
      };
      searches.push(Search {
        local_pattern: (declaration.reach != Reach::TopLevel).then(|| whole_word(&escaped)),
        name,
        file,
        language,
        removed_by: removed_by.to_string(),
        pattern,
        moved,
      });
    }
    if searches.is_empty() && declarations.is_empty() {
      continue;
    }
    built.push(Built {
      name: &branch.name,
      head,
      searches,
      members,
      declared: added.into_keys().collect(),
      declarations,
    });
  }
  if built.is_empty() {
    return Ok(BTreeMap::new());
  }

  // one diff-tree: each virtual tree against HEAD's tree, and the baseline tip against the base
  let mut pairs: Vec<(String, String)> = built.iter().map(|branch| (branch.head.to_string(), input.head.to_string())).collect();
  // the baseline tip ahead of the base; on the local baseline branch the tip is HEAD itself
  let baseline = input.baseline_tip.zip(input.base).filter(|(tip, base)| tip != base && *tip != input.head);
  if let Some((tip, base)) = baseline {
    pairs.push((tip.to_string(), base.to_string()));
  }
  let diffs = diff_trees(git, repository_path, &pairs)?;

  // one cat-file: the blobs of the searched paths, the removing files in each virtual tree, and the packages
  let mut names: Vec<String> = Vec::new();
  let mut index_of: HashMap<String, usize> = HashMap::new();
  let mut want = |name: String, names: &mut Vec<String>| {
    if !index_of.contains_key(&name) {
      index_of.insert(name.clone(), names.len());
      names.push(name);
    }
  };
  for (branch, (_, diff)) in built.iter().zip(&diffs) {
    for (path, (ours, theirs)) in &diff.paths {
      // a declaration search reads a blob whose summary the commits don't have, or one that names it
      let declarations_want = || {
        input.declarations.is_some_and(|facts| {
          let summary = ours.as_ref().and_then(|blob| facts.input.summaries.get(blob)).map(AsRef::as_ref);
          branch.declarations.iter().any(|search| search.searches(facts.input, path) && search.may_use(summary))
        })
      };
      if branch.searches.iter().any(|search| search.pattern_for(path).is_some()) || declarations_want() {
        ours.iter().chain(theirs).for_each(|blob| want(blob.clone(), &mut names));
      }
    }
    for search in &branch.searches {
      want(format!("{}:{}", branch.head, search.file), &mut names);
    }
    if let Some(facts) = input.declarations {
      for search in &branch.declarations {
        let name = branch_file(facts, diff, branch.head, search.file);
        if !facts.input.summaries.contains_key(&name) {
          want(name, &mut names);
        }
      }
    }
  }
  let baseline_diff = baseline.and_then(|_| diffs.last()).map(|(_, diff)| diff).filter(|diff| {
    let fits = diff.paths.len() <= MAX_BASELINE_PATHS;
    if !fits {
      debug!(paths = diff.paths.len(), "the baseline is far ahead of the base: its uses are not searched");
    }
    fits
  });
  if let Some(diff) = baseline_diff {
    for (path, (ours, theirs)) in &diff.paths {
      if built
        .iter()
        .any(|branch| branch.searches.iter().any(|search| search.pattern.is_some() && search.pattern_for(path).is_some()))
      {
        ours.iter().chain(theirs).for_each(|blob| want(blob.clone(), &mut names));
      }
    }
  }
  for name in &packages_needed {
    want(name.clone(), &mut names);
  }
  let texts = read_texts(git, repository_path, &names, MAX_BLOB_BYTES)?;
  let text = |name: &str| index_of.get(name).and_then(|index| texts[*index].as_deref());
  let syntax = |name: &str, path: &str| input.syntax.and_then(|memo| memo.summary(name, path, text(name)?));
  // the uses of both sides of a path, by the syntax trees when both can be read (one method for both: a side the parser
  // can't read keeps the lines on both); `true` when the trees filtered them
  let uses_of = |search: &Search, path: &str, ours: &str, theirs: Option<&str>| -> Option<SidesUses<'_>> {
    let ours_text = text(ours)?;
    let theirs_text = theirs.and_then(text).unwrap_or_default();
    let (uses, their_uses) = (search.uses(path, ours_text), search.uses(path, theirs_text));
    if uses.is_empty() {
      return Some((uses, their_uses, false));
    }
    let Some(ours_syntax) = syntax(ours, path) else {
      return Some((uses, their_uses, false));
    };
    let theirs_syntax = match theirs {
      Some(theirs) => match syntax(theirs, path) {
        Some(summary) => Some(summary),
        None => return Some((uses, their_uses, false)),
      },
      None => None,
    };
    let their_uses = theirs_syntax.map_or_else(Vec::new, |summary| on_syntax(their_uses, &summary, &search.name));
    Some((on_syntax(uses, &ours_syntax, &search.name), their_uses, true))
  };
  let package_of = |name: &str| text(name).and_then(|text| text.lines().find_map(|line| JVM_PACKAGE.captures(line).map(|c| c["name"].to_string())));

  let mut result: BTreeMap<String, Vec<DanglingReference>> = BTreeMap::new();
  for (branch, (_, diff)) in built.iter_mut().zip(&diffs) {
    for search in &mut branch.searches {
      if let Some(moved) = search.moved.take()
        && let Some(old_package) = package_of(&moved.old_file)
        && moved.new_files.iter().all(|new_file| package_of(new_file).as_deref() != Some(old_package.as_str()))
      {
        search.pattern = Some(whole_word(&format!(r"{}\.{}", regex::escape(&old_package), regex::escape(&search.name))));
      }
    }
    let mut found: Vec<DanglingReference> = Vec::new();
    for search in &branch.searches {
      // still declared in its file (an overload, a change in place, a later commit), or the file is the old one; a
      // `use` or an import alias that binds the name keeps it too
      let in_branch = format!("{}:{}", branch.head, search.file);
      let redeclared = text(&in_branch).is_some_and(|content| {
        content
          .lines()
          .any(|line| declaration_of(search.language, line).is_some_and(|declaration| declaration.name == search.name))
      }) || syntax(&in_branch, &search.file).is_some_and(|summary| summary.declares(&search.name));
      if redeclared {
        continue;
      }
      for (path, (ours, theirs)) in &diff.paths {
        let Some((uses, their_uses, by_syntax)) = ours.as_deref().and_then(|ours| uses_of(search, path, ours, theirs.as_deref())) else {
          continue;
        };
        if uses.len() <= their_uses.len() {
          continue;
        }
        let their_lines: HashSet<&str> = their_uses.iter().map(|(_, line)| line.trim()).collect();
        let (line, _) = uses.iter().find(|(_, line)| !their_lines.contains(line.trim())).unwrap_or(&uses[0]);
        // the oldest commit outside the virtual tree whose patch removes the use
        let fixed_by = input.commits.iter().filter(|id| !branch.members.contains(id.as_str())).find_map(|id| {
          let file = input.lines.get(id)?.iter().find(|file| file.path == *path)?;
          (search.count_in(path, &file.removed) > search.count_in(path, &file.added)).then_some((id, file))
        });
        let mutual = fixed_by.is_some_and(|(_, file)| {
          file.added.iter().any(|line| {
            branch
              .declared
              .iter()
              .any(|name| line.contains(name.as_str()) && whole_word(&regex::escape(name)).is_match(line))
          })
        });
        found.push(DanglingReference {
          kind: DanglingKind::Removed,
          identifier: search.name.clone(),
          file: path.clone(),
          line: u32::try_from(line + 1).unwrap_or(u32::MAX),
          removed_by: search.removed_by.clone(),
          fixed_by: fixed_by.map(|(id, _)| id.clone()),
          on_baseline: false,
          possible: false,
          mutual,
          by_syntax,
        });
      }
      if let Some(diff) = baseline_diff {
        for (path, (tip, base)) in &diff.paths {
          let Some((uses, base_uses, by_syntax)) = tip.as_deref().and_then(|tip| uses_of(search, path, tip, base.as_deref())) else {
            continue;
          };
          if uses.len() <= base_uses.len() || found.iter().any(|known| known.file == *path && known.identifier == search.name) {
            continue;
          }
          found.push(DanglingReference {
            kind: DanglingKind::Removed,
            identifier: search.name.clone(),
            file: path.clone(),
            line: u32::try_from(uses[0].0 + 1).unwrap_or(u32::MAX),
            removed_by: search.removed_by.clone(),
            fixed_by: None,
            on_baseline: true,
            possible: false,
            mutual: false,
            by_syntax,
          });
        }
      }
    }
    if let Some(facts) = input.declarations {
      let context = TreeSearch {
        facts,
        input,
        branch_head: branch.head,
        members: &branch.members,
        declared: &branch.declared,
        diff,
        text: &text,
        syntax: &syntax,
      };
      for search in &branch.declarations {
        for reference in context.references(search) {
          // the line search found it already: a static member by its qualified name
          if !found.iter().any(|known| known.file == reference.file && known.identifier == reference.identifier) {
            found.push(reference);
          }
        }
      }
    }
    if !found.is_empty() {
      found.sort_by(|a, b| (&a.file, a.line, &a.identifier).cmp(&(&b.file, b.line, &b.identifier)));
      result.insert(branch.name.to_string(), found);
    }
  }
  Ok(result)
}

/// The `cat-file` name of the branch's version of `file`: its blob where the branch's tree differs from HEAD's there,
/// else HEAD's, the blob the newest commit that changes the file leaves, whose summary the commits have; else
/// `<head>:<file>`
fn branch_file(facts: DeclarationFacts<'_>, diff: &TreeDiff, head: &str, file: &str) -> String {
  let blob = match diff.paths.get(file) {
    Some((ours, _)) => ours.clone(),
    None => facts
      .diffs
      .iter()
      .rev()
      .flat_map(|diff| &diff.files)
      .find(|change| change.path == file)
      .and_then(|change| change.new_blob.clone()),
  };
  blob.unwrap_or_else(|| format!("{head}:{file}"))
}

/// What the searches by the declarations of one branch read (`declaration_checks`)
struct TreeSearch<'s, 'a> {
  facts: DeclarationFacts<'a>,
  input: &'s DanglingInput<'a>,
  branch_head: &'s str,
  /// The commits in its virtual tree
  members: &'s HashSet<&'a str>,
  /// The names its commits and its ancestors' declare, for `mutual`
  declared: &'s HashSet<String>,
  diff: &'s TreeDiff,
  /// The text of a `cat-file` name
  text: &'s dyn Fn(&str) -> Option<&'s str>,
  /// The summary of a `cat-file` name, the blob of a path
  syntax: &'s dyn Fn(&str, &str) -> Option<Arc<FileSummary>>,
}

impl TreeSearch<'_, '_> {
  /// The dangling references of `search` in the paths where the branch's tree differs from HEAD's
  fn references(&self, search: &DeclarationSearch<'_>) -> Vec<DanglingReference> {
    let symbols = self.facts.input;
    let Some(current) = self.summary(&branch_file(self.facts, self.diff, self.branch_head, search.file), search.file) else {
      return Vec::new();
    };
    let Some(form) = search.in_tree(symbols, search.file, &current) else {
      return Vec::new();
    };
    let words = search.words();
    let mut found = Vec::new();
    // a duplicate: the declarations of its key in the paths where the trees differ, and in its own file
    let mut declared = if self.diff.paths.contains_key(search.file) {
      0
    } else {
      search.uses(symbols, &form, search.file, &current).len()
    };
    for (path, (ours, theirs)) in &self.diff.paths {
      let Some(ours) = ours.as_deref().filter(|_| search.searches(symbols, path)) else {
        continue;
      };
      // by the names of a summary the commits have, else by the text, before it is parsed
      let known = symbols.summaries.get(ours);
      if !search.may_use(known.map(AsRef::as_ref)) {
        continue;
      }
      let Some(ours_text) = (self.text)(ours).filter(|text| known.is_some() || words.iter().any(|word| text.contains(word.as_str()))) else {
        continue;
      };
      let Some(ours_summary) = self.summary(ours, path) else { continue };
      // both sides by the trees, or the path is not judged
      let theirs_summary = match theirs.as_deref() {
        Some(theirs) => match self.summary(theirs, path) {
          Some(summary) => Some(summary),
          None => continue,
        },
        None => None,
      };
      let mut uses = search.uses(symbols, &form, path, &ours_summary);
      if search.kind == DanglingKind::Changed {
        // a use the branch's own commits wrote is written against the form they gave the declaration: by the lines of
        // their hunks, where they lead from the base to the branch's blob, else by the text of their added lines
        if let Some(written) = self.written_lines(path, ours) {
          uses.retain(|(line, _)| !written.contains(line));
        } else {
          let written: HashSet<&str> = self.written_by_branch(path);
          let lines: Vec<&str> = ours_text.lines().collect();
          uses.retain(|(line, _)| !lines.get(*line as usize - 1).is_some_and(|text| written.contains(text.trim())));
        }
      }
      declared += uses.len();
      let their_uses = theirs_summary.as_ref().map_or_else(Vec::new, |summary| search.uses(symbols, &form, path, summary));
      // the file of an added declaration has a second one only when it declares the key twice
      if uses.len() <= their_uses.len() || (search.kind == DanglingKind::Duplicate && path == search.file && uses.len() < 2) {
        continue;
      }
      let their_lines: HashSet<&str> = theirs.as_deref().and_then(|theirs| (self.text)(theirs)).map_or_default(|text| {
        let lines: Vec<&str> = text.lines().collect();
        their_uses.iter().filter_map(|(line, _)| lines.get(*line as usize - 1)).map(|line| line.trim()).collect()
      });
      let ours_lines: Vec<&str> = ours_text.lines().collect();
      let (line, _) = uses
        .iter()
        .copied()
        .find(|(line, _)| ours_lines.get(*line as usize - 1).is_none_or(|text| !their_lines.contains(text.trim())))
        .unwrap_or(uses[0]);
      // proven only by more uses the summary tells break than HEAD has
      let breaking = |uses: &[(u32, Fit)]| uses.iter().filter(|(_, fit)| *fit == Fit::Breaks).count();
      let possible = breaking(&uses) <= breaking(&their_uses);
      let (fixed_by, needs_branch) = self.fixed_by(search, &form, path);
      let mutual = needs_branch
        || fixed_by.is_some_and(|id| {
          self.input.lines.get(id).into_iter().flatten().filter(|file| file.path == *path).any(|file| {
            file.added.iter().any(|line| {
              self
                .declared
                .iter()
                .any(|name| line.contains(name.as_str()) && whole_word(&regex::escape(name)).is_match(line))
            })
          })
        });
      found.push(DanglingReference {
        kind: search.kind,
        identifier: search.name.to_string(),
        file: path.clone(),
        line,
        removed_by: search.commit.to_string(),
        fixed_by: fixed_by.map(str::to_string),
        on_baseline: false,
        possible,
        mutual,
        by_syntax: true,
      });
    }
    if search.kind == DanglingKind::Duplicate && declared < 2 {
      found.clear();
    }
    found
  }

  /// The summary of the blob `id` of `path`: the one of a commit's file when a commit has the blob, which the object
  /// cache holds, else parsed in the session (`SyntaxMemo`)
  fn summary(&self, id: &str, path: &str) -> Option<Arc<FileSummary>> {
    match self.facts.input.summaries.get(id) {
      Some(summary) => Some(Arc::clone(summary)).filter(|summary| !summary.is_degraded() && !summary.truncated),
      None => (self.syntax)(id, path),
    }
  }

  /// The lines of `ours`, the branch's blob of `path`, that its commits wrote: their added lines, followed through the
  /// hunks of the later ones. `None` where the hunks don't lead from the base to `ours`: a commit of the branch whose
  /// old blob of the path is not the one before it in the branch's tree (another commit, outside, came between)
  fn written_lines(&self, path: &str, ours: &str) -> Option<HashSet<u32>> {
    let changes = self.facts.diffs.iter().flat_map(|diff| diff.files.iter().map(move |change| (diff, change)));
    // the base's blob, before the first commit of the range that changes the path
    let mut blob = changes
      .clone()
      .find(|(_, change)| change.path == path || change.old_path.as_deref() == Some(path))?
      .1
      .old_blob
      .clone();
    let mut written: HashSet<u32> = HashSet::new();
    for (_, change) in changes.filter(|(diff, change)| self.members.contains(diff.id.as_str()) && change.path == path) {
      if change.old_blob != blob || change.old_path.as_deref().is_some_and(|old| old != path) {
        return None;
      }
      written = written
        .into_iter()
        .filter_map(|line| {
          let mut shift: i64 = 0;
          for hunk in &change.hunks {
            let end = hunk.old_start + hunk.old_lines;
            if hunk.old_lines > 0 && (hunk.old_start..end).contains(&line) {
              return None;
            }
            let before = if hunk.old_lines == 0 { hunk.old_start < line } else { end <= line };
            if before {
              shift += i64::from(hunk.new_lines) - i64::from(hunk.old_lines);
            }
          }
          u32::try_from(i64::from(line) + shift).ok()
        })
        .collect();
      written.extend(change.hunks.iter().flat_map(|hunk| hunk.new_start..hunk.new_start + hunk.new_lines));
      blob.clone_from(&change.new_blob);
    }
    (blob.as_deref() == Some(ours)).then_some(written)
  }

  /// The trimmed lines the commits of the branch's tree add to `path`
  fn written_by_branch(&self, path: &str) -> HashSet<&str> {
    self
      .members
      .iter()
      .filter_map(|id| self.input.lines.get(*id))
      .flatten()
      .filter(|file| file.path == path)
      .flat_map(|file| file.added.iter().map(|line| line.trim()))
      .collect()
  }

  /// The oldest commit outside the branch's tree whose change of `path` removes uses the search counts, and whether
  /// that change uses what only the branch's form of the declaration takes
  fn fixed_by(&self, search: &DeclarationSearch<'_>, form: &TreeForm<'_>, path: &str) -> (Option<&str>, bool) {
    let symbols = self.facts.input;
    let summary_of = |blob: Option<&String>| blob.and_then(|blob| symbols.summaries.get(blob)).filter(|summary| !summary.is_degraded());
    for diff in self.facts.diffs.iter().filter(|diff| !self.members.contains(diff.id.as_str())) {
      for change in &diff.files {
        let old_path = change.old_path.as_deref().unwrap_or(&change.path);
        if old_path != path {
          continue;
        }
        let Some(before) = summary_of(change.old_blob.as_ref()) else { continue };
        let after = summary_of(change.new_blob.as_ref()).map(|summary| (change.path.as_str(), summary.as_ref()));
        if search.removes_use(symbols, form, (old_path, before), after) {
          let needs = after.is_some_and(|after| search.needs_new_form(symbols, form, Some((old_path, before)), after));
          return (Some(diff.id.as_str()), needs);
        }
      }
    }
    (None, false)
  }
}

/// One `diff-tree --stdin -r -z --raw` for `pairs` of commits `(the side with the uses, the other side)`
fn diff_trees(git: &GitCommandExecutor, repository_path: &str, pairs: &[(String, String)]) -> Result<Vec<(String, TreeDiff)>> {
  let input = pairs.iter().fold(String::new(), |mut input, (ours, theirs)| {
    // `<commit> <parent>`: the diff from the parent to the commit
    input.push_str(ours);
    input.push(' ');
    input.push_str(theirs);
    input.push('\n');
    input
  });
  // full blob ids: `diff-tree` abbreviates only when asked, and `--abbrev=40` would cut a SHA-256 id
  let output = git
    .cmd(&["diff-tree", "--stdin", "-r", "-z", "--raw", "--no-renames"], repository_path)
    .stdin(input.as_bytes())
    .bytes()?;
  let output = String::from_utf8_lossy(&output);
  let mut diffs: HashMap<String, TreeDiff> = HashMap::new();
  let mut fields = output.split('\0');
  let mut current: Option<String> = None;
  while let Some(field) = fields.next() {
    if field.is_empty() {
      continue;
    }
    if let Some(raw) = field.strip_prefix(':') {
      // `:<old mode> <new mode> <old blob> <new blob> <status>`, then the path
      let path = fields.next().ok_or_else(|| anyhow!("diff-tree output ends after {field:?}"))?;
      let parts: Vec<&str> = raw.split(' ').collect();
      let [_, _, theirs, ours, _] = parts.as_slice() else {
        return Err(anyhow!("Unexpected diff-tree line {field:?}"));
      };
      let blob = |id: &str| (!id.bytes().all(|byte| byte == b'0')).then(|| id.to_string());
      let commit = current.clone().ok_or_else(|| anyhow!("diff-tree line before its commit: {field:?}"))?;
      diffs
        .entry(commit)
        .or_insert_with(|| TreeDiff { paths: BTreeMap::new() })
        .paths
        .insert(path.to_string(), (blob(ours), blob(theirs)));
    } else {
      current = Some(field.trim().to_string());
    }
  }
  Ok(
    pairs
      .iter()
      .map(|(ours, _)| (ours.clone(), diffs.remove(ours).unwrap_or(TreeDiff { paths: BTreeMap::new() })))
      .collect(),
  )
}

#[cfg(test)]
#[path = "dangling_references_test.rs"]
mod dangling_references_test;
