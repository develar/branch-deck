use git_ops::commit_list::Commit;

/// A commit as the commit list reads it, with fixed metadata; the subject is the first line of `message`
pub(crate) fn commit(id: &str, message: &str) -> Commit {
  let subject = message.split('\n').next().unwrap().to_string();
  Commit {
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
