use crate::author_claim::{check, claim_of};
use domain_types::range_reading::{AuthorClaim, ClaimConflict, ClaimKind, ClaimSource, FileRole, RangeReading, RoleReading, Unproven, UnprovenFile};
use pretty_assertions::assert_eq;

fn labelled(kind: ClaimKind, name: &str) -> AuthorClaim {
  AuthorClaim {
    kind,
    source: ClaimSource::Label { name: name.to_string() },
  }
}

fn titled(kind: ClaimKind, prefix: &str) -> AuthorClaim {
  AuthorClaim {
    kind,
    source: ClaimSource::Title { prefix: prefix.to_string() },
  }
}

/// openspec/specs/review/range-reading/spec.md, "The author's word"
#[test]
fn the_word_of_the_author_is_a_label_or_a_conventional_type_of_a_closed_list() {
  let labels = |names: &[&str]| names.iter().map(ToString::to_string).collect::<Vec<_>>();
  let cases: Vec<(&str, &str, Vec<String>, Option<AuthorClaim>)> = vec![
    (
      "a label of the list",
      "Extract the reader",
      labels(&["refactoring"]),
      Some(labelled(ClaimKind::Refactor, "refactoring")),
    ),
    (
      "labels are case-insensitive",
      "Clean up",
      labels(&["bug", "Cleanup"]),
      Some(labelled(ClaimKind::Refactor, "Cleanup")),
    ),
    ("tests by a label", "More cases", labels(&["Tests"]), Some(labelled(ClaimKind::Tests, "Tests"))),
    ("docs by a label", "Explain", labels(&["documentation"]), Some(labelled(ClaimKind::Docs, "documentation"))),
    (
      "a label goes before the type of the title",
      "docs: explain",
      labels(&["test"]),
      Some(labelled(ClaimKind::Tests, "test")),
    ),
    (
      "a conventional type",
      "refactor(diff): what is left to read",
      vec![],
      Some(titled(ClaimKind::Refactor, "refactor")),
    ),
    ("a breaking type", "refactor!: drop the reader", vec![], Some(titled(ClaimKind::Refactor, "refactor"))),
    ("tests by the type", "test(e2e): the stack window", vec![], Some(titled(ClaimKind::Tests, "test"))),
    ("docs by the type", "docs: the reading", vec![], Some(titled(ClaimKind::Docs, "docs"))),
    ("a fix says nothing", "fix(policy): a merged robot", labels(&["bug"]), None),
    ("a feature says nothing", "feat(review): Review Stack", vec![], None),
    ("a word before a colon is no type of the list", "proxy: route the reads", vec![], None),
    ("the words after the type say nothing", "Move the reader; unchanged", vec![], None),
    ("a type without its colon and space is none", "refactor:the reader", vec![], None),
    ("no label, no type", "Extract the reader", vec![], None),
  ];
  for (name, title, labels, expected) in cases {
    assert_eq!(claim_of(title, &labels), expected, "{name}");
  }
}

fn role(role: FileRole, files_to_read: u32, to_read: u32) -> RoleReading {
  RoleReading {
    role,
    files: files_to_read.max(1),
    files_to_read,
    changes: to_read,
    to_read,
  }
}

/// openspec/specs/review/range-reading/spec.md, "The author's word is checked against the reading"
#[test]
fn the_word_is_wrong_only_when_the_diff_leaves_changes_to_read_outside_its_role() {
  let reading = |roles: Vec<RoleReading>| RangeReading { roles, ..RangeReading::default() };
  let refactor = labelled(ClaimKind::Refactor, "refactor");
  let tests = titled(ClaimKind::Tests, "test");
  let docs = labelled(ClaimKind::Docs, "docs");
  let conflict = |claim: &AuthorClaim, to_read, files| {
    Some(ClaimConflict {
      claim: claim.clone(),
      to_read,
      files,
    })
  };
  let cases: Vec<(&str, AuthorClaim, RangeReading, Option<ClaimConflict>)> = vec![
    (
      "a refactoring whose code is all proven holds, tests to read or not",
      refactor.clone(),
      reading(vec![role(FileRole::Code, 0, 0), role(FileRole::Tests, 1, 2)]),
      None,
    ),
    (
      "a refactoring that leaves code to read",
      refactor.clone(),
      reading(vec![role(FileRole::Code, 2, 3), role(FileRole::Tests, 1, 2)]),
      conflict(&refactor, 3, 2),
    ),
    (
      "tests that change code",
      tests.clone(),
      reading(vec![role(FileRole::Code, 1, 1), role(FileRole::Tests, 2, 4), role(FileRole::Build, 1, 1)]),
      conflict(&tests, 2, 2),
    ),
    ("tests alone hold", tests, reading(vec![role(FileRole::Tests, 2, 4)]), None),
    (
      "docs that change a test",
      docs.clone(),
      reading(vec![role(FileRole::Prose, 1, 2), role(FileRole::Tests, 1, 1)]),
      conflict(&docs, 1, 1),
    ),
    ("docs alone hold", docs, reading(vec![role(FileRole::Prose, 1, 2)]), None),
  ];
  for (name, claim, reading, expected) in cases {
    assert_eq!(check(claim, &reading), expected, "{name}");
  }

  // a rename that waits for the index may be passed by its answer: no verdict on the word until then
  let waiting = RangeReading {
    roles: vec![role(FileRole::Code, 1, 2)],
    unproven: vec![UnprovenFile {
      path: "Report.kt".to_string(),
      role: FileRole::Code,
      why: Unproven::RenamesPending,
    }],
    proofs: vec![],
  };
  assert_eq!(check(refactor, &waiting), None, "a pending rename");
}
