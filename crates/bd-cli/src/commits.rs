use crate::context::RepoContext;
use crate::dependencies::compute_dependencies;
use crate::diff_scan::{CommitDiff, read_commit_diffs_cached, read_commit_patches};
use crate::error::CliError;
use crate::generated::generated_paths;
use crate::output::SchemaVersion;
use crate::status::{read_grouping, read_head};
use git_ops::commit_list::Commit;
use object_cache::commit_diff::FileChange;
use schemars::JsonSchema;
use serde::Serialize;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use sync_core::autosquash::{FixupKind, plan_autosquash};
use sync_core::commit_grouper::{AUTOSQUASH_MARKERS, parse_subject};
use sync_utils::issue_pattern::find_issue_number;

#[derive(Debug, Default, clap::Args)]
pub struct CommitsArgs {
  /// The changed files of each commit: status, path, added and deleted lines
  #[arg(long)]
  pub files: bool,
  /// The patch of each commit, with three lines of context
  #[arg(long)]
  pub patch: bool,
  /// Only the commits of this branch, and the edges and overlaps that involve it
  #[arg(long)]
  pub branch: Option<String>,
  /// Keep lock files and generated files in the dependencies and the overlaps
  #[arg(long)]
  pub include_generated: bool,
}

/// `bd commits`: every commit ahead of the baseline, oldest first, with what ties them together
#[derive(Debug, Serialize, JsonSchema)]
pub struct CommitsOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  pub head: String,
  pub baseline: String,
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
  #[serde(skip_serializing_if = "String::is_empty")]
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
  #[serde(skip_serializing_if = "std::ops::Not::not")]
  pub binary: bool,
  /// A lock file, or marked `linguist-generated` or `-diff` in the attributes
  #[serde(skip_serializing_if = "std::ops::Not::not")]
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

pub fn commits(context: &RepoContext, args: &CommitsArgs) -> Result<CommitsOutput, CliError> {
  let (head, _) = read_head(context)?;
  let grouping = read_grouping(context, args.branch.as_deref())?;

  // oldest first, with their groups
  let mut by_id: HashMap<&str, (&Commit, Option<&str>)> = HashMap::new();
  for branch in &grouping.branches {
    for commit in &branch.commits {
      by_id.insert(&commit.id, (commit, Some(&branch.name)));
    }
  }
  for commit in &grouping.unassigned {
    by_id.insert(&commit.id, (commit, None));
  }
  let ordered: Vec<(&Commit, Option<&str>)> = grouping.commit_ids.iter().filter_map(|id| by_id.get(id.as_str()).copied()).collect();
  let in_scope = |group: Option<&str>| args.branch.as_deref().is_none_or(|name| group == Some(name));

  // the dependencies need every commit, whatever the scope
  let diffs = read_commit_diffs_cached(&context.git, &context.repository, &context.cache(), &grouping.commit_ids)?;
  let mut patches: HashMap<String, String> = if args.patch {
    let ids: Vec<String> = ordered.iter().filter(|(_, group)| in_scope(*group)).map(|(commit, _)| commit.id.clone()).collect();
    read_commit_patches(&context.git, &context.repository, &ids)?.into_iter().collect()
  } else {
    HashMap::new()
  };
  let fixups = fixup_targets(&head, &ordered);
  let generated = if args.files || !args.include_generated {
    let paths = diffs
      .iter()
      .flat_map(|diff| &diff.files)
      .flat_map(|change| std::iter::once(change.path.as_str()).chain(change.old_path.as_deref()));
    generated_paths(&context.git, &context.repository, paths)?
  } else {
    HashSet::new()
  };
  let is_generated = |change: &FileChange| generated.contains(&change.path) || change.old_path.as_ref().is_some_and(|path| generated.contains(path));
  // the same positions as `diffs`, without the generated files
  let linked_diffs: Cow<[CommitDiff]> = if args.include_generated {
    Cow::Borrowed(&diffs)
  } else {
    let diffs = diffs.iter().map(|diff| CommitDiff {
      id: diff.id.clone(),
      files: diff.files.iter().filter(|change| !is_generated(change)).cloned().collect(),
    });
    Cow::Owned(diffs.collect())
  };

  let short = context.short_ids(grouping.commit_ids.iter().map(String::as_str).chain(std::iter::once(head.as_str())));
  let diff_of: HashMap<&str, &CommitDiff> = diffs.iter().map(|diff| (diff.id.as_str(), diff)).collect();
  let group_of = |index: usize| diffs.get(index).and_then(|diff| by_id.get(diff.id.as_str())).and_then(|(_, group)| *group);

  let commits = ordered
    .iter()
    .filter(|(_, group)| in_scope(*group))
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
        fixup: fixups.get(commit.id.as_str()).map(|(kind, target)| FixupTarget {
          kind: match kind {
            FixupKind::Fixup => FixupMarker::Fixup,
            FixupKind::Squash => FixupMarker::Squash,
            FixupKind::Amend => FixupMarker::Amend,
          },
          target: target.as_deref().map(|target| short.get(target).to_string()),
          target_subject: commit.subject[parsed.autosquash.len()..].to_string(),
        }),
        files: args.files.then(|| {
          diff_of
            .get(commit.id.as_str())
            .map_or_else(Vec::new, |diff| diff.files.iter().map(|change| file_entry(change, is_generated(change))).collect())
        }),
        patch: patches.remove(&commit.id),
      }
    })
    .collect();

  let dependencies = compute_dependencies(&linked_diffs)
    .into_iter()
    .filter(|edge| in_scope(group_of(edge.from)) || in_scope(group_of(edge.to)))
    .map(|edge| DependencyEdge {
      from: short.get(&diffs[edge.from].id).to_string(),
      to: short.get(&diffs[edge.to].id).to_string(),
      files: edge.files,
    })
    .collect();

  let cluster_overlap = cluster_overlap(&linked_diffs, |id| by_id.get(id).and_then(|(_, group)| *group))
    .into_iter()
    .filter(|overlap| args.branch.as_ref().is_none_or(|name| overlap.branches.contains(name)))
    .collect();

  Ok(CommitsOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    head: short.get(&head).to_string(),
    baseline: grouping.baseline_branch.clone(),
    commits,
    dependencies,
    cluster_overlap,
  })
}

/// Fixup commit id -> its kind and the target `plan_autosquash` resolves, `None` for an orphan
fn fixup_targets(head: &str, ordered: &[(&Commit, Option<&str>)]) -> HashMap<String, (FixupKind, Option<String>)> {
  let commits: Vec<Commit> = ordered.iter().map(|(commit, _)| (*commit).clone()).collect();
  // the CLI groups without stacked branches so far
  let plan = plan_autosquash(head.to_string(), &commits, false);
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
