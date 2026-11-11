## ADDED Requirements

### Requirement: Views: H shows the old lines of the notes in view

In a review window H MUST show the old lines of every note in view that opens, and hide them again once they all
show, from the diffs and from the tree of the files; the view stays where it is and the focus where it was. A note
that is not in view MUST stay as it is. A note MUST name the key after its words, and to a screen reader, only where
the key works: the preview of a commit shows none.

#### Scenario: The key in the diffs and in the tree
- **GIVEN** a file with two wrapped calls, each a quiet run, below the first screen of the review
- **WHEN** the user presses H at the top of the review, shows the file, presses H twice, clicks the first note and presses H, then presses H with the focus on a row of the tree
- **THEN** the press at the top opens no note: both are closed once the file shows
- **AND** with both notes in view the first press shows the old line of each call, the second hides them, the diffs keeping the focus
- **AND** with one note open the press opens the other, and the press from the tree closes both, the row keeping the focus
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`H shows the old lines of the notes in view and hides them again, from the diffs and from the tree of the files`)

#### Scenario: The hint of the key
- **WHEN** a note that opens shows in a review window, and in the preview of a commit
- **THEN** the note of the review names the key H, the note of the preview names none
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`H shows the old lines of the notes in view and hides them again, from the diffs and from the tree of the files`, `Inline shows a wrapped call once, played down after its note, and a click on the note shows and hides its old line`)

#### Scenario: The rows of the notes
- **WHEN** the rows of a file with two quiet runs are built, the first note closed and opened, and in Unified
- **THEN** each note that opens has its row, the second one a row lower once the first is open, and a layout that hides no line has none
  [@test] src/lib/diff/rows.test.ts (`keeps every line of a quiet run in Unified and Split, played down, after a note that opens nothing`)
  [@test] src/lib/diff/block-parts.test.ts (`counts the rows of each part: a note a row, a quiet run of Inline its new lines until its note is opened`)

### Requirement: Views: The key of the notes is the key of no action of the window

The key of the notes MUST be the key of no action of a window that holds a review, as every key of the review. With
a note in view O MUST still open the pull request and change no note, and H MUST open nothing.

#### Scenario: The key of the pull request beside the key of the notes
- **GIVEN** the review window of a branch that has a pull request, or of a pull request under review, with two notes in view
- **WHEN** the user presses O and then H in the diffs, then O and H on a row of the tree of the files
- **THEN** each O opens the pull request and the notes stay as they were, closed the first time and open the second
- **AND** the first H shows the old lines of both notes, the second hides them, and neither opens the pull request
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`O opens the pull request of a reviewed branch while its notes are in view, and H shows and hides their old lines`, `O opens a pull request under review on GitHub while its notes are in view, and H shows and hides their old lines`)

#### Scenario: No key of an action
- **WHEN** the keys of the review are compared with the keys that the actions of a branch and of a pull request name in a review window
- **THEN** no letter is both, and the review answers a key by its physical key alone
  [@test] src/lib/review-keys.test.ts (`takes no letter that an action of a window of the review names as its key: O stays the pull request's`, `answers the action of each of its keys by the physical key, and none for another key`)
