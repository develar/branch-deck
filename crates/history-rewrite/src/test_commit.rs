//! The commits, the regroup plans and the mapping notes of the unit tests of the crates that build on this one

use domain_types::commit::{Commit, NoteSource};
use domain_types::rewrite::RegroupPlan;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::notes::{MappingNote, write_commit_notes};
use std::collections::BTreeMap;
use sync_utils::commit_subject::subject_parts;

/// A commit as the commit list reads it, with fixed metadata; the subject is the first line of `message`
pub fn commit(id: &str, message: &str) -> Commit {
  let subject = message.split('\n').next().unwrap().to_string();
  Commit {
    subject_parts: subject_parts(&subject),
    id: id.to_string(),
    stripped_subject: subject.clone(),
    message: message.to_string(),
    author_name: "Test Author".to_string(),
    author_email: "test@example.com".to_string(),
    author_timestamp: 1_234_567_890,
    committer_timestamp: 1_234_567_890,
    author_timezone: String::new(),
    committer_timezone: String::new(),
    subject,
    parent_id: None,
    tree_id: "tree".to_string(),
    note: None,
    mapped_commit_id: None,
    mapped_source: None,
    mapped_resolution: None,
  }
}

/// A plan of `branches` (path, commits) and `unassign`
#[must_use]
pub fn plan(branches: &[(&str, &[&String])], unassign: &[&String]) -> RegroupPlan {
  RegroupPlan {
    branches: branches
      .iter()
      .map(|(name, ids)| ((*name).to_string(), ids.iter().map(|id| (*id).clone()).collect()))
      .collect::<BTreeMap<_, _>>(),
    unassign: unassign.iter().map(|id| (*id).clone()).collect(),
    ..RegroupPlan::default()
  }
}

/// A plan that puts the commits of `keys` into the issue group `name`
#[must_use]
pub fn issue_group_plan(name: &str, keys: &[&str]) -> RegroupPlan {
  RegroupPlan {
    issue_groups: BTreeMap::from([(name.to_string(), keys.iter().map(|key| (*key).to_string()).collect())]),
    ..RegroupPlan::default()
  }
}

/// A mapping note on each of `commits`, as a sync leaves them: its source the trees of the commit and of its first
/// parent, its copy a made-up id; one `rev-parse`, one `fast-import`. A rewrite that keeps those trees carries them
///
/// # Panics
/// The trees not read, the notes not written
pub fn write_mapping_notes(git_executor: &GitCommandExecutor, repo_path: &str, commits: &[&str]) {
  let revs: Vec<String> = commits.iter().flat_map(|commit| [format!("{commit}^{{tree}}"), format!("{commit}^^{{tree}}")]).collect();
  let mut args = vec!["rev-parse"];
  args.extend(revs.iter().map(String::as_str));
  let trees = git_executor.execute_command_lines(&args, repo_path).unwrap();
  let notes: Vec<MappingNote> = commits
    .iter()
    .zip(trees.chunks(2))
    .zip(1u32..)
    .map(|((commit, trees), copy)| MappingNote {
      original: (*commit).to_string(),
      mapped: format!("{copy:040x}"),
      source: NoteSource {
        tree: trees[0].clone(),
        parent_tree: trees[1].clone(),
      },
      resolved: None,
    })
    .collect();
  write_commit_notes(git_executor, repo_path, &notes).unwrap();
}
