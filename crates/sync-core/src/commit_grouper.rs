use git_ops::commit_list::Commit;
use indexmap::IndexMap;
use sync_utils::issue_pattern::find_issue_number;
use tracing::info;

// Type alias for grouped commits result
pub type GroupedCommitsResult = (IndexMap<String, Vec<Commit>>, Vec<Commit>);

/// Struct to incrementally group commits by prefix
pub struct CommitGrouper {
  prefix_to_commits: IndexMap<String, Vec<Commit>>,
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
      prefix_to_commits: IndexMap::new(),
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

    // First try to find explicit prefix in parentheses using manual parsing (faster than regex)
    if subject.starts_with('(')
      && let Some(close_paren_pos) = subject.find(')')
    {
      // Extract prefix between parentheses
      let prefix = &subject[1..close_paren_pos];

      // Only accept non-empty prefixes
      if !prefix.is_empty() {
        let prefix = prefix.trim();

        // Get the rest of the message after the closing parenthesis
        let rest = &subject[close_paren_pos + 1..];
        let message_text = rest.trim_start();

        // Set the stripped subject
        commit.stripped_subject = message_text.to_string();

        self.prefix_to_commits.entry(prefix.to_string()).or_default().push(commit);
        return;
      }
    }

    // If no explicit parentheses prefix, look for issue number pattern in the subject line
    // Manual parsing for issue pattern (e.g., JIRA-123, ABC-4567)
    if let Some(issue_number) = find_issue_number(subject) {
      // For issue-based grouping, we don't strip anything
      // The subject remains as-is

      self.prefix_to_commits.entry(issue_number.to_owned()).or_default().push(commit);
      return;
    }

    // If no prefix found, add to unassigned commits
    self.unassigned_commits.push(commit);
  }

  pub fn finish(self) -> GroupedCommitsResult {
    // Build a summary of all branches for a single structured log entry
    let branch_summary: Vec<String> = self.prefix_to_commits.iter().map(|(prefix, commits)| format!("{}: {}", prefix, commits.len())).collect();

    info!(
      branches = %self.prefix_to_commits.len(),
      unassigned = %self.unassigned_commits.len(),
      branch_details = ?branch_summary,
      "Commit grouping completed"
    );

    (self.prefix_to_commits, self.unassigned_commits)
  }
}

#[cfg(test)]
#[path = "commit_grouper_test.rs"]
mod commit_grouper_test;
