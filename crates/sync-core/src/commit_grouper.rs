use crate::issue_groups::IssueGroups;
use crate::stack::{StackDeclarations, StackForest, leaf_of, stack_path};
use git_ops::commit_list::Commit;
use git_ops::model::sanitize_branch_name;
use indexmap::IndexMap;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use sync_utils::issue_pattern::find_issue_number;
use tracing::info;

/// Branch data combining commits and author frequency tracking
#[derive(Debug)]
struct BranchData {
  commits: Vec<Commit>,
  author_frequencies: HashMap<String, u32>,
  /// Position of the oldest commit among all grouped commits
  first_position: usize,
}

impl BranchData {
  fn new(first_position: usize) -> Self {
    Self {
      commits: Vec::new(),
      author_frequencies: HashMap::new(),
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

  fn most_frequent_author(&self) -> Option<String> {
    self.author_frequencies.iter().max_by_key(|(_, count)| *count).map(|(email, _)| email.clone())
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
  /// The keys of the issue groups that group no commit, sorted
  pub stale_issue_groups: Vec<String>,
}

/// What decides the group of a commit besides its subject (`issue_groups::read_grouping_inputs`)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GroupingInputs {
  /// Stacked branches are on (`stack::stacks_enabled`)
  pub stacks: bool,
  pub issue_groups: IssueGroups,
}

impl GroupingInputs {
  /// Without issue groups
  pub fn with_stacks(stacks: bool) -> Self {
    Self {
      stacks,
      issue_groups: IssueGroups::default(),
    }
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

/// Git's autosquash markers; each needs the space, `fixup!X` is an ordinary subject (`git help rebase`, `--autosquash`)
pub const AUTOSQUASH_MARKERS: [&str; 3] = ["fixup! ", "squash! ", "amend! "];

/// A commit subject split into the parts that decide its group
#[derive(Debug, PartialEq, Eq)]
pub struct ParsedSubject<'a> {
  /// The leading git autosquash markers (`fixup! `, `squash! `, `amend! `, repeated as in `fixup! fixup! X`) with the
  /// whitespace after them, or empty
  pub autosquash: &'a str,
  /// The unsanitized text of a leading non-empty `(prefix)`
  pub prefix: Option<&'a str>,
  /// The subject after the autosquash marker and the prefix
  pub rest: &'a str,
}

/// Splits a subject the way the grouper reads it: an explicit `(prefix)` at the start (after an autosquash marker,
/// so `fixup! (feat) X` stays in `feat`) names the branch; otherwise the rest may still carry an issue key.
pub fn parse_subject(subject: &str) -> ParsedSubject<'_> {
  let body = strip_autosquash_markers(subject);
  let autosquash = &subject[..subject.len() - body.len()];
  if body.starts_with('(')
    && let Some(close_paren_pos) = body.find(')')
    && close_paren_pos > 1
  {
    return ParsedSubject {
      autosquash,
      prefix: Some(&body[1..close_paren_pos]),
      rest: body[close_paren_pos + 1..].trim_start(),
    };
  }
  ParsedSubject {
    autosquash,
    prefix: None,
    rest: body,
  }
}

/// The subject after all leading autosquash markers, as git reads the target of `fixup! squash! X`: each marker and
/// the whitespace after it are skipped
fn strip_autosquash_markers(subject: &str) -> &str {
  let mut rest = subject;
  while let Some(stripped) = AUTOSQUASH_MARKERS.iter().find_map(|marker| rest.strip_prefix(marker)) {
    rest = stripped.trim_start();
  }
  rest
}

/// The virtual branch a commit with this subject is grouped into: its sanitized `(prefix)`, else the issue key of the
/// subject; `None` for an unassigned commit
pub fn branch_name_of(subject: &str) -> Option<String> {
  branch_of(&parse_subject(subject), &BTreeMap::new())
}

/// The subject of the commit's virtual branch commit: an explicit prefix (and the autosquash marker before it) is
/// stripped, an issue-based or unassigned subject stays as is
pub fn stripped_subject(subject: &str) -> &str {
  let parsed = parse_subject(subject);
  if parsed.prefix.is_some() { parsed.rest } else { subject }
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
      stacks: inputs.stacks.then(StackDeclarations::default),
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
    if parsed.prefix.is_some() {
      commit.stripped_subject = parsed.rest.to_string();
    }
    self.branch_data.entry(branch_name).or_insert_with(|| BranchData::new(position)).add_commit(commit);
  }

  pub fn finish(self) -> GroupedCommits {
    self.finish_with_stacks().0
  }

  /// The grouped commits and, with stacked branches on, how the branches stack
  pub fn finish_with_stacks(self) -> (GroupedCommits, Option<StackForest>) {
    let stacks = self.stacks.map(|declarations| {
      let first_positions: HashMap<&str, usize> = self.branch_data.iter().map(|(name, data)| (name.as_str(), data.first_position)).collect();
      declarations.into_forest(&first_positions)
    });

    // Extract commits and author emails from unified structure
    let mut grouped_commits = IndexMap::new();
    let mut branch_emails = HashMap::new();

    for (branch_name, branch_data) in self.branch_data {
      branch_emails.insert(branch_name.clone(), branch_data.most_frequent_author());
      grouped_commits.insert(branch_name.clone(), branch_data.commits);
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
      stale_issue_groups: self.issue_groups.into_keys().filter(|key| !self.used_issue_groups.contains(key)).collect(),
    };
    (grouped, stacks)
  }
}

#[cfg(test)]
#[path = "commit_grouper_test.rs"]
mod commit_grouper_test;
