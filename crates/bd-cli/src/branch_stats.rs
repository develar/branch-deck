//! `bd status --stats`: what each branch changes, from at most three processes for all branches: the commit diffs the
//! cache misses (one `log -p -U0`), one `diff-tree --stdin` and one `check-attr`

use crate::areas::DirLines;
use crate::context::RepoContext;
use crate::error::CliError;
use crate::generated::generated_paths;
use crate::numstat::{NumstatEntry, read_tree_numstats};
use crate::output::ShortIds;
use git_ops::commit_list::Commit;
use schemars::JsonSchema;
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::{BTreeSet, HashMap, HashSet};
use sync_core::commit_grouper::parse_subject;
use sync_core::diff_scan::read_commit_diffs_cached;
use sync_utils::issue_pattern::find_issue_number;

/// The number of directories in `top_dirs`
const TOP_DIRS: usize = 3;

#[derive(Debug, Serialize, JsonSchema)]
pub struct BranchStats {
  /// The distinct files its commits change
  pub files: usize,
  /// How many of `files` are lock files or generated files
  #[serde(default, skip_serializing_if = "is_zero")]
  pub generated_files: usize,
  /// Added lines summed over the commits; lock files and generated files don't count without --include-generated
  pub additions: u32,
  pub deletions: u32,
  /// The directories with the most changed lines, at most three
  pub top_dirs: Vec<DirLines>,
  /// Most commits first
  pub authors: Vec<AuthorCommits>,
  /// The oldest author time of its commits, Unix seconds
  pub first_time: u32,
  /// The newest author time of its commits
  pub last_time: u32,
  /// The distinct issue keys of its subjects, oldest first
  pub issue_keys: Vec<String>,
  /// The synced virtual branch against the base the sync builds on; `null` if the branch was never synced
  pub diffstat: Option<Diffstat>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AuthorCommits {
  /// `Name <email>`
  pub author: String,
  pub commits: usize,
}

/// The net change of a virtual branch: `git diff --numstat <base> <tip>`
#[derive(Debug, Serialize, JsonSchema)]
pub struct Diffstat {
  /// The parent of the oldest commit ahead of the baseline, which the sync rebuilds every branch on. Exact while the tip
  /// is synced with the current commits
  pub base: String,
  pub files: usize,
  pub additions: u32,
  pub deletions: u32,
}

#[expect(clippy::trivially_copy_pass_by_ref, reason = "the signature serde's skip_serializing_if needs")]
fn is_zero(value: &usize) -> bool {
  *value == 0
}

/// A branch to compute the stats of
pub struct BranchInput<'a> {
  pub name: &'a str,
  pub commits: &'a [Commit],
  /// The full id of the synced virtual branch
  pub tip: Option<&'a str>,
}

/// The stats by branch name
pub fn branch_stats(
  context: &RepoContext,
  base: Option<&str>,
  branches: &[BranchInput<'_>],
  include_generated: bool,
  short: &ShortIds,
) -> Result<HashMap<String, BranchStats>, CliError> {
  let ids: Vec<String> = branches.iter().flat_map(|branch| branch.commits).map(|commit| commit.id.clone()).collect();
  // the per-commit changes `bd commits` caches; a miss costs one `-U0` patch pass for all missing commits
  let numstats: HashMap<String, Vec<NumstatEntry>> = read_commit_diffs_cached(&context.git, &context.repository, &context.cache(), &ids)?
    .into_iter()
    .map(|diff| (diff.id, diff.files.iter().map(NumstatEntry::from).collect()))
    .collect();
  let tips: Vec<&str> = branches.iter().filter_map(|branch| branch.tip).collect();
  let diffstats = match base {
    Some(base) => read_tree_numstats(&context.git, &context.repository, base, &tips)?,
    None => HashMap::new(),
  };
  let paths = numstats.values().chain(diffstats.values()).flatten().map(|entry| entry.path.as_str());
  let generated = generated_paths(&context.git, &context.repository, paths)?;
  let counted = |entry: &NumstatEntry| include_generated || !generated.contains(&entry.path);

  let mut result = HashMap::new();
  for branch in branches {
    let entries: Vec<&NumstatEntry> = branch.commits.iter().filter_map(|commit| numstats.get(&commit.id)).flatten().collect();
    let files: HashSet<&str> = entries.iter().map(|entry| entry.path.as_str()).collect();
    let mut additions = 0;
    let mut deletions = 0;
    let mut dir_lines: HashMap<&str, u32> = HashMap::new();
    for entry in entries.iter().filter(|entry| counted(entry)) {
      let (added, deleted) = (entry.additions.unwrap_or(0), entry.deletions.unwrap_or(0));
      additions += added;
      deletions += deleted;
      let dir = entry.path.rsplit_once('/').map_or(".", |(dir, _)| dir);
      *dir_lines.entry(dir).or_default() += added + deleted;
    }
    let mut top_dirs: Vec<(&str, u32)> = dir_lines.into_iter().filter(|(_, lines)| *lines > 0).collect();
    top_dirs.sort_by_key(|&(dir, lines)| (Reverse(lines), dir));
    top_dirs.truncate(TOP_DIRS);

    let mut author_commits: HashMap<String, usize> = HashMap::new();
    for commit in branch.commits {
      *author_commits.entry(format!("{} <{}>", commit.author_name, commit.author_email)).or_default() += 1;
    }
    let mut authors: Vec<AuthorCommits> = author_commits.into_iter().map(|(author, commits)| AuthorCommits { author, commits }).collect();
    authors.sort_by(|a, b| b.commits.cmp(&a.commits).then_with(|| a.author.cmp(&b.author)));

    // the commits of a branch are newest first
    let mut seen_keys = BTreeSet::new();
    let issue_keys = branch
      .commits
      .iter()
      .rev()
      .filter_map(|commit| find_issue_number(parse_subject(&commit.subject).rest))
      .filter(|key| seen_keys.insert(*key))
      .map(str::to_string)
      .collect();

    let diffstat = base.zip(branch.tip).map(|(base, tip)| {
      let entries = diffstats.get(tip).map_or(&[][..], Vec::as_slice);
      let counted_entries = entries.iter().filter(|entry| counted(entry));
      Diffstat {
        base: short.get(base).to_string(),
        files: entries.len(),
        additions: counted_entries.clone().filter_map(|entry| entry.additions).sum(),
        deletions: counted_entries.filter_map(|entry| entry.deletions).sum(),
      }
    });

    result.insert(
      branch.name.to_string(),
      BranchStats {
        generated_files: files.iter().filter(|path| generated.contains(**path)).count(),
        files: files.len(),
        additions,
        deletions,
        top_dirs: top_dirs.into_iter().map(|(dir, lines)| DirLines { dir: dir.to_string(), lines }).collect(),
        authors,
        first_time: branch.commits.iter().map(|commit| commit.author_timestamp).min().unwrap_or(0),
        last_time: branch.commits.iter().map(|commit| commit.author_timestamp).max().unwrap_or(0),
        issue_keys,
        diffstat,
      },
    );
  }
  Ok(result)
}
