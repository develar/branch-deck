use git_ops::commit_list::Commit;
use indexmap::IndexMap;
use regex::Regex;
use std::sync::OnceLock;
use tracing::{debug, info};

// Static regex patterns - compiled once on first use
static PREFIX_PATTERN: OnceLock<Regex> = OnceLock::new();
pub static ISSUE_PATTERN: OnceLock<Regex> = OnceLock::new();

// Type alias for grouped commits result
pub type GroupedCommitsResult = (IndexMap<String, Vec<Commit>>, Vec<Commit>);

/// Struct to incrementally group commits by prefix
pub struct CommitGrouper {
  prefix_to_commits: IndexMap<String, Vec<Commit>>,
  unassigned_commits: Vec<Commit>,
  prefix_pattern: &'static Regex,
  issue_pattern: &'static Regex,
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
    // Initialize regex patterns on first use
    let prefix_pattern = PREFIX_PATTERN.get_or_init(|| Regex::new(r"\((.+?)\)(.*?)(?:\r?\n|$)").unwrap());
    let issue_pattern = ISSUE_PATTERN.get_or_init(|| Regex::new(r"\b([A-Z]+-\d+)\b").unwrap());

    Self {
      prefix_to_commits: IndexMap::new(),
      unassigned_commits: Vec::new(),
      prefix_pattern,
      issue_pattern,
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

    // First try to find explicit prefix in parentheses
    if let Some(captures) = self.prefix_pattern.captures(subject) {
      if let (Some(prefix_match), Some(message_match)) = (captures.get(1), captures.get(2)) {
        let prefix = prefix_match.as_str().trim();
        let message_text = message_match.as_str().trim();

        // Set the stripped subject
        commit.stripped_subject = message_text.to_string();

        self.prefix_to_commits.entry(prefix.to_string()).or_default().push(commit);
        return;
      }
    }

    // If no explicit parentheses prefix, look for issue number pattern in the subject line
    if let Some(issue_match) = self.issue_pattern.find(subject) {
      let issue_number = issue_match.as_str();

      // For issue-based grouping, we don't strip anything
      // The subject remains as-is

      self.prefix_to_commits.entry(issue_number.to_string()).or_default().push(commit);
      return;
    }

    // If no prefix found, add to unassigned commits
    self.unassigned_commits.push(commit);
  }

  pub fn finish(self) -> GroupedCommitsResult {
    info!(
      "Grouped commits into {} branches, {} unassigned commits",
      self.prefix_to_commits.len(),
      self.unassigned_commits.len()
    );
    for (prefix, commits) in &self.prefix_to_commits {
      debug!("Branch '{prefix}' has {} commits", commits.len());
    }
    (self.prefix_to_commits, self.unassigned_commits)
  }
}
