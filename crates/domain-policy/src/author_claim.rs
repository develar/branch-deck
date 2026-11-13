//! The author's word of a pull request (openspec/specs/review/range-reading/spec.md, "The author's word"): what its
//! labels or the conventional type of its title say it is, from a closed list (`claim_of`), and the one case the diff
//! proves it wrong (`check`): the author says refactor, tests or docs, and the range leaves changes to read outside
//! that role in files the reading proves; a file no proof can be made of proves nothing. A fix or a feature is never
//! checked: nothing in a diff proves one. The word is the author's: the window shows it as such, never as a fact.

use domain_types::range_reading::{AuthorClaim, ClaimConflict, ClaimKind, ClaimSource, FileRole, RangeReading};

/// The kind a label names, case-insensitive: refactor, refactoring, cleanup; test, tests; docs, documentation
fn label_kind(label: &str) -> Option<ClaimKind> {
  match label.trim().to_ascii_lowercase().as_str() {
    "refactor" | "refactoring" | "cleanup" => Some(ClaimKind::Refactor),
    "test" | "tests" => Some(ClaimKind::Tests),
    "docs" | "documentation" => Some(ClaimKind::Docs),
    _ => None,
  }
}

/// The kind a conventional type names, case-insensitive: refactor; test, tests; docs. Any other type (`fix`, `feat`, a
/// word before a colon that is no type) says nothing
fn type_kind(name: &str) -> Option<ClaimKind> {
  match name.to_ascii_lowercase().as_str() {
    "refactor" => Some(ClaimKind::Refactor),
    "test" | "tests" => Some(ClaimKind::Tests),
    "docs" => Some(ClaimKind::Docs),
    _ => None,
  }
}

/// The conventional type `title` starts with: `type`, `type(scope)` or either with `!`, then `: `; the type alone
fn conventional_type(title: &str) -> Option<&str> {
  let title = title.trim_start();
  let end = title.find(|c: char| !c.is_ascii_alphabetic())?;
  let (name, rest) = title.split_at(end);
  let rest = match rest.strip_prefix('(') {
    Some(scoped) => &scoped[scoped.find(')')? + 1..],
    None => rest,
  };
  let rest = rest.strip_prefix('!').unwrap_or(rest);
  (!name.is_empty() && rest.starts_with(": ")).then_some(name)
}

/// What the author says the pull request of `title` and `labels` is: the first label of the closed list, else the
/// conventional type of the title. A title's words past its type ("; unchanged" too) say nothing
#[must_use]
pub fn claim_of(title: &str, labels: &[String]) -> Option<AuthorClaim> {
  let labelled = labels.iter().find_map(|label| {
    label_kind(label).map(|kind| AuthorClaim {
      kind,
      source: ClaimSource::Label { name: label.clone() },
    })
  });
  labelled.or_else(|| {
    let prefix = conventional_type(title)?;
    type_kind(prefix).map(|kind| AuthorClaim {
      kind,
      source: ClaimSource::Title { prefix: prefix.to_string() },
    })
  })
}

/// The diff proves `claim` wrong: `reading` leaves changes to read outside the role it names (refactor: of code, tests:
/// outside the tests, docs: outside the prose) in files it proves (`RoleReading::to_read_proven`). `None` when it
/// holds, and when every change left outside the role is in a file no proof can be made of (`RangeReading::unproven`:
/// a rename that waits for the index or that it did not prove, a text without a grammar, a binary file): such a
/// change is counted to read and proves nothing, and the summary names the file as not proven
#[must_use]
pub fn check(claim: AuthorClaim, reading: &RangeReading) -> Option<ClaimConflict> {
  let outside = |role: FileRole| match claim.kind {
    ClaimKind::Refactor => role == FileRole::Code,
    ClaimKind::Tests => role != FileRole::Tests,
    ClaimKind::Docs => role != FileRole::Prose,
  };
  let (to_read, files) = reading
    .roles
    .iter()
    .filter(|role| outside(role.role))
    .fold((0, 0), |(to_read, files), role| (to_read + role.to_read_proven, files + role.files_to_read_proven));
  (to_read > 0).then_some(ClaimConflict { claim, to_read, files })
}
