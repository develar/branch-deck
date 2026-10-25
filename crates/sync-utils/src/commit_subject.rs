//! The grammar of a commit subject: git's autosquash markers, the `(prefix)` that names a virtual branch, the subject
//! of the virtual branch commit without it, and the subject as git's `%s` reads a message. Plain string code; the
//! grouping that reads it is `history_rewrite::commit_grouper`, the identity of a commit across a rebase
//! `git_ops::landed_pairing::CommitKey`.

use crate::issue_pattern::{find_issue_number, has_issue_reference, issue_key_position};
use std::borrow::Cow;

/// Git's autosquash markers; each needs the space, `fixup!X` is an ordinary subject (`git help rebase`, `--autosquash`)
pub const AUTOSQUASH_MARKERS: [&str; 3] = ["fixup! ", "squash! ", "amend! "];

/// The index in `AUTOSQUASH_MARKERS` of the marker the subject starts with: the first one decides (`fixup! squash! X`
/// is a fixup)
pub fn first_marker(subject: &str) -> Option<usize> {
  AUTOSQUASH_MARKERS.iter().position(|marker| subject.starts_with(marker))
}

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

/// The subject of the commit's virtual branch commit: an explicit prefix (and the autosquash marker before it) is
/// stripped (`subject_after_prefix`), an issue-based or unassigned subject stays as is
pub fn stripped_subject(subject: &str) -> Cow<'_, str> {
  let parsed = parse_subject(subject);
  match parsed.prefix {
    Some(prefix) => subject_after_prefix(prefix, parsed.rest),
    None => Cow::Borrowed(subject),
  }
}

/// The subject after the prefix `prefix`. A prefix whose branch is an issue key names the issue of a commit that has no
/// key of its own, so the key stays where `find_issue_number` reads one: `(IJPL-1) docs: x` becomes `docs: IJPL-1 x`,
/// and the commit that lands still names its issue. A rest with a key of its own stays as is
pub fn subject_after_prefix<'a>(prefix: &str, rest: &'a str) -> Cow<'a, str> {
  let leaf = prefix.rsplit('/').next().unwrap_or(prefix).trim();
  if find_issue_number(leaf) != Some(leaf) || has_issue_reference(rest) {
    return Cow::Borrowed(rest);
  }
  let (before, after) = rest.split_at(issue_key_position(rest).unwrap_or(0));
  let mut subject = String::with_capacity(rest.len() + leaf.len() + 2);
  subject.push_str(before);
  if !before.is_empty() && !before.ends_with(char::is_whitespace) {
    subject.push(' ');
  }
  subject.push_str(leaf);
  if !after.is_empty() {
    subject.push(' ');
    subject.push_str(after);
  }
  Cow::Owned(subject)
}

/// The subject as git's `%s` reads it: the first paragraph of the message, its lines joined by a space
pub fn subject_of(message: &str) -> String {
  message
    .trim_start()
    .lines()
    .take_while(|line| !line.trim().is_empty())
    .map(str::trim)
    .collect::<Vec<_>>()
    .join(" ")
}
