//! The grammar of a commit subject: git's autosquash markers, the `(prefix)` that names a virtual branch, the subject
//! of the virtual branch commit without it, and the subject as git's `%s` reads a message. Plain string code; the
//! grouping that reads it is `history_rewrite::commit_grouper`, the identity of a commit across a rebase
//! `git_ops::landed_pairing::CommitKey`.

use crate::issue_pattern::{find_issue_number, has_issue_reference, issue_key_position, issue_lead};
use domain_types::commit::SubjectParts;
use std::borrow::Cow;

/// Git's autosquash markers; each needs the space, `fixup!X` is an ordinary subject (`git help rebase`, `--autosquash`)
pub const AUTOSQUASH_MARKERS: [&str; 3] = ["fixup! ", "squash! ", "amend! "];

/// A subject read as git's autosquash reads it, including a marker immediately after a branch prefix
#[derive(Debug, PartialEq, Eq)]
pub struct AutosquashSubject<'a> {
  /// Index in `AUTOSQUASH_MARKERS` of the first marker: the kind
  pub marker: usize,
  /// The subject the fixup names, with its prefix kept
  pub names: Cow<'a, str>,
  /// The subject after the markers and without its prefix, used by the hash rule
  pub after_markers: &'a str,
}

/// The autosquash subject and the first marker that decides its kind, whether before or after a prefix
pub fn autosquash_subject(subject: &str) -> Option<AutosquashSubject<'_>> {
  if let Some((marker, names)) = marker_chain(subject) {
    let parsed = parse_subject(names);
    if let Some(prefix) = parsed.prefix {
      if let Some((_, after_markers)) = marker_chain(parsed.rest) {
        return Some(AutosquashSubject {
          marker,
          names: prefixed_subject(prefix, after_markers),
          after_markers,
        });
      }
      return Some(AutosquashSubject {
        marker,
        names: Cow::Borrowed(names),
        after_markers: parsed.rest,
      });
    }
    return Some(AutosquashSubject {
      marker,
      names: Cow::Borrowed(names),
      after_markers: names,
    });
  }

  let parsed = parse_subject(subject);
  let prefix = parsed.prefix?;
  let close = subject.find(')')?;
  if subject[close + 1..].trim_start().len() == subject[close + 1..].len() {
    return None;
  }
  let (marker, after_markers) = marker_chain(parsed.rest)?;
  Some(AutosquashSubject {
    marker,
    names: prefixed_subject(prefix, after_markers),
    after_markers,
  })
}

/// The index in `AUTOSQUASH_MARKERS` of the first autosquash marker, before or after a prefix
pub fn first_marker(subject: &str) -> Option<usize> {
  autosquash_subject(subject).map(|parsed| parsed.marker)
}

fn marker_chain(subject: &str) -> Option<(usize, &str)> {
  let mut rest = subject;
  let mut first = None;
  while let Some((index, marker)) = AUTOSQUASH_MARKERS.iter().enumerate().find(|(_, marker)| rest.starts_with(**marker)) {
    first.get_or_insert(index);
    rest = rest[marker.len()..].trim_start();
  }
  first.map(|marker| (marker, rest))
}

fn prefixed_subject<'a>(prefix: &str, rest: &str) -> Cow<'a, str> {
  let mut subject = format!("({prefix})");
  if !rest.is_empty() {
    subject.push(' ');
    subject.push_str(rest);
  }
  Cow::Owned(subject)
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

/// The parts of `subject` the page shows and words (`SubjectParts`): the markers, the prefix, the rest with its issue key,
/// and what a row shows (the subject of the virtual commit without the markers) split at the key it starts with
pub fn subject_parts(subject: &str) -> SubjectParts {
  let parsed = parse_subject(subject);
  let shown = parsed.prefix.map_or(Cow::Borrowed(parsed.rest), |prefix| subject_after_prefix(prefix, parsed.rest));
  SubjectParts {
    autosquash: parsed.autosquash.to_owned(),
    prefix: parsed.prefix.map(str::to_owned),
    rest: parsed.rest.to_owned(),
    issue_key: find_issue_number(parsed.rest).map(str::to_owned),
    lead: issue_lead(&shown),
  }
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

#[cfg(test)]
mod tests {
  use super::*;
  use domain_types::commit::IssueLead;
  use pretty_assertions::assert_eq;

  #[test]
  fn autosquash_subject_reads_markers_before_or_after_a_prefix() {
    let marker_first = autosquash_subject("fixup! (b) X").unwrap();
    assert_eq!((marker_first.marker, marker_first.names.as_ref(), marker_first.after_markers), (0, "(b) X", "X"));
    assert_eq!(first_marker("fixup! (b) X"), Some(0));

    let prefix_first = autosquash_subject("(b) fixup! X").unwrap();
    assert_eq!((prefix_first.marker, prefix_first.names.as_ref(), prefix_first.after_markers), (0, "(b) X", "X"));
    assert_eq!(first_marker("(b) fixup! X"), Some(0));

    let chained = autosquash_subject("(b) squash! amend! X").unwrap();
    assert_eq!((chained.marker, chained.names.as_ref(), chained.after_markers), (1, "(b) X", "X"));
  }

  #[test]
  fn autosquash_subject_requires_the_space_after_the_marker_and_prefix() {
    assert!(autosquash_subject("fixup!X").is_none());
    assert!(autosquash_subject("(b) fixup!X").is_none());
    assert!(autosquash_subject("fixup!(b) X").is_none());
    assert!(autosquash_subject("(b)fixup! X").is_none());
  }

  #[test]
  fn the_parts_of_a_subject_are_its_markers_its_prefix_its_rest_and_the_key_a_row_shows_it_led_by() {
    assert_eq!(
      subject_parts("fixup! (auth) IJPL-1: Add login"),
      SubjectParts {
        autosquash: "fixup! ".to_string(),
        prefix: Some("auth".to_string()),
        rest: "IJPL-1: Add login".to_string(),
        issue_key: Some("IJPL-1".to_string()),
        lead: Some(IssueLead {
          key: "IJPL-1".to_string(),
          text: "Add login".to_string(),
        }),
      }
    );
    // the key of a prefix that names an issue leads what the row shows, the subject of the virtual commit
    let parts = subject_parts("(IJPL-1) Fix x");
    assert_eq!((parts.issue_key, parts.lead.map(|lead| lead.text)), (None, Some("Fix x".to_string())));
    let plain = subject_parts("[ui] fix(api): ABC-123 Fix");
    assert_eq!(
      (plain.autosquash.as_str(), plain.prefix, plain.issue_key.as_deref(), plain.lead),
      ("", None, Some("ABC-123"), None)
    );
  }
}
