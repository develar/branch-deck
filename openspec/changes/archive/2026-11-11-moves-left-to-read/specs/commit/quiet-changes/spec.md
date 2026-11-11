## MODIFIED Requirements

### Requirement: Views: A file says how many of its changes are left to read

Where a file shows its counts (its row in the tree of the files, the header of its card in a review), it MUST say how
many of its blocks of changed lines are stops of N and P, where they are fewer than all: "10 to read", "Nothing to
read", in full in its hover and its description ("10 of 11 changes to read"). A card counts the stops of its section.

#### Scenario: The row of a file
- **WHEN** the preview of the `commit_marks` template lists its files
- **THEN** Ledger.kt says how many of its eleven changes are left to read ("8 to read"), fewer than all, and in full in its hover and its description ("8 of 11 changes to read"): the wrapped call and the two blocks its moved functions left are none of them
- **AND** Report.kt, whose every block is a stop, says nothing
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`the row of a file says how many of its changes are left to read, and nothing where every change is read`)
  [@test] src/lib/diff/view-files.test.ts (`counts the stops of N and P among the changes of a file where some change is quiet, and nothing where every change is read`, `counts no block without a line: the stand-in of a use a change left outside is no change`)

#### Scenario: The header of a card
- **WHEN** the review of the template, and of a file of quiet runs alone, shows its cards
- **THEN** the header of Ledger.kt and its row in the tree say the same count ("8 to read"), the header and the row of the other file "Nothing to read"
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`N and P stop where the reading of a block starts and pass the wrapped call; the header counts the same stops`, `N and P pass a file whose every change is a quiet run, which says that nothing of it is to read`)

#### Scenario: A review with its plan
- **WHEN** a file has a quiet run and a block of the imports alone, in a review with its plan
- **THEN** neither is left to read, as N and P pass both there
  [@test] src/lib/diff/view-files.test.ts (`passes a block of the imports alone too in a review with its plan, as N and P do there`)

#### Scenario: The words
- **WHEN** ten of eleven changes, one of two, none of three and none of one are left to read
- **THEN** the count is "10 to read", "1 to read", "Nothing to read", and in full "10 of 11 changes to read", "None of the 3 changes needs reading", "The change needs no reading"
  [@test] src/presenters/diff-marks.test.ts (`says how many changes of a file are left to read, short beside its counts and in full for a hover`)
