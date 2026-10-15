use crate::patronus_command::{GhComment, patronus_command_body, post_patronus_command, read_patronus_command_reply, rejection_reason};
use crate::pull_requests::GitHubRepository;
use pretty_assertions::assert_eq;
use std::path::{Path, PathBuf};
use std::time::Duration;
use sync_types::patronus::{PatronusCommandComment, PatronusCommandReply, PatronusPushMode};
use test_log::test;
use test_utils::fake_executable::fake_executable;

pub(crate) fn repository() -> GitHubRepository {
  GitHubRepository {
    owner: "acme".to_string(),
    name: "monorepo".to_string(),
  }
}

/// A fake `gh` that records its arguments and answers by the endpoint (or as `script` says)
pub(crate) fn fake_gh(dir: &Path, script: &str) -> PathBuf {
  fake_executable(dir, "gh", &format!("printf '%s\\n' \"$@\" >> '{}'\n{script}", dir.join("args").display()))
}

/// The arguments of every run of `fake_gh`, one per line
pub(crate) fn args(dir: &Path) -> Vec<String> {
  std::fs::read_to_string(dir.join("args")).unwrap().lines().map(str::to_string).collect()
}

#[test]
fn a_merge_keeps_the_source_branch_and_a_dry_run_is_plain() {
  assert_eq!(patronus_command_body(PatronusPushMode::RebaseAutosquash).unwrap(), "/patronus merge --keep-source-branch");
  assert_eq!(patronus_command_body(PatronusPushMode::DryRun).unwrap(), "/patronus dry-run");
  assert!(patronus_command_body(PatronusPushMode::SquashAll).is_err());
}

#[test]
fn posts_the_command_as_a_regular_comment_of_the_pull_request() {
  let dir = tempfile::tempdir().unwrap();
  let gh = fake_gh(
    dir.path(),
    r#"echo '{"id": 4402154133, "html_url": "https://github.com/acme/monorepo/pull/7#issuecomment-4402154133"}'"#,
  );
  let comment = post_patronus_command(&gh, ".", &repository(), 7, PatronusPushMode::RebaseAutosquash, Duration::from_secs(10)).unwrap();
  assert_eq!(
    comment,
    PatronusCommandComment {
      id: "4402154133".to_string(),
      url: "https://github.com/acme/monorepo/pull/7#issuecomment-4402154133".to_string(),
    }
  );
  assert_eq!(
    args(dir.path()),
    vec![
      "api",
      "--method",
      "POST",
      "repos/acme/monorepo/issues/7/comments",
      "-f",
      "body=/patronus merge --keep-source-branch"
    ]
  );
}

#[test]
fn a_comment_without_reactions_is_pending_and_a_thumbs_up_is_accepted() {
  let dir = tempfile::tempdir().unwrap();
  let gh = fake_gh(dir.path(), r#"echo '{"id": 10, "created_at": "2026-10-15T10:00:00Z", "reactions": {"+1": 0, "-1": 0}}'"#);
  let reply = read_patronus_command_reply(&gh, ".", &repository(), 7, "10", Duration::from_secs(10)).unwrap();
  assert_eq!(reply, PatronusCommandReply::Pending);
  assert_eq!(args(dir.path()), vec!["api", "repos/acme/monorepo/issues/comments/10"]);

  let gh = fake_gh(dir.path(), r#"echo '{"id": 10, "created_at": "2026-10-15T10:00:00Z", "reactions": {"+1": 1, "-1": 0}}'"#);
  let reply = read_patronus_command_reply(&gh, ".", &repository(), 7, "10", Duration::from_secs(10)).unwrap();
  assert_eq!(reply, PatronusCommandReply::Accepted);
}

#[test]
fn a_thumbs_down_reads_the_reason_from_the_bot_comment() {
  let dir = tempfile::tempdir().unwrap();
  let gh = fake_gh(
    dir.path(),
    r#"case "$2" in
  *issues/comments/10) echo '{"id": 10, "created_at": "2026-10-15T10:00:00Z", "reactions": {"+1": 0, "-1": 1}}' ;;
  *) echo '[{"id": 10, "body": "/patronus merge", "user": {"type": "User"}}, {"id": 11, "body": "⚠️ Pull request #7 is a draft and cannot be merged.", "user": {"type": "Bot"}}]' ;;
esac"#,
  );
  let reply = read_patronus_command_reply(&gh, ".", &repository(), 7, "10", Duration::from_secs(10)).unwrap();
  assert_eq!(
    reply,
    PatronusCommandReply::Rejected {
      reason: Some("Pull request #7 is a draft and cannot be merged.".to_string())
    }
  );
  assert!(
    args(dir.path()).contains(&"repos/acme/monorepo/issues/7/comments?since=2026-10-15T10:00:00Z&per_page=100".to_string()),
    "{:?}",
    args(dir.path())
  );
}

#[test]
fn the_reason_is_a_later_bot_comment_and_a_user_warning_is_not_one() {
  let comments: Vec<GhComment> = serde_json::from_str(
    r#"[
      {"id": 9, "body": "⚠️ an older answer", "user": {"type": "Bot"}},
      {"id": 11, "body": "⚠️ a user quoting the bot", "user": {"type": "User"}},
      {"id": 12, "body": "🚀 started", "user": {"type": "Bot"}}
    ]"#,
  )
  .unwrap();
  assert_eq!(rejection_reason(&comments, 10), None);
}
