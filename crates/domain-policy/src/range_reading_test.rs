use crate::range_reading::{ReadAheadOf, reading_summary, reading_view, request_readings, stack_summary};
use domain_types::fixtures::review_request;
use domain_types::range_reading::{
  AuthorClaim, ClaimConflict, ClaimKind, ClaimSource, FileRole, QuietProof, RangeReading, ReadingSummary, ReadingVerdict, RequestReading, RequestReadingState, RoleReading,
  StackReading, StackReadingState, StackRowReading, Unproven, UnprovenFile,
};
use domain_types::review_request::{RequestRow, ReviewRequest};
use pretty_assertions::assert_eq;
use std::collections::HashMap;

/// `files` files of `role`, `files_to_read` of them with `to_read` of their `changes` to read
fn role(role: FileRole, files: u32, files_to_read: u32, changes: u32, to_read: u32) -> RoleReading {
  RoleReading {
    role,
    files,
    files_to_read,
    changes,
    to_read,
  }
}

fn unproven(path: &str, role: FileRole, why: Unproven) -> UnprovenFile {
  UnprovenFile {
    path: path.to_string(),
    role,
    why,
  }
}

fn reading(roles: Vec<RoleReading>, unproven: Vec<UnprovenFile>, proofs: Vec<QuietProof>) -> RangeReading {
  RangeReading { roles, unproven, proofs }
}

fn summary(verdict: ReadingVerdict, unproven: u32, pending: bool) -> ReadingSummary {
  ReadingSummary { verdict, unproven, pending }
}

/// openspec/specs/review/range-reading/spec.md, "The verdict of a reading"
#[test]
fn a_reading_is_nothing_to_read_only_when_no_change_is_left_and_every_file_is_proven() {
  let formatting = vec![QuietProof::Formatting, QuietProof::Renames, QuietProof::Imports, QuietProof::Moves];
  let cases: Vec<(&str, RangeReading, ReadingSummary)> = vec![
    (
      "every change passed: nothing to read, with the proofs that passed them",
      reading(vec![role(FileRole::Code, 3, 0, 7, 0), role(FileRole::Tests, 1, 0, 2, 0)], vec![], formatting.clone()),
      summary(ReadingVerdict::NothingToRead { proofs: formatting }, 0, false),
    ),
    (
      "no changed file: nothing to read, no proof",
      reading(vec![], vec![], vec![]),
      summary(ReadingVerdict::NothingToRead { proofs: vec![] }, 0, false),
    ),
    (
      "a binary file changed: it cannot be proven, so the range is no longer nothing to read",
      reading(
        vec![role(FileRole::Code, 2, 0, 3, 0), role(FileRole::Resources, 1, 0, 0, 0)],
        vec![unproven("icon.png", FileRole::Resources, Unproven::Binary)],
        vec![QuietProof::Formatting],
      ),
      summary(ReadingVerdict::ToRead { to_read: 0, files: 0 }, 1, false),
    ),
    (
      "only tests are left to read",
      reading(vec![role(FileRole::Code, 2, 0, 4, 0), role(FileRole::Tests, 2, 2, 5, 3)], vec![], vec![QuietProof::Renames]),
      summary(ReadingVerdict::TestsOnly { to_read: 3, files: 2 }, 0, false),
    ),
    (
      "only prose is left to read",
      reading(vec![role(FileRole::Prose, 1, 1, 2, 2)], vec![], vec![]),
      summary(ReadingVerdict::DocsOnly { to_read: 2, files: 1 }, 0, false),
    ),
    (
      "code and tests are left: what is left, of every role",
      reading(
        vec![role(FileRole::Code, 4, 3, 9, 6), role(FileRole::Tests, 1, 1, 1, 1)],
        vec![],
        vec![QuietProof::Formatting],
      ),
      summary(ReadingVerdict::ToRead { to_read: 7, files: 4 }, 0, false),
    ),
    (
      "tests are left and a file of code cannot be proven: not tests only",
      reading(
        vec![role(FileRole::Code, 1, 1, 1, 1), role(FileRole::Tests, 1, 1, 1, 1)],
        vec![unproven("build.sh", FileRole::Code, Unproven::NoGrammar)],
        vec![],
      ),
      summary(ReadingVerdict::ToRead { to_read: 2, files: 2 }, 1, false),
    ),
    (
      "a rename waits for the index: its places are left to read, and it is no file that cannot be proven",
      reading(
        vec![role(FileRole::Code, 1, 1, 2, 2)],
        vec![unproven("Report.kt", FileRole::Code, Unproven::RenamesPending)],
        vec![],
      ),
      summary(ReadingVerdict::ToRead { to_read: 2, files: 1 }, 0, true),
    ),
    (
      "the index could not prove a rename: the file cannot be proven",
      reading(
        vec![role(FileRole::Code, 1, 1, 2, 2)],
        vec![unproven("Report.kt", FileRole::Code, Unproven::RenamesUnanswered)],
        vec![],
      ),
      summary(ReadingVerdict::ToRead { to_read: 2, files: 1 }, 1, false),
    ),
  ];
  for (name, reading, expected) in cases {
    assert_eq!(reading_summary(&reading), expected, "{name}");
  }
}

/// openspec/specs/review/range-reading/spec.md, "The author's word is checked against the reading"
#[test]
fn the_view_of_a_range_flags_the_word_of_its_author_only_where_the_diff_proves_it_wrong() {
  let code_left = reading(vec![role(FileRole::Code, 2, 1, 3, 2), role(FileRole::Tests, 1, 1, 1, 1)], vec![], vec![QuietProof::Renames]);
  let refactoring = vec!["refactoring".to_string()];
  let view = reading_view(code_left.clone(), Some("Extract the reader"), &refactoring);
  assert_eq!(
    view.conflict,
    Some(ClaimConflict {
      claim: AuthorClaim {
        kind: ClaimKind::Refactor,
        source: ClaimSource::Label { name: "refactoring".to_string() },
      },
      to_read: 2,
      files: 1,
    }),
    "a refactoring that leaves code to read",
  );
  assert_eq!(view.summary, reading_summary(&code_left), "the verdict is the reading's");
  assert_eq!(reading_view(code_left.clone(), Some("fix: the reader"), &[]).conflict, None, "a fix is never checked");
  assert_eq!(reading_view(code_left, None, &refactoring).conflict, None, "a branch or a commit has no author's word");
}

/// openspec/specs/review/range-reading/spec.md, "A stack: the layers with nothing to read fold"
#[test]
fn a_stack_counts_its_layers_and_opens_on_the_first_one_to_read_once_those_below_are_known() {
  let nothing = summary(
    ReadingVerdict::NothingToRead {
      proofs: vec![QuietProof::Formatting],
    },
    0,
    false,
  );
  let some = summary(ReadingVerdict::ToRead { to_read: 4, files: 2 }, 0, false);
  let tests = summary(ReadingVerdict::TestsOnly { to_read: 1, files: 1 }, 0, false);
  let cases: Vec<(&str, Vec<Option<&ReadingSummary>>, StackReading)> = vec![
    (
      "ten layers with nothing to read and two to read: the first of those",
      [vec![Some(&nothing); 4], vec![Some(&some)], vec![Some(&nothing); 6], vec![Some(&tests)]].concat(),
      StackReading {
        nothing_to_read: 10,
        to_read_layers: 2,
        to_read: 5,
        unknown: 0,
        first_to_read: Some(4),
      },
    ),
    (
      "a layer below is not read yet: no first layer to open on",
      vec![Some(&nothing), None, Some(&some)],
      StackReading {
        nothing_to_read: 1,
        to_read_layers: 1,
        to_read: 4,
        unknown: 1,
        first_to_read: None,
      },
    ),
    (
      "every layer has nothing to read",
      vec![Some(&nothing), Some(&nothing)],
      StackReading {
        nothing_to_read: 2,
        ..StackReading::default()
      },
    ),
  ];
  for (name, layers, expected) in cases {
    assert_eq!(stack_summary(&layers), expected, "{name}");
  }
}

/// openspec/specs/repository/review-requests/spec.md, "Read ahead: Each row says what its request leaves to read": a
/// request being read is checking, a read one its view with the word of its author checked, one the read ahead could not
/// read nothing; a stack is checking while a layer is, the sum of its layers once every one is read, and nothing while a
/// layer could not be read
#[test]
fn the_waiting_requests_say_what_they_leave_to_read_and_a_stack_the_sum_of_its_layers() {
  let formatting = reading(vec![role(FileRole::Code, 1, 0, 1, 0)], vec![], vec![QuietProof::Formatting]);
  let body = reading(vec![role(FileRole::Code, 1, 1, 1, 1)], vec![], vec![]);
  let labelled = |number: u32, labels: &[&str]| ReviewRequest {
    labels: labels.iter().map(|label| (*label).to_string()).collect(),
    ..review_request(number, 100)
  };
  let requests = vec![
    labelled(1, &[]),
    labelled(2, &["refactoring"]),
    labelled(3, &[]),
    labelled(4, &[]),
    labelled(5, &[]),
    labelled(6, &[]),
  ];
  let known: HashMap<u32, ReadAheadOf<'_>> = HashMap::from([
    (1, ReadAheadOf::Read(&formatting)),
    (2, ReadAheadOf::Read(&body)),
    (3, ReadAheadOf::Checking),
    (5, ReadAheadOf::Read(&formatting)),
  ]);
  let rows = vec![
    RequestRow::Stack { layers: vec![1, 2] },
    RequestRow::Stack { layers: vec![3, 1] },
    RequestRow::Stack { layers: vec![4, 5] },
    RequestRow::Single { number: 6 },
  ];
  let view = request_readings(&requests, &rows, |request| known.get(&request.number).copied());
  let read = |number: u32, of: &RangeReading, labels: &[String]| RequestReading {
    number,
    state: RequestReadingState::Read {
      view: reading_view(of.clone(), Some(&format!("Change {number}")), labels),
    },
  };
  assert_eq!(
    view.requests,
    vec![
      read(1, &formatting, &[]),
      read(2, &body, &["refactoring".to_string()]),
      RequestReading {
        number: 3,
        state: RequestReadingState::Checking
      },
      read(5, &formatting, &[]),
    ],
    "#4 and #6 the read ahead could not read: nothing"
  );
  let RequestReadingState::Read { view: flagged } = &view.requests[1].state else {
    unreachable!()
  };
  assert!(flagged.conflict.is_some(), "a refactoring that changes a body is flagged in its row");
  assert_eq!(
    view.stacks,
    vec![
      StackRowReading {
        bottom: 1,
        state: StackReadingState::Read {
          summary: StackReading {
            nothing_to_read: 1,
            to_read_layers: 1,
            to_read: 1,
            unknown: 0,
            first_to_read: Some(1),
          }
        }
      },
      StackRowReading {
        bottom: 3,
        state: StackReadingState::Checking
      },
    ],
    "the stack of #4 has a layer the read ahead could not read: no sum"
  );
}
