use crate::common::oldest_logged_commit;
use git_executor::git_command_executor::GitCommandExecutor;
use std::sync::OnceLock;
use sync_types::branch_integration::{BranchIntegrationStatus, IntegrationConfidence};
use tracing::info;

/// The squash check of one detection: the tree of the baseline is read once, by the first branch that needs it
pub struct SquashCheck<'a> {
  git: &'a GitCommandExecutor,
  repo: &'a str,
  /// A commit id (`resolve_baseline`)
  baseline: &'a str,
  baseline_tree: OnceLock<Option<String>>,
}

impl<'a> SquashCheck<'a> {
  pub const fn new(git: &'a GitCommandExecutor, repo: &'a str, baseline: &'a str) -> Self {
    Self {
      git,
      repo,
      baseline,
      baseline_tree: OnceLock::new(),
    }
  }

  fn baseline_tree(&self) -> Option<&str> {
    self
      .baseline_tree
      .get_or_init(|| {
        let tree = format!("{}^{{tree}}", self.baseline);
        let output = self.git.execute_command(&["rev-parse", "--verify", "--end-of-options", &tree], self.repo).ok()?;
        Some(output.trim().to_string())
      })
      .as_deref()
  }

  /// Whether the changes of the branch are all in the baseline: merging `tip`, the tip of `branch_name`, into the
  /// baseline gives the baseline's tree (one `merge-tree`, a clean one: a conflict leaves out the side the baseline
  /// deleted). An empty change merges trivially, so it counts too. `stack_base` is the commit a stacked branch was
  /// built on and the merge base then: only the changes after it are the branch's own; otherwise `merge-tree` finds the
  /// merge base itself. `subject` is the subject of the tip, read again when unknown (`None` or empty).
  pub fn detect(&self, branch_name: &str, tip: &str, subject: Option<&str>, stack_base: Option<&str>, right_count: usize) -> Option<BranchIntegrationStatus> {
    let merge_base = stack_base.map(|base| format!("--merge-base={base}"));
    let mut args = vec!["merge-tree", "--write-tree"];
    args.extend(merge_base.as_deref());
    args.extend(["--end-of-options", self.baseline, tip]);
    // the tree alone when clean; with conflicts, their list follows
    let merged = self.git.execute_command_accepting(&args, self.repo, &[1]).ok()?;
    if Some(merged.trim()) != self.baseline_tree() {
      return None;
    }
    info!(name = %branch_name, method = "merge-tree", "Branch fully integrated");
    Some(self.integrated(tip, subject, right_count))
  }

  /// The squashed branch as integrated, with the baseline commit that squashed it in: the newest one whose message has
  /// the subject of the branch tip
  fn integrated(&self, tip: &str, subject: Option<&str>, right_count: usize) -> BranchIntegrationStatus {
    let subject = match subject.map(str::trim).filter(|subject| !subject.is_empty()) {
      Some(subject) => Some(subject.to_string()),
      None => self
        .git
        .execute_command(&["log", "-1", "--format=%s", "--end-of-options", tip], self.repo)
        .ok()
        .map(|subject| subject.trim().to_string()),
    };
    let commit = subject
      .filter(|subject| !subject.is_empty())
      .and_then(|subject| oldest_logged_commit(self.git, self.repo, &["-F", "--grep", &subject, "-n", "1", "--end-of-options", self.baseline]));
    BranchIntegrationStatus::Integrated {
      integrated_at: commit.as_ref().map(|commit| commit.time),
      confidence: IntegrationConfidence::High,
      commit_count: u32::try_from(right_count).unwrap_or(u32::MAX),
      commit,
    }
  }
}
