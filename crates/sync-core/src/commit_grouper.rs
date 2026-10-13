use git_ops::commit_list::Commit;
use git_ops::model::sanitize_branch_name;
use indexmap::IndexMap;
use std::collections::HashMap;
use sync_utils::issue_pattern::find_issue_number;
use tracing::info;

/// Branch data combining commits and author frequency tracking
#[derive(Debug)]
struct BranchData {
  commits: Vec<Commit>,
  author_frequencies: HashMap<String, u32>,
}

impl BranchData {
  fn new() -> Self {
    Self {
      commits: Vec::new(),
      author_frequencies: HashMap::new(),
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
}

/// A commit subject split into the parts that decide its group
#[derive(Debug, PartialEq, Eq)]
pub struct ParsedSubject<'a> {
  /// A git autosquash marker (`fixup!`, `squash!`, `amend!`) with the whitespace after it, or empty
  pub autosquash: &'a str,
  /// The unsanitized text of a leading non-empty `(prefix)`
  pub prefix: Option<&'a str>,
  /// The subject after the autosquash marker and the prefix
  pub rest: &'a str,
}

/// Splits a subject the way the grouper reads it: an explicit `(prefix)` at the start (after an autosquash marker,
/// so `fixup! (feat) X` stays in `feat`) names the branch; otherwise the rest may still carry an issue key.
pub fn parse_subject(subject: &str) -> ParsedSubject<'_> {
  let body = ["fixup!", "squash!", "amend!"]
    .iter()
    .find_map(|marker| subject.strip_prefix(marker))
    .map_or(subject, str::trim_start);
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

/// Struct to incrementally group commits by prefix
pub struct CommitGrouper {
  /// Unified structure combining commits and author frequencies per branch
  branch_data: IndexMap<String, BranchData>,
  unassigned_commits: Vec<Commit>,
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
    Self {
      branch_data: IndexMap::new(),
      unassigned_commits: Vec::new(),
      first_parent_id: None,
      commit_count: 0,
    }
  }

  pub fn add_commit(&mut self, mut commit: Commit) {
    // The oldest commit is the first one we see
    if self.commit_count == 0 {
      self.first_parent_id.clone_from(&commit.parent_id);
    }
    self.commit_count += 1;

    let parsed = parse_subject(&commit.subject);
    if let Some(prefix) = parsed.prefix {
      // Sanitize the prefix to make it a valid Git branch name
      let branch_name = sanitize_branch_name(prefix.trim());
      commit.stripped_subject = parsed.rest.to_string();
      self.branch_data.entry(branch_name).or_insert_with(BranchData::new).add_commit(commit);
      return;
    }

    // If no explicit parentheses prefix, look for issue number pattern in the subject line
    // Manual parsing for issue pattern (e.g., JIRA-123, ABC-4567)
    if let Some(issue_number) = find_issue_number(parsed.rest) {
      // For issue-based grouping, we don't strip anything
      // The subject remains as-is

      // Add commit to unified branch data structure
      self.branch_data.entry(issue_number.to_owned()).or_insert_with(BranchData::new).add_commit(commit);
      return;
    }

    // If no prefix found, add to unassigned commits
    self.unassigned_commits.push(commit);
  }

  pub fn finish(self) -> GroupedCommits {
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

    GroupedCommits {
      branches: grouped_commits,
      unassigned: self.unassigned_commits,
      branch_emails,
    }
  }
}

#[cfg(test)]
#[path = "commit_grouper_test.rs"]
mod commit_grouper_test;
