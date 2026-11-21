//! The generated files the words merged in the copies of a sync that the tips of the branches still hold
//! (docs/design/git-operations.md, "Resolve by words"): a copy of a sync resolves a conflict in a generated file by
//! words, so the copy a branch pushes and lands holds a merge of words, not the output of its generator. Of each built
//! virtual branch, with the copies of the branches it is built on (its history holds them, and its pull request lands
//! them), and of each stack branch at its build, the merges its tip still holds as the words wrote them
//! (`git_ops::generated::held_word_merges`), each with the commit below it that the copy lacks: the last commit ahead of
//! the baseline before it that changes the file, in no branch of the tip (`git_ops::generated::commits_changing`). One
//! phase for the sync, not one per branch: one `cat-file --batch-check` when a merge lies below its tip, one `log` when
//! one is held; none without merges.

use anyhow::Result;
use domain_types::conflict::RegenerateFile;
use domain_types::landing::StackCommit;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::generated::{CopyMerge, commits_changing, held_word_merges};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use sync_utils::commit_subject::parse_subject;
use tracing::instrument;

/// A tip of a sync and the merges of the copies of its history
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldTip {
  pub tip: String,
  /// The generated files the words merged in the copies of its history, the oldest copy first
  pub merges: Vec<CopyMerge>,
  /// The branches whose commits its history holds: the branch and those it is built on, or the members of a stack
  pub members: BTreeSet<String>,
}

/// An original commit ahead of the baseline: its branch (empty for an unassigned commit) and its subject as written
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OriginalCommit {
  pub branch: String,
  pub subject: String,
}

/// What a tip of `tips` must regenerate, in their order: of each, the merges it still holds, by path, each with the commit
/// its copy lacks. `originals`: the commits ahead of the baseline (`head` the newest, `baseline` excluded), by id. One
/// `cat-file --batch-check` when a merge lies below its tip, one `log` when one is held; none without merges
#[instrument(skip_all, fields(tips = tips.len()))]
pub fn regenerate_files(
  git: &GitCommandExecutor,
  repository: &str,
  head: &str,
  baseline: &str,
  tips: &[HeldTip],
  originals: &HashMap<String, OriginalCommit>,
) -> Result<Vec<Vec<RegenerateFile>>> {
  if tips.iter().all(|tip| tip.merges.is_empty()) {
    return Ok(vec![Vec::new(); tips.len()]);
  }
  let inputs: Vec<(String, Vec<CopyMerge>)> = tips.iter().map(|tip| (tip.tip.clone(), tip.merges.clone())).collect();
  let held = held_word_merges(git, repository, &inputs)?;
  let paths: Vec<String> = held.iter().flatten().map(|merge| merge.file.path.clone()).collect::<BTreeSet<_>>().into_iter().collect();
  let changing = commits_changing(git, repository, head, baseline, &paths)?;
  let stack_commit = |id: &str, subject: &str| {
    let original = originals.get(id);
    StackCommit {
      id: id.to_string(),
      subject: without_prefix(original.map_or(subject, |original| original.subject.as_str())),
      branch: original.map_or_default(|original| original.branch.clone()),
    }
  };
  Ok(
    tips
      .iter()
      .zip(held)
      .map(|(tip, merges)| {
        merges
          .into_iter()
          .map(|merge| {
            // newest first: the commits after the original that change the file and lie in no branch of the tip
            let below = changing.iter().skip_while(|commit| commit.hash != merge.commit).skip(1);
            let lacks = below
              .filter(|commit| commit.files_touched.contains(&merge.file.path))
              .find(|commit| originals.get(&commit.hash).is_none_or(|original| !tip.members.contains(&original.branch)))
              .map(|commit| stack_commit(&commit.hash, &commit.subject));
            RegenerateFile {
              commit: stack_commit(&merge.commit, ""),
              path: merge.file.path,
              generated_by: merge.file.generated_by,
              lacks,
            }
          })
          .collect()
      })
      .collect(),
  )
}

/// The subject without its prefix, an autosquash marker kept: as `StackCommit` names a commit
fn without_prefix(subject: &str) -> String {
  let parsed = parse_subject(subject);
  format!("{}{}", parsed.autosquash, parsed.rest)
}

/// The branch and those it is built on, the root first (`parents`: branch -> the branch it is built on)
pub fn chain_of(branch: &str, parents: &HashMap<String, String>) -> Vec<String> {
  let mut chain = vec![branch.to_string()];
  let mut seen: HashSet<&str> = HashSet::from([branch]);
  let mut current = branch;
  while let Some(parent) = parents.get(current) {
    if !seen.insert(parent) {
      break;
    }
    chain.push(parent.clone());
    current = parent;
  }
  chain.reverse();
  chain
}

/// The facts of a sync, by key: of the virtual branches (short name) and of the stack branches (root); a tip without a
/// held merge is left out
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RegenerateFacts {
  pub branches: BTreeMap<String, Vec<RegenerateFile>>,
  pub stacks: BTreeMap<String, Vec<RegenerateFile>>,
}
