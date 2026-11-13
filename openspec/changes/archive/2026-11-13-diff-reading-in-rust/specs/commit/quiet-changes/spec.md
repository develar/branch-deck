## MODIFIED Requirements

### Requirement: Views: A file says how many of its changes are left to read

Where a file shows its counts (its row in the tree of the files, the header of its card in a review), it MUST say how
many of its blocks of changed lines are stops of N and P, where they are fewer than all: "10 to read", "Nothing to
read", in full in its hover and its description ("10 of 11 changes to read"). A card counts the stops of its section.
The count of a whole file MUST be Rust's (`DiffReading`), which the page words.

#### Scenario: Rust counts what is left to read
- **WHEN** the blocks of a diff are a block of the imports alone, a quiet run and a real change; every block a stop; a block the policy ignores; a block without a line; lines deleted alone
- **THEN** its reading is three changes, two to read, one past the imports; three of three; one change; no change; one of one
- **AND** the reading is sent with the content of a text and with each event that changes its starts or its blocks of the imports: each event of the stream of the `commit_marks` template brings the reading of its file as the page has it after the event, and Ledger.kt ends with five of its eleven changes to read, Report.kt with every change
- **AND** the marks the index answers come with their reading: the declaration alone is left to read of a renamed private function and its two calls
  [@test] crates/file-diffs/src/reading_test.rs (`the_reading_of_a_diff_counts_its_starts_among_its_blocks_of_changed_lines`, `every_event_of_the_stream_brings_the_reading_of_its_file_as_it_stands_then`, `the_marks_the_index_answers_come_with_what_is_left_to_read`)
  [@test] src/stores/file-diffs.test.ts (`shows each file as its content arrives, and the late scopes of a text keep the lines it expanded`, `takes the marks of a late marks event in place of those of the content, and keeps what a view opened of the file`)

#### Scenario: The row of a file
- **WHEN** the preview of the `commit_marks` template lists its files
- **THEN** Ledger.kt says how many of its eleven changes are left to read ("5 to read"), fewer than all, and in full in its hover and its description ("5 of 11 changes to read"): the wrapped call, the three lines of the renamed local and the two blocks its moved functions left are none of them
- **AND** Report.kt, whose every block is a stop, says nothing
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`the row of a file says how many of its changes are left to read, and nothing where every change is read`)
  [@test] src/lib/diff/view-files.test.ts (`takes what Rust says is left to read of a file where some change is quiet, and nothing where every change is read`, `counts no block without a line: the stand-in of a use a change left outside is no change`)

#### Scenario: The header of a card
- **WHEN** the review of the template, and of a file of quiet runs alone, shows its cards
- **THEN** the header of Ledger.kt and its row in the tree say the same count ("5 to read"), the header and the row of the other file "Nothing to read"
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`N and P stop where the reading of a block starts and pass the wrapped call; the header counts the same stops`, `N and P pass a file whose every change is a quiet run, which says that nothing of it is to read`)

#### Scenario: A rename that is quiet
- **GIVEN** the `commit_marks` template, whose renamed local is three quiet runs
- **WHEN** the preview lists its files, and the review shows its cards
- **THEN** the row of Ledger.kt, its row in the tree of the review and the header of its card say "5 to read", in full "5 of 11 changes to read": the three blocks of the name are no more left to read
  [@test] tests/e2e/commit-files/renamed-names.spec.ts (`Inline shows each line of a quiet rename once, the new name in its mark, and the one note of the rename shows and hides the old line of every site`)
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`N and P pass the lines of a renamed local once its runs are quiet, and the file counts three changes fewer to read`)

#### Scenario: A review with its plan
- **WHEN** a file has a quiet run and a block of the imports alone, in a review with its plan
- **THEN** neither is left to read, as N and P pass both there
  [@test] src/lib/diff/view-files.test.ts (`passes a block of the imports alone too in a review with its plan, as N and P do there`)

#### Scenario: The words
- **WHEN** ten of eleven changes, one of two, none of three and none of one are left to read
- **THEN** the count is "10 to read", "1 to read", "Nothing to read", and in full "10 of 11 changes to read", "None of the 3 changes needs reading", "The change needs no reading"
  [@test] src/presenters/diff-marks.test.ts (`says how many changes of a file are left to read, short beside its counts and in full for a hover`)
