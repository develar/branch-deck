use crate::areas::{DEFAULT_AREA_DEPTH, DirLines, top_areas};
use crate::context::RepoContext;
use crate::dependencies::{Dependency, compute_dependencies, connected_components};
use crate::diff_scan::{CommitDiff, read_commit_diffs_cached, read_commit_patches};
use crate::error::{CliError, ErrorCode};
use crate::generated::generated_paths;
use crate::output::{SchemaVersion, ShortIds};
use crate::status::{read_grouping, read_head};
use clap::builder::RangedU64ValueParser;
use git_ops::commit_list::Commit;
use object_cache::commit_diff::FileChange;
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use sync_core::autosquash::{FixupKind, plan_autosquash};
use sync_core::commit_grouper::{AUTOSQUASH_MARKERS, GroupingInputs, parse_subject};
use sync_core::commit_ids::{IdMatch, match_commit_id};
use sync_core::grouping::BranchGrouping;
use sync_core::move_commits::with_branch_prefix;
use sync_utils::issue_pattern::find_issue_number;

#[derive(Debug, clap::Args)]
#[expect(clippy::struct_excessive_bools, reason = "independent command line flags")]
pub struct CommitsArgs {
  /// Only these commits (full ids, or abbreviated to at least 4 characters), and the edges that involve them
  #[arg(value_name = "ID", conflicts_with_all = ["branch", "unassigned"])]
  pub ids: Vec<String>,
  /// The changed files of each commit: status, path, added and deleted lines
  #[arg(long, conflicts_with = "brief")]
  pub files: bool,
  /// The patch of each commit, with three lines of context
  #[arg(long, conflicts_with = "brief")]
  pub patch: bool,
  /// Only the commits of this branch, and the edges and overlaps that involve it
  #[arg(long)]
  pub branch: Option<String>,
  /// Only the unassigned commits, and the edges that involve them
  #[arg(long, conflicts_with = "branch")]
  pub unassigned: bool,
  /// Keep lock files and generated files in the dependencies, the overlaps and the areas
  #[arg(long)]
  pub include_generated: bool,
  /// A compact view: per commit the id, subject, group, issue key, fixup target and top areas, the dependencies
  /// without files, and `units`, the commits that must stay together
  #[arg(long)]
  pub brief: bool,
  /// How many directories an area of --brief keeps
  #[arg(long, value_name = "N", default_value_t = DEFAULT_AREA_DEPTH, value_parser = RangedU64ValueParser::<usize>::new().range(1..), requires = "brief")]
  pub depth: usize,
}

/// What `bd commits` and `bd commits --brief` start with
#[derive(Debug, Serialize, JsonSchema)]
pub struct CommitsHeader {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  pub head: String,
  pub baseline: String,
}

/// `bd commits`: every commit ahead of the baseline, oldest first, with what ties them together
#[derive(Debug, Serialize, JsonSchema)]
pub struct CommitsOutput {
  #[serde(flatten)]
  pub header: CommitsHeader,
  /// Oldest first, the order they are applied in
  pub commits: Vec<CommitEntry>,
  /// `from` depends on `to`: its hunks touch lines `to` added (approximate, see docs/guidelines/agent-cli.md). Lock
  /// files and generated files don't count without --include-generated
  pub dependencies: Vec<DependencyEdge>,
  /// For each pair of current branches, the files both change; without lock files and generated files unless
  /// --include-generated
  pub cluster_overlap: Vec<ClusterOverlap>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CommitEntry {
  pub id: String,
  /// The full subject, `(prefix)` included
  pub subject: String,
  /// The message after the subject; omitted when empty
  #[serde(default, skip_serializing_if = "String::is_empty")]
  pub body: String,
  /// `Name <email>`
  pub author: String,
  /// Author time, Unix seconds
  pub time: u32,
  /// The branch the commit is grouped into; `null` for an unassigned commit
  pub group: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub issue_key: Option<String>,
  /// For a `fixup!`, `squash!` or `amend!` commit: what it folds into
  #[serde(skip_serializing_if = "Option::is_none")]
  pub fixup: Option<FixupTarget>,
  /// With --files
  #[serde(skip_serializing_if = "Option::is_none")]
  pub files: Option<Vec<FileEntry>>,
  /// With --patch
  #[serde(skip_serializing_if = "Option::is_none")]
  pub patch: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FixupMarker {
  Fixup,
  Squash,
  Amend,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FixupTarget {
  pub kind: FixupMarker,
  /// The target commit, as `git rebase --autosquash` resolves it; `null` when no older commit matches
  pub target: Option<String>,
  /// The subject the marker names
  pub target_subject: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FileEntry {
  pub path: String,
  /// The path before a rename
  #[serde(skip_serializing_if = "Option::is_none")]
  pub old_path: Option<String>,
  /// `A`, `M`, `D`, `R` (rename) or `T` (type change)
  pub status: String,
  /// Added lines; omitted for a binary file
  #[serde(skip_serializing_if = "Option::is_none")]
  pub additions: Option<u32>,
  /// Deleted lines; omitted for a binary file
  #[serde(skip_serializing_if = "Option::is_none")]
  pub deletions: Option<u32>,
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub binary: bool,
  /// A lock file, or marked `linguist-generated` or `-diff` in the attributes
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub generated: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DependencyEdge {
  /// The dependent (newer) commit
  pub from: String,
  /// The commit it depends on
  pub to: String,
  pub files: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ClusterOverlap {
  pub branches: [String; 2],
  pub files: Vec<String>,
}

/// `bd commits --brief`: the commits of `bd commits` without bodies, authors and files, small enough to read whole
#[derive(Debug, Serialize, JsonSchema)]
pub struct CommitsBrief {
  #[serde(flatten)]
  pub header: CommitsHeader,
  /// Oldest first
  pub commits: Vec<BriefCommit>,
  /// `from` depends on `to`, as the `dependencies` of `bd commits`
  pub dependencies: Vec<BriefEdge>,
  /// The commits that must stay together: the connected components of the dependencies and of the fixups with their
  /// targets, computed over every commit. Only components of two or more commits, ordered by their oldest commit, each
  /// oldest first
  pub units: Vec<Vec<String>>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct BriefCommit {
  pub id: String,
  /// The subject without the `(prefix)` when the prefix is `group`; a stack path or a prefix the grouper sanitizes
  /// stays, and so does a `fixup!` marker
  pub subject: String,
  /// The branch the commit is grouped into; `null` for an unassigned commit
  pub group: Option<String>,
  /// The issue key of the subject; omitted when it is `group`, an issue branch
  #[serde(skip_serializing_if = "Option::is_none")]
  pub issue_key: Option<String>,
  /// For a `fixup!`, `squash!` or `amend!` commit: what it folds into
  #[serde(skip_serializing_if = "Option::is_none")]
  pub fixup: Option<BriefFixup>,
  /// The directories (cut to --depth) with the most added plus deleted lines, at most three, without those under 5 % of
  /// the lines except the first; hub files (`HUB_FILES` of `generated.rs`) don't count, and lock files and generated
  /// files only with --include-generated
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub areas: Vec<DirLines>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct BriefFixup {
  pub kind: FixupMarker,
  /// The target commit, as `git rebase --autosquash` resolves it; `null` when no older commit matches
  pub target: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct BriefEdge {
  /// The dependent (newer) commit
  pub from: String,
  /// The commit it depends on
  pub to: String,
}

/// The changes of the commits ahead of the baseline, and the generated paths among them
pub(crate) struct LinkedDiffs {
  /// The same positions as the ids they were read for
  pub diffs: Vec<CommitDiff>,
  /// Lock files and generated files; read only when the links leave them out or the output flags them
  pub generated: HashSet<String>,
  /// `diffs` without the generated files; `None` when they count (--include-generated)
  linked: Option<Vec<CommitDiff>>,
}

impl LinkedDiffs {
  /// The changes that link commits (dependencies, overlaps, areas), at the same positions as `diffs`
  pub fn linked(&self) -> &[CommitDiff] {
    self.linked.as_deref().unwrap_or(&self.diffs)
  }

  pub fn is_generated(&self, change: &FileChange) -> bool {
    self.generated.contains(&change.path) || change.old_path.as_ref().is_some_and(|path| self.generated.contains(path))
  }
}

/// The changes of `ids` (oldest first) from the object cache and one `log` for the misses, and the generated paths
/// among them from one `check-attr`. The linked changes leave the generated files out unless `include_generated`;
/// `flag_generated` reads the generated paths with `include_generated` too, for an output that flags them
pub(crate) fn read_linked_diffs(context: &RepoContext, ids: &[String], include_generated: bool, flag_generated: bool) -> Result<LinkedDiffs, CliError> {
  let diffs = read_commit_diffs_cached(&context.git, &context.repository, &context.cache(), ids)?;
  let generated = if flag_generated || !include_generated {
    let paths = diffs
      .iter()
      .flat_map(|diff| &diff.files)
      .flat_map(|change| std::iter::once(change.path.as_str()).chain(change.old_path.as_deref()));
    generated_paths(&context.git, &context.repository, paths)?
  } else {
    HashSet::new()
  };
  let mut result = LinkedDiffs { diffs, generated, linked: None };
  if !include_generated {
    let linked = result.diffs.iter().map(|diff| CommitDiff {
      id: diff.id.clone(),
      files: diff.files.iter().filter(|change| !result.is_generated(change)).cloned().collect(),
    });
    result.linked = Some(linked.collect());
  }
  Ok(result)
}

/// The commits an output lists
enum Scope {
  All,
  Branch(String),
  Unassigned,
  /// Full ids
  Commits(HashSet<String>),
}

impl Scope {
  fn of(args: &CommitsArgs, commit_ids: &[String]) -> Result<Self, CliError> {
    Ok(if let Some(branch) = &args.branch {
      Self::Branch(branch.clone())
    } else if args.unassigned {
      Self::Unassigned
    } else if args.ids.is_empty() {
      Self::All
    } else {
      Self::Commits(resolve_ids(&args.ids, commit_ids)?)
    })
  }

  fn contains(&self, id: &str, group: Option<&str>) -> bool {
    match self {
      Self::All => true,
      Self::Branch(name) => group == Some(name.as_str()),
      Self::Unassigned => group.is_none(),
      Self::Commits(ids) => ids.contains(id),
    }
  }
}

/// The full ids of `ids`, each a prefix of at least 4 characters of exactly one commit ahead of the baseline
pub(crate) fn resolve_ids(ids: &[String], commit_ids: &[String]) -> Result<HashSet<String>, CliError> {
  let mut resolved = HashSet::new();
  let (mut unknown, mut ambiguous) = (Vec::new(), Vec::new());
  for id in ids {
    match match_commit_id(id, commit_ids.iter().map(String::as_str)) {
      IdMatch::One(full) => {
        resolved.insert(full.to_string());
      }
      IdMatch::None => unknown.push(id.clone()),
      IdMatch::Ambiguous => ambiguous.push(id.clone()),
    }
  }
  let (code, message, commits) = if !ambiguous.is_empty() {
    (ErrorCode::InvalidArguments, "Ambiguous commit ids", ambiguous)
  } else if !unknown.is_empty() {
    (ErrorCode::NotAheadOfBaseline, "Not commits ahead of the baseline", unknown)
  } else {
    return Ok(resolved);
  };
  let mut error = CliError::new(code, format!("{message}: {}", commits.join(", ")));
  error.commits = commits;
  Err(error)
}

/// What `bd commits` and `bd commits --brief` both read: HEAD, the grouping, the changes and the fixup targets
struct Reading {
  head: String,
  grouping: BranchGrouping,
  scope: Scope,
  diffs: LinkedDiffs,
  /// Fixup commit id -> its kind and its target, `None` for an orphan
  fixups: HashMap<String, (FixupKind, Option<String>)>,
}

impl Reading {
  fn read(context: &RepoContext, args: &CommitsArgs) -> Result<Self, CliError> {
    let (head, _) = read_head(context)?;
    let grouping = read_grouping(context, args.branch.as_deref())?;
    let scope = Scope::of(args, &grouping.commit_ids)?;
    // the dependencies need every commit, whatever the scope
    let diffs = read_linked_diffs(context, &grouping.commit_ids, args.include_generated, args.files)?;
    let fixups = fixup_targets(&head, &ordered_commits(&grouping), &grouping.grouping);
    Ok(Self {
      head,
      grouping,
      scope,
      diffs,
      fixups,
    })
  }

  fn header(&self, context: &RepoContext, short: &ShortIds) -> CommitsHeader {
    CommitsHeader {
      schema: SchemaVersion,
      repository: context.repository.clone(),
      prefix: context.prefix.clone(),
      head: short.get(&self.head).to_string(),
      baseline: self.grouping.baseline_branch.clone(),
    }
  }

  fn short_ids(&self, context: &RepoContext) -> ShortIds {
    context.short_ids(self.grouping.commit_ids.iter().map(String::as_str).chain(std::iter::once(self.head.as_str())))
  }

  /// The dependency edges over every commit, positions in `grouping.commit_ids`
  fn dependencies(&self) -> Vec<Dependency> {
    compute_dependencies(self.diffs.linked())
  }

  /// Whether the output lists an edge between these positions
  fn edge_in_scope(&self, edge: &Dependency, group_of: &HashMap<&str, Option<&str>>) -> bool {
    [edge.from, edge.to].iter().any(|&index| {
      let id = self.grouping.commit_ids[index].as_str();
      self.scope.contains(id, group_of.get(id).copied().flatten())
    })
  }

  fn fixup_kind_and_target(&self, commit: &Commit, short: &ShortIds) -> Option<(FixupMarker, Option<String>)> {
    self.fixups.get(commit.id.as_str()).map(|(kind, target)| {
      let kind = match kind {
        FixupKind::Fixup => FixupMarker::Fixup,
        FixupKind::Squash => FixupMarker::Squash,
        FixupKind::Amend => FixupMarker::Amend,
      };
      (kind, target.as_deref().map(|target| short.get(target).to_string()))
    })
  }
}

/// Every commit ahead of the baseline, oldest first, with its group
fn ordered_commits(grouping: &BranchGrouping) -> Vec<(&Commit, Option<&str>)> {
  let mut by_id: HashMap<&str, (&Commit, Option<&str>)> = HashMap::new();
  for branch in &grouping.branches {
    for commit in &branch.commits {
      by_id.insert(&commit.id, (commit, Some(&branch.name)));
    }
  }
  for commit in &grouping.unassigned {
    by_id.insert(&commit.id, (commit, None));
  }
  grouping.commit_ids.iter().filter_map(|id| by_id.get(id.as_str()).copied()).collect()
}

fn group_by_id<'g>(ordered: &[(&'g Commit, Option<&'g str>)]) -> HashMap<&'g str, Option<&'g str>> {
  ordered.iter().map(|(commit, group)| (commit.id.as_str(), *group)).collect()
}

pub fn commits(context: &RepoContext, args: &CommitsArgs) -> Result<CommitsOutput, CliError> {
  let reading = Reading::read(context, args)?;
  let ordered = ordered_commits(&reading.grouping);
  let group_of = group_by_id(&ordered);
  let listed: Vec<(&Commit, Option<&str>)> = ordered.iter().copied().filter(|(commit, group)| reading.scope.contains(&commit.id, *group)).collect();

  let mut patches: HashMap<String, String> = if args.patch {
    let ids: Vec<String> = listed.iter().map(|(commit, _)| commit.id.clone()).collect();
    read_commit_patches(&context.git, &context.repository, &ids)?.into_iter().collect()
  } else {
    HashMap::new()
  };
  let short = reading.short_ids(context);
  let diffs = &reading.diffs.diffs;
  let diff_of: HashMap<&str, &CommitDiff> = diffs.iter().map(|diff| (diff.id.as_str(), diff)).collect();

  let commits = listed
    .iter()
    .map(|&(commit, group)| {
      let parsed = parse_subject(&commit.subject);
      CommitEntry {
        id: short.get(&commit.id).to_string(),
        subject: commit.subject.clone(),
        body: commit.message.strip_prefix(commit.subject.as_str()).unwrap_or_default().trim().to_string(),
        author: format!("{} <{}>", commit.author_name, commit.author_email),
        time: commit.author_timestamp,
        group: group.map(str::to_string),
        issue_key: find_issue_number(parsed.rest).map(str::to_string),
        fixup: reading.fixup_kind_and_target(commit, &short).map(|(kind, target)| FixupTarget {
          kind,
          target,
          target_subject: commit.subject[parsed.autosquash.len()..].to_string(),
        }),
        files: args.files.then(|| {
          diff_of.get(commit.id.as_str()).map_or_else(Vec::new, |diff| {
            diff.files.iter().map(|change| file_entry(change, reading.diffs.is_generated(change))).collect()
          })
        }),
        patch: patches.remove(&commit.id),
      }
    })
    .collect();

  let dependencies = reading
    .dependencies()
    .into_iter()
    .filter(|edge| reading.edge_in_scope(edge, &group_of))
    .map(|edge| DependencyEdge {
      from: short.get(&diffs[edge.from].id).to_string(),
      to: short.get(&diffs[edge.to].id).to_string(),
      files: edge.files,
    })
    .collect();

  // the overlaps of the branches the listed commits are in
  let listed_groups: HashSet<&str> = listed.iter().filter_map(|(_, group)| *group).collect();
  let cluster_overlap = cluster_overlap(reading.diffs.linked(), |id| group_of.get(id).copied().flatten())
    .into_iter()
    .filter(|overlap| matches!(reading.scope, Scope::All) || overlap.branches.iter().any(|name| listed_groups.contains(name.as_str())))
    .collect();

  Ok(CommitsOutput {
    header: reading.header(context, &short),
    commits,
    dependencies,
    cluster_overlap,
  })
}

pub fn commits_brief(context: &RepoContext, args: &CommitsArgs) -> Result<CommitsBrief, CliError> {
  let reading = Reading::read(context, args)?;
  let ordered = ordered_commits(&reading.grouping);
  let group_of = group_by_id(&ordered);
  let short = reading.short_ids(context);
  let commit_ids = &reading.grouping.commit_ids;
  let position_of: HashMap<&str, usize> = commit_ids.iter().enumerate().map(|(index, id)| (id.as_str(), index)).collect();
  let linked = reading.diffs.linked();

  let commits = ordered
    .iter()
    .filter(|(commit, group)| reading.scope.contains(&commit.id, *group))
    .map(|&(commit, group)| BriefCommit {
      id: short.get(&commit.id).to_string(),
      subject: brief_subject(&commit.subject, group),
      group: group.map(str::to_string),
      issue_key: find_issue_number(parse_subject(&commit.subject).rest).filter(|key| Some(*key) != group).map(str::to_string),
      fixup: reading.fixup_kind_and_target(commit, &short).map(|(kind, target)| BriefFixup { kind, target }),
      areas: position_of
        .get(commit.id.as_str())
        .and_then(|&index| linked.get(index))
        .map_or_else(Vec::new, |diff| top_areas(&diff.files, args.depth)),
    })
    .collect();

  let edges = reading.dependencies();
  let fixup_edges = reading.fixups.iter().filter_map(|(fixup, (_, target))| {
    let target = target.as_deref()?;
    Some((*position_of.get(fixup.as_str())?, *position_of.get(target)?))
  });
  let in_scope = |index: usize| {
    let id = commit_ids[index].as_str();
    reading.scope.contains(id, group_of.get(id).copied().flatten())
  };
  let units = connected_components(commit_ids.len(), edges.iter().map(|edge| (edge.from, edge.to)).chain(fixup_edges))
    .into_iter()
    .filter(|unit| unit.iter().any(|&index| in_scope(index)))
    .map(|unit| unit.into_iter().map(|index| short.get(&commit_ids[index]).to_string()).collect())
    .collect();
  let dependencies = edges
    .iter()
    .filter(|edge| reading.edge_in_scope(edge, &group_of))
    .map(|edge| BriefEdge {
      from: short.get(&commit_ids[edge.from]).to_string(),
      to: short.get(&commit_ids[edge.to]).to_string(),
    })
    .collect();

  Ok(CommitsBrief {
    header: reading.header(context, &short),
    commits,
    dependencies,
    units,
  })
}

/// The subject without its prefix when the prefix is exactly `group`: a stack path (`group` is its leaf) or a prefix
/// the grouper sanitizes (`(Foo Bar)` is in `Foo-Bar`) stays, so that a plan made from the brief view rewrites neither
fn brief_subject(subject: &str, group: Option<&str>) -> String {
  if group.is_some() && parse_subject(subject).prefix == group {
    with_branch_prefix(subject, None)
  } else {
    subject.to_string()
  }
}

/// Fixup commit id -> its kind and the target `plan_autosquash` resolves, `None` for an orphan
pub(crate) fn fixup_targets(head: &str, ordered: &[(&Commit, Option<&str>)], grouping: &GroupingInputs) -> HashMap<String, (FixupKind, Option<String>)> {
  let commits: Vec<Commit> = ordered.iter().map(|(commit, _)| (*commit).clone()).collect();
  let plan = plan_autosquash(head.to_string(), &commits, grouping);
  let mut targets: HashMap<String, (FixupKind, Option<String>)> = plan
    .groups
    .into_iter()
    .flat_map(|group| group.folded.into_iter().map(move |fixup| (fixup.commit_id, (fixup.kind, Some(group.target.clone())))))
    .collect();
  for orphan in plan.orphans {
    // the first marker decides the kind, in the order of `AUTOSQUASH_MARKERS`
    let kind = AUTOSQUASH_MARKERS
      .iter()
      .position(|marker| orphan.subject.starts_with(marker))
      .map(|index| [FixupKind::Fixup, FixupKind::Squash, FixupKind::Amend][index]);
    if let Some(kind) = kind {
      targets.insert(orphan.commit_id, (kind, None));
    }
  }
  targets
}

fn file_entry(change: &FileChange, generated: bool) -> FileEntry {
  FileEntry {
    path: change.path.clone(),
    old_path: change.old_path.clone(),
    status: change.status.to_string(),
    additions: (!change.binary).then_some(change.additions),
    deletions: (!change.binary).then_some(change.deletions),
    binary: change.binary,
    generated,
  }
}

/// For each pair of groups that change a common file, the common files, ordered by the branch names
fn cluster_overlap<'d, 'g>(diffs: &'d [CommitDiff], group_of: impl Fn(&str) -> Option<&'g str>) -> Vec<ClusterOverlap> {
  let mut files_of: BTreeMap<&'g str, BTreeSet<&'d str>> = BTreeMap::new();
  for diff in diffs {
    let Some(group) = group_of(&diff.id) else { continue };
    let files = files_of.entry(group).or_default();
    for change in &diff.files {
      files.insert(&change.path);
      files.extend(change.old_path.as_deref());
    }
  }
  let groups: Vec<(&str, &BTreeSet<&str>)> = files_of.iter().map(|(group, files)| (*group, files)).collect();
  let mut overlaps = Vec::new();
  for (index, (first, first_files)) in groups.iter().enumerate() {
    for (second, second_files) in &groups[index + 1..] {
      let shared: Vec<String> = first_files.intersection(second_files).map(|file| (*file).to_string()).collect();
      if !shared.is_empty() {
        overlaps.push(ClusterOverlap {
          branches: [(*first).to_string(), (*second).to_string()],
          files: shared,
        });
      }
    }
  }
  overlaps
}
