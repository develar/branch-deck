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

// Type alias for grouped commits result with author emails
pub type GroupedCommitsResult = (IndexMap<String, Vec<Commit>>, Vec<Commit>, HashMap<String, Option<String>>);

/// Struct to incrementally group commits by prefix
pub struct CommitGrouper {
  /// Unified structure combining commits and author frequencies per branch
  branch_data: IndexMap<String, BranchData>,
  unassigned_commits: Vec<Commit>,
  pub oldest_commit: Option<Commit>,
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
      oldest_commit: None,
      commit_count: 0,
    }
  }

  pub fn add_commit(&mut self, mut commit: Commit) {
    // Track the oldest commit (first one we see)
    if self.oldest_commit.is_none() {
      self.oldest_commit = Some(commit.clone());
    }
    self.commit_count += 1;

    let subject = &commit.subject;

    // Strip git autosquash prefixes (fixup!, squash!, amend!) for grouping purposes
    // These are used by git rebase --autosquash to combine commits
    let subject_for_grouping = if let Some(stripped) = subject
      .strip_prefix("fixup!")
      .or_else(|| subject.strip_prefix("squash!"))
      .or_else(|| subject.strip_prefix("amend!"))
    {
      stripped.trim_start()
    } else {
      subject
    };

    // First try to find explicit prefix in parentheses using manual parsing (faster than regex)
    if subject_for_grouping.starts_with('(')
      && let Some(close_paren_pos) = subject_for_grouping.find(')')
    {
      // Extract prefix between parentheses
      let prefix = &subject_for_grouping[1..close_paren_pos];
      // Only accept non-empty prefixes
      if !prefix.is_empty() {
        // Sanitize the prefix to make it a valid Git branch name
        let sanitized_prefix = sanitize_branch_name(prefix.trim());

        // Get the rest of the message after the closing parenthesis
        let rest = &subject_for_grouping[close_paren_pos + 1..];
        let message_text = rest.trim_start();

        // Set the stripped subject
        commit.stripped_subject = message_text.to_string();

        // Add commit to unified branch data structure
        self.branch_data.entry(sanitized_prefix).or_insert_with(BranchData::new).add_commit(commit);
        return;
      }
    }

    // If no explicit parentheses prefix, look for issue number pattern in the subject line
    // Manual parsing for issue pattern (e.g., JIRA-123, ABC-4567)
    if let Some(issue_number) = find_issue_number(subject_for_grouping) {
      // For issue-based grouping, we don't strip anything
      // The subject remains as-is

      // Add commit to unified branch data structure
      self.branch_data.entry(issue_number.to_owned()).or_insert_with(BranchData::new).add_commit(commit);
      return;
    }

    // If no prefix found, add to unassigned commits
    self.unassigned_commits.push(commit);
  }

  pub fn finish(self) -> GroupedCommitsResult {
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

    (grouped_commits, self.unassigned_commits, branch_emails)
  }
}

#[cfg(test)]
#[path = "commit_grouper_test.rs"]
mod commit_grouper_test;
