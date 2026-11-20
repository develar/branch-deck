//! Lost bindings: a commit removes what binds a name in a file, an import or a private declaration, while the file
//! after the commit still uses the name. Git merges such a commit cleanly onto a parent that added a use (an update
//! replays a commit that removed the last use of a declaration onto a baseline commit that uses it again), and the file
//! fails only when it is compiled.
//!
//! - **What the move brought, not what the commit has.** One file can't tell a lost binding from a name that resolves
//!   another way (an extension called in a lambda with a receiver, a default import, a helper of the package in another
//!   file): of 11741 commits of a monorepo's master, each built where it was authored, 68 had one (2026-11-20). So the
//!   finding is a lost binding of the copy of a commit that its original lacks (`introduced_lost_bindings`): the parent
//!   the copy got added the use, which the original never saw.
//! - **One file, two sides.** The rule reads the summaries (`code_symbols`) of the file before and after the commit:
//!   the name is bound before (an import of its name or alias, or a `private` declaration: of a class member, or
//!   private at the top level of a Kotlin file), it is bound after by nothing (no declaration of any reach, no import of
//!   it, no star import of its package and no new star import), and the file after still names it bare (or by `this`),
//!   in a use the summary keeps: references to locals and parameters are dropped by the summary itself.
//! - **Only what can't resolve another way.** A wider declaration may come from a supertype or another file of the
//!   package, so only `private` ones count; an import of a package that Kotlin or Java imports by default, or of the
//!   file's own package, binds nothing the file lacks without it; a name that the commit adds to another file of the
//!   package moved there. A Kotlin extension called on a receiver (`x.name()`) is no use: a member of the receiver's
//!   type of that name would take the call, and only a compiler knows that type; a missed lost binding costs a failed
//!   build, a wrong one a refused update.
//! - **Kotlin and Java**, the languages whose summaries hold the reach of a member; a degraded or truncated summary on
//!   either side, a declaration in a parse error or an `override` one tells nothing.
//! - **Cost**: the changes of the commits from one `log --raw` (`diff_scan::read_commit_changes`), the summaries from
//!   the object cache or one `cat-file --batch` of the misses (`symbol_scan::read_change_summaries`).
//! - **The update** (`replay_lost_bindings`): the copies of its replay against the local commits, both ranges from one
//!   `log --raw`, and only the files a copy and its original change from other blobs (the upstream changed them too)
//!   summarized; a finding counts only while the file at the tip still lacks the binding, so a later commit that binds
//!   the name again, the `fixup!` of a lost-bindings round of `brd update` among them, leaves none
//!   (docs/design/git-operations.md, "Update project"). `lost_names_in_edit` checks the edit of such a round.

use crate::diff_scan::{CommitDiff, DIFF_OPTIONS, Renames, parse_commit_diffs, read_commit_changes};
use crate::symbol_scan::read_change_summaries;
use anyhow::{Result, bail};
use code_symbols::extract::summarize;
use code_symbols::language::{Language, language_of};
use code_symbols::summary::{DeclKind, FileSummary, ImportTarget, Reach, Role};
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::ObjectCache;
use object_cache::commit_diff::FileChange;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::Arc;
use tracing::instrument;

/// What bound the name before the commit
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BindingKind {
  /// An import of the name or an alias of it
  Import,
  /// A `private` declaration of the file
  Declaration,
}

/// A name in a file whose binding a commit removes while the file after it still uses the name
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LostBinding {
  /// The commit that removes the binding, its full id
  pub commit: String,
  /// Its position among the commits asked for
  pub index: usize,
  pub file: String,
  pub name: String,
  pub kind: BindingKind,
  /// The line of the import or of the name of the declaration before the commit, 1-based
  pub binding_line: u32,
  /// The first line after the commit that uses the name, 1-based
  pub use_line: u32,
}

/// One binding of a name in a file before the commit, by `bindings_of`
#[derive(Debug, Clone, Copy)]
struct Binding {
  kind: BindingKind,
  line: u32,
  /// The import path of an import, an index into `FileSummary::names` of its summary per segment
  import_package: Option<usize>,
}

/// The lost bindings of `commits` (revisions without duplicates, `<head>~2` too, each against its first parent), in
/// their order, each commit's by file and line
#[instrument(skip_all, fields(commits = commits.len()))]
pub fn find_lost_bindings(git: &GitCommandExecutor, repository_path: &str, cache: &ObjectCache, commits: &[String]) -> Result<Vec<LostBinding>> {
  let diffs = read_commit_changes(git, repository_path, commits, Renames::Off)?;
  lost_of_diffs(git, repository_path, cache, &diffs)
}

/// The lost bindings of the commits of `diffs`, each with its position in `diffs` as its `index`
fn lost_of_diffs(git: &GitCommandExecutor, repository_path: &str, cache: &ObjectCache, diffs: &[CommitDiff]) -> Result<Vec<LostBinding>> {
  // the modified files on both sides, the files a commit adds for the names that moved into them
  let read = |change: &&FileChange| change.new_blob.is_some() && is_checked(&change.path);
  let has_candidates = diffs.iter().flat_map(|diff| &diff.files).any(is_candidate);
  if !has_candidates {
    return Ok(Vec::new());
  }
  let summaries = read_change_summaries(git, repository_path, cache, diffs.iter().flat_map(|diff| &diff.files).filter(read))?;
  let mut found = Vec::new();
  for (index, diff) in diffs.iter().enumerate() {
    for (file, lost) in lost_of_diff(diff, &summaries, |_| true) {
      found.push(LostBinding {
        commit: diff.id.clone(),
        index,
        file: file.to_string(),
        name: lost.name,
        kind: lost.kind,
        binding_line: lost.binding_line,
        use_line: lost.use_line,
      });
    }
  }
  Ok(found)
}

/// The lost bindings of the commit of `diff` in the files `checked` takes, by file, with the summaries of its files
fn lost_of_diff<'a>(diff: &'a CommitDiff, summaries: &HashMap<String, Arc<FileSummary>>, checked: impl Fn(&str) -> bool) -> Vec<(&'a str, LostName)> {
  let mut found = Vec::new();
  for change in diff.files.iter().filter(|change| is_candidate(change) && checked(&change.path)) {
    let (Some(before), Some(after)) = (summary(summaries, change.old_blob.as_deref()), summary(summaries, change.new_blob.as_deref())) else {
      continue;
    };
    for lost in lost_in(before, after) {
      if !moved(diff, summaries, &change.path, &lost.name) {
        found.push((change.path.as_str(), lost));
      }
    }
  }
  found
}

/// A change the rule reads: a Kotlin or Java file modified, with its blob on both sides
fn is_candidate(change: &FileChange) -> bool {
  change.status == 'M' && change.old_blob.is_some() && change.new_blob.is_some() && is_checked(&change.path)
}

/// The lost bindings that the copies of commits have and their originals don't, each with the index of its pair in
/// `pairs` (original, copy): what moving a commit onto another parent brought, a replay of an update or the build of a
/// virtual branch. A commit authored and built where it is has the lost bindings of its own as well, which resolve in
/// a way one file doesn't show (an extension's receiver in a lambda, a default import, a helper of the package): those
/// are no finding. One `log` and at most one `cat-file` for all of them; a pair of one commit is skipped
pub fn introduced_lost_bindings(git: &GitCommandExecutor, repository_path: &str, cache: &ObjectCache, pairs: &[(String, String)]) -> Result<Vec<(usize, LostBinding)>> {
  let moved: Vec<usize> = (0..pairs.len()).filter(|index| pairs[*index].0 != pairs[*index].1).collect();
  if moved.is_empty() {
    return Ok(Vec::new());
  }
  // the originals, then the copies, from one `log`; a copy that is its original is one record of it, then two keep the
  // order
  let originals: Vec<String> = moved.iter().map(|index| pairs[*index].0.clone()).collect();
  let copies: Vec<String> = moved.iter().map(|index| pairs[*index].1.clone()).collect();
  let revisions: Vec<String> = originals.iter().chain(&copies).cloned().collect();
  let mut diffs = read_commit_changes(git, repository_path, &revisions, Renames::Off)?;
  if diffs.len() != revisions.len() {
    diffs = read_commit_changes(git, repository_path, &originals, Renames::Off)?;
    diffs.extend(read_commit_changes(git, repository_path, &copies, Renames::Off)?);
  }
  let found = lost_of_diffs(git, repository_path, cache, &diffs)?;
  let own: HashSet<(usize, &str, &str)> = found
    .iter()
    .filter(|lost| lost.index < moved.len())
    .map(|lost| (moved[lost.index], lost.file.as_str(), lost.name.as_str()))
    .collect();
  Ok(
    found
      .iter()
      .filter(|lost| lost.index >= moved.len())
      .map(|lost| (moved[lost.index - moved.len()], lost))
      .filter(|(pair, lost)| !own.contains(&(*pair, lost.file.as_str(), lost.name.as_str())))
      .map(|(pair, lost)| (pair, lost.clone()))
      .collect(),
  )
}

/// A replay: the copies of the local commits on the upstream and the originals they copy, as two ranges
#[derive(Debug, Clone, Copy)]
pub struct ReplayRanges<'a> {
  /// The tip of the copies: the upstream with the replayed commits (and the commits a resolve session added) on it
  pub tip: &'a str,
  /// The tip of the originals: HEAD before the update
  pub head: &'a str,
  /// The upstream the copies are on: neither range has its commits
  pub upstream_head: &'a str,
  /// The previous upstream tips the originals leave out (`git_ops::update_branch::UpdatePlan::excluded`)
  pub excluded: &'a [String],
}

/// A lost binding that a replay brought: the copy of a local commit removes the binding of a name the file at the tip
/// still uses, and the original doesn't
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayedLostBinding {
  /// The local commit, its full id
  pub original: String,
  /// Its copy on the upstream
  pub copy: String,
  pub file: String,
  pub name: String,
  pub kind: BindingKind,
  /// The line of the binding before the copy, 1-based
  pub binding_line: u32,
  /// The first line at the tip that uses the name, 1-based
  pub use_line: u32,
}

/// What `replay_lost_bindings` read
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReplayLostBindings {
  /// By the order of the originals, each commit's by file and line
  pub found: Vec<ReplayedLostBinding>,
  /// The originals, oldest first
  pub originals: Vec<String>,
}

/// The format of a commit of `replay_lost_bindings`: the id, the tip it was reached from (`%S`, the copies' or the
/// originals'), and its identity (author email, author time, subject), split by `UNIT`
const REPLAY_FORMAT: &str = "--format=%x1e%H%x1f%S%x1f%ae%x1f%at%x1f%s";
const UNIT: char = '\x1f';

/// The lost bindings a replay brought (`ReplayRanges`): of a copy, which its original lacks, whose name the file at the
/// tip still uses without a binding, but the `kept` ones (file, name: the user said they resolve another way). A copy is
/// paired with its original by identity, the author's email and time and the subject (`git replay` drops a commit that
/// becomes empty, so positions shift); a copy without one (a `fixup!` a resolve session wrote) is no finding.
///
/// Only a file that the copy and its original change from other blobs can differ in the rule's answer: the upstream
/// changed it too. So both ranges come from one `log --raw` (`%S` tells them apart), and the summaries of the commits
/// with such a file from the object cache or one `cat-file --batch` of the misses: none at all when the upstream changed
/// no file a local commit changes
#[instrument(skip_all, fields(tip = ranges.tip, head = ranges.head))]
pub fn replay_lost_bindings(
  git: &GitCommandExecutor,
  repository_path: &str,
  cache: &ObjectCache,
  ranges: ReplayRanges<'_>,
  kept: &[(String, String)],
) -> Result<ReplayLostBindings> {
  let mut input = format!("{}\n{}\n^{}\n", ranges.tip, ranges.head, ranges.upstream_head);
  for excluded in ranges.excluded {
    input.push('^');
    input.push_str(excluded);
    input.push('\n');
  }
  let mut args = vec!["log", "--stdin", "--topo-order", "--reverse", REPLAY_FORMAT, "-z", "--raw", "--no-abbrev", "--no-renames"];
  args.extend(DIFF_OPTIONS);
  let output = git.cmd(&args, repository_path).stdin(input.as_bytes()).bytes()?;
  let mut copies: Vec<(String, CommitDiff)> = Vec::new();
  let mut originals: Vec<(String, CommitDiff)> = Vec::new();
  for mut diff in parse_commit_diffs(&output)? {
    let record = std::mem::take(&mut diff.id);
    let mut fields = record.splitn(3, UNIT);
    let (Some(id), Some(source), Some(identity)) = (fields.next(), fields.next(), fields.next()) else {
      bail!("Unexpected log record {record:?}");
    };
    diff.id = id.to_string();
    if source == ranges.tip {
      copies.push((identity.to_string(), diff));
    } else {
      originals.push((identity.to_string(), diff));
    }
  }
  let mut result = ReplayLostBindings {
    found: Vec::new(),
    originals: originals.iter().map(|(_, diff)| diff.id.clone()).collect(),
  };

  // each copy with its original, the oldest original of an identity first
  let mut by_identity: HashMap<&str, VecDeque<usize>> = HashMap::new();
  for (index, (identity, _)) in originals.iter().enumerate() {
    by_identity.entry(identity.as_str()).or_default().push_back(index);
  }
  let mut pairs: Vec<(&CommitDiff, &CommitDiff, HashSet<&str>)> = Vec::new();
  for (identity, copy) in &copies {
    let Some(original) = by_identity.get_mut(identity.as_str()).and_then(VecDeque::pop_front).map(|index| &originals[index].1) else {
      continue;
    };
    let changed: HashSet<&str> = copy
      .files
      .iter()
      .filter(|change| is_candidate(change))
      .filter(|change| {
        !original
          .files
          .iter()
          .any(|it| it.path == change.path && it.old_blob == change.old_blob && it.new_blob == change.new_blob)
      })
      .map(|change| change.path.as_str())
      .collect();
    if !changed.is_empty() {
      pairs.push((copy, original, changed));
    }
  }
  if pairs.is_empty() {
    return Ok(result);
  }

  // the file of each path at the tip: the last copy that changes it has it
  let mut at_tip: HashMap<&str, Option<&str>> = HashMap::new();
  for (_, copy) in &copies {
    for change in &copy.files {
      at_tip.insert(change.path.as_str(), change.new_blob.as_deref());
    }
  }
  let tip_files: Vec<FileChange> = pairs
    .iter()
    .flat_map(|(_, _, changed)| changed.iter())
    .filter_map(|path| at_tip.get(path).copied().flatten().map(|blob| tip_change(path, blob)))
    .collect();
  // every file of each pair for the names that moved into another one, and the files at the tip
  let read = |change: &&FileChange| (change.old_blob.is_some() || change.new_blob.is_some()) && is_checked(&change.path);
  let changes = pairs
    .iter()
    .flat_map(|(copy, original, _)| copy.files.iter().chain(&original.files))
    .chain(&tip_files)
    .filter(read);
  let summaries = read_change_summaries(git, repository_path, cache, changes)?;

  for (copy, original, changed) in &pairs {
    let own: HashSet<(&str, String)> = lost_of_diff(original, &summaries, |path| changed.contains(path))
      .into_iter()
      .map(|(file, lost)| (file, lost.name))
      .collect();
    for (file, lost) in lost_of_diff(copy, &summaries, |path| changed.contains(path)) {
      if own.contains(&(file, lost.name.clone())) || kept.iter().any(|(path, name)| path == file && *name == lost.name) {
        continue;
      }
      let before = copy
        .files
        .iter()
        .find(|change| change.path == file)
        .and_then(|change| summary(&summaries, change.old_blob.as_deref()));
      let tip = summary(&summaries, at_tip.get(file).copied().flatten());
      let (Some(before), Some(tip)) = (before, tip) else { continue };
      // a later copy may bind the name again or drop its use: only what the tip still lacks fails the build
      let Some(at_tip) = lost_in(before, tip).into_iter().find(|it| it.name == lost.name) else {
        continue;
      };
      result.found.push(ReplayedLostBinding {
        original: original.id.clone(),
        copy: copy.id.clone(),
        file: file.to_string(),
        name: lost.name,
        kind: lost.kind,
        binding_line: lost.binding_line,
        use_line: at_tip.use_line,
      });
    }
  }
  Ok(result)
}

/// A change that only names the blob of `path` at a tip, for `read_change_summaries`
fn tip_change(path: &str, blob: &str) -> FileChange {
  FileChange {
    status: 'A',
    path: path.to_string(),
    old_path: None,
    additions: 0,
    deletions: 0,
    git_additions: 0,
    git_deletions: 0,
    binary: false,
    old_blob: None,
    new_blob: Some(blob.to_string()),
    hunks: Vec::new(),
  }
}

/// The names that `before` binds and `edited` binds by nothing while it still uses them bare: the rule of one file on
/// an edit of it, both parsed in process. Nothing for a file of another language than Kotlin and Java
pub fn lost_names_in_edit(path: &str, before: &[u8], edited: &[u8]) -> Vec<LostName> {
  match language_of(path) {
    Some(language @ (Language::Kotlin | Language::Java)) => lost_in(&summarize(language, path, before), &summarize(language, path, edited)),
    _ => Vec::new(),
  }
}

/// Whether the commit of `diff` adds a declaration of `name` to another file of the package of `file`: the name moved
/// there, where the package sees it without an import
fn moved(diff: &CommitDiff, summaries: &HashMap<String, Arc<FileSummary>>, file: &str, name: &str) -> bool {
  let package_of = |change: &FileChange| summary(summaries, change.new_blob.as_deref()).map(|summary| summary.package.clone());
  let Some(package) = diff.files.iter().find(|change| change.path == file).and_then(package_of) else {
    return false;
  };
  diff.files.iter().filter(|change| change.path != file).any(|change| {
    let Some(after) = summary(summaries, change.new_blob.as_deref()) else { return false };
    after.package == package && after.declares(name) && !summary(summaries, change.old_blob.as_deref()).is_some_and(|before| before.declares(name))
  })
}

fn summary<'a>(summaries: &'a HashMap<String, Arc<FileSummary>>, blob: Option<&str>) -> Option<&'a FileSummary> {
  summaries.get(blob?).map(Arc::as_ref)
}

/// Whether the rule reads a file of this path: Kotlin and Java
fn is_checked(path: &str) -> bool {
  matches!(language_of(path), Some(Language::Kotlin | Language::Java))
}

/// A lost binding of one file, without the commit and the path
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LostName {
  pub name: String,
  pub kind: BindingKind,
  pub binding_line: u32,
  pub use_line: u32,
}

/// The names `before` binds that `after` binds by nothing while it still uses them bare, by name: the rule of one file
pub fn lost_in(before: &FileSummary, after: &FileSummary) -> Vec<LostName> {
  if !readable(before) || !readable(after) || before.language != after.language {
    return Vec::new();
  }
  let bound_before = bindings_of(before);
  if bound_before.is_empty() {
    return Vec::new();
  }
  let before_stars: HashSet<Vec<&str>> = star_imports(before);
  let after_stars: HashSet<Vec<&str>> = star_imports(after);
  let new_star = after_stars.iter().any(|path| !before_stars.contains(path));
  let mut lost = Vec::new();
  for (name, binding) in bound_before {
    if binds(after, name) {
      continue;
    }
    if binding.kind == BindingKind::Import {
      let package = import_path(before, binding.import_package);
      if new_star || after_stars.contains(&package) {
        continue;
      }
    }
    let Some(use_line) = after
      .references_to(name)
      .filter(|reference| is_bare_use(after, reference))
      .map(|reference| reference.line)
      .min()
    else {
      continue;
    };
    lost.push(LostName {
      name: name.to_string(),
      kind: binding.kind,
      binding_line: binding.line,
      use_line,
    });
  }
  lost
}

/// A summary the rule trusts: neither truncated nor degraded
fn readable(summary: &FileSummary) -> bool {
  !summary.truncated && !summary.is_degraded()
}

/// The names a file binds that nothing else may bind once they go, by name, the first binding of each: the imports of
/// a name (its alias when it has one) of a package other than the file's own and the default ones, and the `private`
/// declarations not in a parse error that override nothing
fn bindings_of(summary: &FileSummary) -> BTreeMap<&str, Binding> {
  let mut bindings: BTreeMap<&str, Binding> = BTreeMap::new();
  let own_package: Option<Vec<&str>> = summary.package.as_deref().map(|package| package.split('.').collect());
  for (index, import) in summary.imports.iter().enumerate() {
    let ImportTarget::Name(target) = import.target else { continue };
    if import.exported {
      continue;
    }
    let path: Vec<&str> = import.path.iter().map(|id| summary.name(*id)).collect();
    if is_default_package(summary.language, &path) || own_package.as_ref() == Some(&path) {
      continue;
    }
    let name = summary.name(import.alias.unwrap_or(target));
    bindings.entry(name).or_insert(Binding {
      kind: BindingKind::Import,
      line: import.line,
      import_package: Some(index),
    });
  }
  for declaration in &summary.declarations {
    if declaration.kind == DeclKind::Binding || declaration.in_error || declaration.overrides || !matches!(declaration.reach, Reach::Container | Reach::File) {
      continue;
    }
    let name = summary.name(declaration.name);
    bindings.entry(name).or_insert(Binding {
      kind: BindingKind::Declaration,
      line: declaration.name_line,
      import_package: None,
    });
  }
  // a name declared beside a private one of the same name (an overload of another reach) stays bound
  bindings.retain(|name, binding| {
    binding.kind == BindingKind::Import
      || !summary
        .declarations
        .iter()
        .any(|it| summary.name(it.name) == *name && !matches!(it.reach, Reach::Container | Reach::File))
  });
  bindings
}

/// The import path of the import at `index` of `summary`
fn import_path(summary: &FileSummary, index: Option<usize>) -> Vec<&str> {
  index
    .and_then(|index| summary.imports.get(index))
    .map_or_default(|import| import.path.iter().map(|id| summary.name(*id)).collect())
}

/// Whether `summary` binds `name` in any way: a declaration of any reach or kind, or an import of the name or an alias
fn binds(summary: &FileSummary, name: &str) -> bool {
  summary.declares(name)
    || summary.imports.iter().any(|import| match import.target {
      ImportTarget::Name(target) => summary.name(import.alias.unwrap_or(target)) == name,
      _ => false,
    })
}

/// The paths of the star imports of `summary`
fn star_imports(summary: &FileSummary) -> HashSet<Vec<&str>> {
  summary
    .imports
    .iter()
    .filter(|import| import.target == ImportTarget::All)
    .map(|import| import.path.iter().map(|id| summary.name(*id)).collect())
    .collect()
}

/// A use of a name that only a binding of the file gives: bare or by `this`, as a value, a call or a type
fn is_bare_use(summary: &FileSummary, reference: &code_symbols::summary::Reference) -> bool {
  if !matches!(reference.role, Role::Value | Role::Call | Role::Type) {
    return false;
  }
  match reference.qualifier.as_deref() {
    None => true,
    Some([this]) => summary.name(*this) == "this",
    Some(_) => false,
  }
}

/// The packages whose names a file of `language` sees without an import
fn is_default_package(language: Language, path: &[&str]) -> bool {
  const KOTLIN: [&[&str]; 10] = [
    &["kotlin"],
    &["kotlin", "annotation"],
    &["kotlin", "collections"],
    &["kotlin", "comparisons"],
    &["kotlin", "io"],
    &["kotlin", "ranges"],
    &["kotlin", "sequences"],
    &["kotlin", "text"],
    &["kotlin", "jvm"],
    &["java", "lang"],
  ];
  match language {
    Language::Kotlin => KOTLIN.contains(&path),
    Language::Java => path == ["java", "lang"],
    _ => false,
  }
}
