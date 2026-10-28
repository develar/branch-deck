//! The group of each commit ahead of the baseline (`CommitGrouper`): its `(prefix)`, else the issue group of its issue
//! key, else the key itself, else none; a pure function of the subjects and the `GroupingInputs`, no git. The grammar
//! of a subject (the autosquash markers, the `(prefix)`) is `sync_utils::commit_subject`.

use crate::stack::{StackDeclarations, StackForest};
use crate::store_lines::StoreLines;
use domain_types::branch_names::sanitize_branch_name;
use domain_types::commit::{Commit, CommitGroup, GroupOrigin};
use domain_types::stack::{ParentSource, leaf_of, stack_path};
use indexmap::IndexMap;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use sync_utils::commit_subject::{ParsedSubject, parse_subject, subject_after_prefix};
use sync_utils::issue_pattern::find_issue_number;
use tracing::info;

/// Branch data combining commits and author frequency tracking
#[derive(Debug)]
struct BranchData {
  commits: Vec<Commit>,
  /// Author email -> commits, in the order the authors are first seen
  author_frequencies: IndexMap<String, u32>,
  /// Position of the oldest commit among all grouped commits
  first_position: usize,
}

impl BranchData {
  fn new(first_position: usize) -> Self {
    Self {
      commits: Vec::new(),
      author_frequencies: IndexMap::new(),
      first_position,
    }
  }

  fn add_commit(&mut self, commit: Commit) {
    // Track author frequency
    if !commit.author_email.is_empty() {
      *self.author_frequencies.entry(commit.author_email.clone()).or_insert(0) += 1;
    }

    self.commits.push(commit);
  }

  /// The email of the most commits; of a tie the author seen first, so a regrouping of the same commits gives the same
  /// email (it keys the remote status of the branch)
  fn most_frequent_author(&self) -> Option<String> {
    let mut authors = self.author_frequencies.iter();
    let first = authors.next()?;
    let (email, _) = authors.fold(first, |best, author| if author.1 > best.1 { author } else { best });
    Some(email.clone())
  }
}

/// Result of grouping commits by prefix
pub struct GroupedCommits {
  /// Branch name -> commits (oldest first), in first-seen order
  pub branches: IndexMap<String, Vec<Commit>>,
  /// Commits without a prefix (oldest first)
  pub unassigned: Vec<Commit>,
  /// Branch name -> most frequent author email
  pub branch_emails: HashMap<String, Option<String>>,
  /// The keys of the issue groups that group none of these commits, sorted: idle here, they may group the commits of
  /// another branch or worktree
  pub idle_issue_groups: Vec<String>,
  /// With stacked branches on, the children of the stack parents that no branch here reaches, sorted: idle here, they
  /// may stack a branch of another worktree
  pub idle_stack_parents: Vec<String>,
}

/// What decides the group of a commit besides its subject (`snapshot::read_grouping_inputs`)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GroupingInputs {
  /// Stacked branches are on (`stack::stacks_enabled`)
  pub stacks: bool,
  /// Issue key -> branch name (`issue_groups::ISSUE_GROUPS`)
  pub issue_groups: StoreLines,
  /// Child -> parent (`stack_parents::STACK_PARENTS`); read with stacked branches off too, and not used then
  pub stack_parents: StoreLines,
  /// Branch -> note (`parked::PARKED`): read with the other maps, decides no group
  pub parked: StoreLines,
}

impl GroupingInputs {
  /// Without issue groups, stack parents and parked branches
  pub fn with_stacks(stacks: bool) -> Self {
    Self { stacks, ..Self::default() }
  }

  /// The branch a commit with this subject is grouped into: the prefix (its leaf with stacked branches on), else the
  /// issue group of the subject's issue key, else the key; `None` for an unassigned commit
  pub fn branch_of(&self, subject: &str) -> Option<String> {
    let parsed = parse_subject(subject);
    match parsed.prefix {
      Some(prefix) if self.stacks => leaf_of(prefix),
      _ => branch_of(&parsed, &self.issue_groups.entries),
    }
  }
}

/// The virtual branch a commit with this subject is grouped into: its sanitized `(prefix)`, else the issue key of the
/// subject; `None` for an unassigned commit
pub fn branch_name_of(subject: &str) -> Option<String> {
  branch_of(&parse_subject(subject), &BTreeMap::new())
}

/// `branch_name_of` with stacked branches on: the leaf of a stack prefix, so `(auth/login) X` is in `login`
pub fn stack_branch_name_of(subject: &str) -> Option<String> {
  GroupingInputs::with_stacks(true).branch_of(subject)
}

/// The flat rule: the sanitized prefix, else the issue group of the issue key, else the key
fn branch_of(parsed: &ParsedSubject<'_>, issue_groups: &BTreeMap<String, String>) -> Option<String> {
  match parsed.prefix {
    Some(prefix) => Some(sanitize_branch_name(prefix.trim())),
    None => find_issue_number(parsed.rest).map(|key| issue_groups.get(key).map_or(key, String::as_str).to_owned()),
  }
}

/// The group of a commit with this subject, as `CommitGrouper::add_commit` puts it, with where its parent comes from: the
/// preview of a typed message or branch name (`preview_group`). With stacked branches, `(auth/login)` is `login` on
/// `auth`, a branch whose prefix names no parent takes its stack parent, and an empty level (`(a//b)`) groups into its
/// last level, `invalid`; `None` for an unassigned commit and for `(/)`
pub fn group_of(subject: &str, inputs: &GroupingInputs) -> Option<CommitGroup> {
  let parsed = parse_subject(subject);
  let group = |branch: String, origin: GroupOrigin| CommitGroup {
    branch,
    origin,
    parent: None,
    parent_source: None,
    invalid: false,
  };
  let stored = |mut group: CommitGroup| {
    if let Some(parent) = inputs.stack_parents.entries.get(&group.branch) {
      group.parent = Some(parent.clone());
      group.parent_source = Some(ParentSource::Store);
    }
    group
  };
  match parsed.prefix {
    Some(prefix) if inputs.stacks => match stack_path(prefix) {
      Ok(mut path) => {
        let leaf = path.pop()?;
        match path.pop() {
          Some(parent) => Some(CommitGroup {
            parent: Some(parent),
            parent_source: Some(ParentSource::Prefix),
            ..group(leaf, GroupOrigin::Prefix)
          }),
          None => Some(stored(group(leaf, GroupOrigin::Prefix))),
        }
      }
      Err(error) => error.leaf.map(|leaf| CommitGroup {
        invalid: true,
        ..group(leaf, GroupOrigin::Prefix)
      }),
    },
    Some(prefix) => Some(group(sanitize_branch_name(prefix.trim()), GroupOrigin::Prefix)),
    None => {
      let key = find_issue_number(parsed.rest)?;
      let found = match inputs.issue_groups.entries.get(key) {
        Some(name) => group(name.clone(), GroupOrigin::IssueGroup),
        None => group(key.to_owned(), GroupOrigin::Issue),
      };
      Some(if inputs.stacks { stored(found) } else { found })
    }
  }
}

/// The issue group a subject is grouped into through `issue_groups`, with its key: only a subject without a prefix
pub fn issue_group_of<'a>(subject: &str, issue_groups: &'a BTreeMap<String, String>) -> Option<(&'a str, &'a str)> {
  let parsed = parse_subject(subject);
  if parsed.prefix.is_some() {
    return None;
  }
  let key = find_issue_number(parsed.rest)?;
  issue_groups.get_key_value(key).map(|(key, name)| (key.as_str(), name.as_str()))
}

/// Struct to incrementally group commits by prefix
pub struct CommitGrouper {
  /// Unified structure combining commits and author frequencies per branch
  branch_data: IndexMap<String, BranchData>,
  unassigned_commits: Vec<Commit>,
  /// With stacked branches on, what the prefixes declare
  stacks: Option<StackDeclarations>,
  /// Issue key -> branch name
  issue_groups: BTreeMap<String, String>,
  /// The keys of `issue_groups` that grouped a commit
  used_issue_groups: BTreeSet<String>,
  /// First parent of the oldest commit seen (the commit the virtual branches are rebuilt on)
  pub first_parent_id: Option<String>,
  pub commit_count: usize,
}

impl Default for CommitGrouper {
  fn default() -> Self {
    Self::new()
  }
}

impl CommitGrouper {
  pub fn new() -> Self {
    Self::new_with_stacks(false)
  }

  /// A grouper that reads `(parent/child)` prefixes as stacked branches if `stacks` is on (`stack::stacks_enabled`);
  /// otherwise a `/` is part of a flat branch name
  pub fn new_with_stacks(stacks: bool) -> Self {
    Self::new_with(&GroupingInputs::with_stacks(stacks))
  }

  /// A grouper with the stack setting and the issue groups of `inputs`
  pub fn new_with(inputs: &GroupingInputs) -> Self {
    Self {
      branch_data: IndexMap::new(),
      unassigned_commits: Vec::new(),
      stacks: inputs.stacks.then(|| StackDeclarations::with_stored(inputs.stack_parents.entries.clone())),
      issue_groups: inputs.issue_groups.entries.clone(),
      used_issue_groups: BTreeSet::new(),
      first_parent_id: None,
      commit_count: 0,
    }
  }

  pub fn add_commit(&mut self, mut commit: Commit) {
    // The oldest commit is the first one we see
    if self.commit_count == 0 {
      self.first_parent_id.clone_from(&commit.parent_id);
    }
    let position = self.commit_count;
    self.commit_count += 1;

    let parsed = parse_subject(&commit.subject);
    let branch_name = if let (Some(declarations), Some(prefix)) = (&mut self.stacks, parsed.prefix) {
      match stack_path(prefix) {
        Ok(path) => {
          declarations.declare(&path, &commit.id);
          path.last().cloned()
        }
        // grouped into its last level, which reports the problem; `(/)` names no branch
        Err(error) => error.leaf.inspect(|leaf| declarations.syntax_error(leaf, &commit.id)),
      }
    } else {
      if parsed.prefix.is_none()
        && let Some(key) = find_issue_number(parsed.rest)
        && self.issue_groups.contains_key(key)
      {
        self.used_issue_groups.insert(key.to_string());
      }
      branch_of(&parsed, &self.issue_groups)
    };
    let Some(branch_name) = branch_name else {
      self.unassigned_commits.push(commit);
      return;
    };
    if let Some(declarations) = &mut self.stacks {
      declarations.note_branch(&branch_name);
    }
    // an explicit prefix is stripped (`stripped_subject`); an issue-based subject stays as is
    if let Some(prefix) = parsed.prefix {
      commit.stripped_subject = subject_after_prefix(prefix, parsed.rest).into_owned();
    }
    self.branch_data.entry(branch_name).or_insert_with(|| BranchData::new(position)).add_commit(commit);
  }

  pub fn finish(self) -> GroupedCommits {
    self.finish_with_stacks().0
  }

  /// The grouped commits and, with stacked branches on, how the branches stack
  pub fn finish_with_stacks(self) -> (GroupedCommits, Option<StackForest>) {
    let (stacks, idle_stack_parents) = self.stacks.map_or_else(
      || (None, Vec::new()),
      |declarations| {
        let first_positions: HashMap<&str, usize> = self.branch_data.iter().map(|(name, data)| (name.as_str(), data.first_position)).collect();
        let (forest, idle) = declarations.into_forest(&first_positions);
        (Some(forest), idle)
      },
    );

    // Extract commits and author emails from unified structure
    let mut grouped_commits = IndexMap::new();
    let mut branch_emails = HashMap::new();

    for (branch_name, branch_data) in self.branch_data {
      branch_emails.insert(branch_name.clone(), branch_data.most_frequent_author());
      grouped_commits.insert(branch_name, branch_data.commits);
    }

    // Build a summary of all branches for a single structured log entry
    let branch_details: Vec<String> = grouped_commits.iter().map(|(prefix, commits)| format!("{}: {}", prefix, commits.len())).collect();

    info!(
      branches = %grouped_commits.len(),
      unassigned = %self.unassigned_commits.len(),
      branch_details = ?branch_details,
      "Commit grouping completed"
    );

    let grouped = GroupedCommits {
      branches: grouped_commits,
      unassigned: self.unassigned_commits,
      branch_emails,
      idle_issue_groups: self.issue_groups.into_keys().filter(|key| !self.used_issue_groups.contains(key)).collect(),
      idle_stack_parents,
    };
    (grouped, stacks)
  }
}

#[cfg(test)]
#[path = "commit_grouper_test.rs"]
mod commit_grouper_test;
