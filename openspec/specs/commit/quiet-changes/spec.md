---
name: Quiet Changes
description: Requirements for the marks of a diff that say what a reviewer may pass; the runs of lines that changed in their formatting alone, proven by the tokens of both sides and quiet, the runs that changed in their comments alone, a label, a block cut into runs at the line boundaries both sides share, the line the reading of each block starts at, what is never marked, the question a block is asked before the tokens of its file are read, the limits by work, which the time of a comparison never changes, and what the views draw of the marks; a note before the lines of each run, a quiet run shown once in Inline and played down elsewhere, the key that shows its old lines, the stops of N and P, the count of what is left to read, the find, the copy and the quiet colour.
style: plain-1
targets:
  - crates/text-diff/src/model.rs
  - crates/text-diff/src/lcs/diff.rs
  - crates/file-diffs/src/marks.rs
  - crates/file-diffs/src/marks_runs.rs
  - crates/file-diffs/src/service.rs
  - crates/file-diffs/benches/marks_bench.rs
  - crates/test-utils/src/repo_template.rs
  - src/lib/diff/block-parts.ts
  - src/presenters/diff-marks.ts
  - src/components/diff/diff-painter.ts
  - src/components/diff/diff-view.css
  - src/lib/diff/layout.ts
  - src/lib/review-keys.ts
  - src/stores/file-diffs.ts
---

# Quiet Changes

Status: Implemented
Date: 2026-11-11

## Purpose

A reviewer reads every changed block of a diff, also the one that only wrapped a call over three lines or indented
forty lines inside a new `if`. Such lines can be passed, but only when they provably are no change of the program: a
reviewer who passes a quiet block that hid a change has reviewed nothing. This spec is what the diff says of its own
blocks, decided in Rust for every view: which lines changed in their formatting alone (quiet), which in their
comments alone (a label), and where the reading of each block starts; and what a view draws of it (the group Views):
a note before the lines of each run, a quiet run played down, the stops of N and P and the count of what is left to
read. The page words what Rust decided and decides nothing. The marks of moved lines and renamed names are the next
stages of [Diff View](../../../../docs/design/diff-view.md#marks).

## Goals

- Quiet only by proof: the tokens of the lines are the same on both sides
  ([Equal by Tokens](../equal-by-tokens/spec.md)). Everything else is a label or nothing, and stays a stop.
- One source of where a reader stops, `starts`, for the navigation of changes and the count of what is left to read.
- The marks change no block: the scans, the patches and the review plan read the blocks they read before.
- The same marks of the same blocks on every machine and under every load: limits by work, never by time.
- A file that cannot hold a mark costs no parse.
- Every line that is hidden can be shown where it is: only Inline hides a line, the old form of a quiet run, behind
  a note that opens.
- No new setting: the marks are drawn in every layout as the layout shows lines.

## Non-goals

- A file of no grammar (Python, Go, YAML, Markdown): its block of whitespace stays the pale stop it is.
- A block with a side of no lines: an added comment is an added line.
- The one-sided diffs of a conflict: they have their starts and no runs.
- The marks in the patches of `brd` for agents.

## Concepts

- **A run** is a row of lines inside one block, with lines on both sides, that changed in one way alone
  (`formatting`, `comments`; `renamed` with its stage).
- **Quiet** is a run that provably changes no behaviour: what the navigation of changes passes and the Inline view
  shows by its new lines alone. Whether a kind is quiet is decided in one function (`is_quiet`); a run that is not
  quiet is a label.
- **A start** is the line the reading of a block starts at: the first line of the block that is in no quiet run.
- **A note** is a row of words between the lines of a diff that says what the lines after it changed in. **A stop**
  is where N and P land: the row of a start in the layout.

## Requirements

### Requirement: Marks: The marks are a field of a text diff and change no block

A text diff MUST carry its marks in a field of their own, `marks`: the runs of lines that changed in one way alone,
the moved lines, the renamed names and the starts, each list written when it is empty too. The marks MUST change no
block of the diff, and they MUST come with the content of a file, before its scopes: they change the rows a view
draws.

#### Scenario: The shape
- **WHEN** a text diff is written as JSON
- **THEN** `marks` holds `runs`, `moves`, `renames` and `starts`, in camel case, an empty list as `[]`
- **AND** a run is `oldStart`, `oldEnd`, `newStart`, `newEnd`, its `kind` (`formatting`, `comments`, `renamed`) and `quiet`
- **AND** nothing beside the tag of the content is named `kind` at the top of the diff
  [@test] crates/text-diff/src/model_test.rs (`the_marks_are_camel_case_json_with_every_field`)

#### Scenario: With the content
- **WHEN** the diffs of a commit stream, a call of one file wrapped over two lines
- **THEN** the content of the file has the run of the call and the starts, before the event of its scopes
- **AND** no later event brings a mark
  [@test] crates/file-diffs/src/service_test.rs (`the_marks_of_a_text_come_with_its_content`)

#### Scenario: The diff is the same from memory and from the store
- **WHEN** a diff is read again, from memory and from the store of another app
- **THEN** it is the diff of the first read, its marks too
  [@test] crates/file-diffs/src/service_test.rs (`a_diff_costs_one_list_and_one_cat_file_then_nothing`, `the_same_pair_read_by_two_grammars_is_two_diffs`)

### Requirement: Starts: The reading of a block starts at its first line outside its quiet runs

`starts` MUST name the line the reading of each block starts at, as `[block, side, line]`, in the order of the
blocks: the quiet runs cut a block into parts, and the reading starts at the first part with a line, at its first new
line (side 1), at its first old line (side 0) when it has no new one. A block the policy cut off and a block whose
every line is in a quiet run MUST have no entry: nothing to read. `starts` MUST be the one source of where a reader
stops.

#### Scenario: A diff without runs
- **WHEN** two texts are compared and nothing is marked
- **THEN** every block that counts has its start: its first new line, the first old line of deleted lines, a pale block too
- **AND** a block `IgnoreBlankLines` cut off has none
  [@test] crates/text-diff/src/model_test.rs (`a_diff_without_runs_starts_the_reading_at_each_block_that_counts`, `a_diff_starts_the_reading_at_each_block_that_counts`)

#### Scenario: Quiet runs in a block
- **WHEN** a block has quiet runs
- **THEN** its reading starts after a run at its start, at an old line that stands before the run of all its new lines, and nowhere when the runs leave no line
- **AND** a run that is a label moves no start
  [@test] crates/text-diff/src/model_test.rs (`the_reading_of_a_block_starts_after_its_quiet_runs`)

#### Scenario: Any blocks and runs
- **WHEN** any blocks have any runs in the order of both sides
- **THEN** a block has an entry exactly when it is not cut off and a line of it is in no quiet run, and the entry names the first such line of the first part that has one
  [@test] crates/text-diff/src/model_test.rs (`starts_agree_with_the_runs`)

### Requirement: Formatting: Lines with the same tokens on both sides are a quiet run

Lines of a block whose tokens are the same on both sides, the code and the comments
([Equal by Tokens](../equal-by-tokens/spec.md)), with a character that is no blank, MUST be a run of formatting, and
quiet. A change of the code MUST never be one, whatever a whitespace policy calls equal: a block that is pale under
the policy passes the same test, with no shortcut. Whether a kind of run is quiet MUST be decided in one place
(`is_quiet`).

#### Scenario: A call wrapped, lines indented again
- **WHEN** a call is wrapped inside its brackets in a file of Kotlin, a Kotlin script, Java, Rust, TypeScript, TSX, JavaScript, C# or Starlark, lines are indented again, or a space is put between the two `>` that close two lists of type arguments
- **THEN** the lines are one quiet run of formatting, a comment that moved with them and the blank lines between them too, and the block has no start
- **AND** a trailing comma added to the wrapped call is a token: no run
  [@test] crates/file-diffs/src/marks_test.rs (`lines_that_changed_in_their_formatting_alone_are_a_quiet_run`)

#### Scenario: What changes the code
- **WHEN** a block changes a space inside a string, `a - -b` into `a--b`, `>>` into `> >`, joins a `return` with the call of the next line, swaps two operands, adds spaces at the end of a line of a raw string, indents a statement of Starlark again, or renames a name
- **THEN** it has no quiet run and its reading starts at its first line
- **AND** so for a line of Starlark joined with the one its `\` continued, a label of Kotlin with a space before its `@`, a lambda joined with the name of the line before, a line break before the `=>` of an arrow function, a zero width space between two tokens of TypeScript, two spaces in the text of a JSX element, the text of a branch of `#if` that C# does not take, and two marks of the input of a Rust macro that join
- **AND** a blank line a text block of Java gains is a block of its own, a stop
  [@test] crates/file-diffs/src/marks_test.rs (`what_changes_the_code_is_never_formatting`)

#### Scenario: A block that is pale under the policy
- **WHEN** a block is equal under the `Ignore` policy
- **THEN** a call of Starlark wrapped inside its brackets is a quiet run
- **AND** a `return` joined with the call of the next line and a blank line a raw string gained stay stops
  [@test] crates/file-diffs/src/marks_test.rs (`a_pale_block_passes_the_same_test_of_tokens`)

#### Scenario: Nothing left to read
- **WHEN** files of every grammar are edited at random (indents, wrapped and joined lines, comments, blank lines, names) and compared with themselves
- **THEN** every run of formatting has the same tokens on both sides and is quiet as `is_quiet` says
- **AND** when no block of a diff that ignores nothing has a start, the two texts have the same tokens: the same program
  [@test] crates/file-diffs/src/marks_test.rs (`the_marks_of_edited_files_are_proven_by_their_tokens`)

### Requirement: Comments: Lines with the same code and other comments are a run that is a label

Lines of a block with the same tokens of the code on both sides and other comments, with lines on both sides, MUST be
a run of comments. It MUST never be quiet: a view labels it, and the reading stops at it.

#### Scenario: A comment worded again
- **WHEN** a block words a line comment again, a line of a doc comment, a line of a block comment that holds nothing but its words, a comment after the same code, a comment of Starlark, a documentation comment of C#, a comment of JSX, a comment of Razor in a C# file on its line and on a line inside it, or removes a comment from a line
- **THEN** the lines are a run of comments that is not quiet, and the reading of the block starts at it
- **AND** so is the line after a lone `\r` that a line comment of Rust runs over: the view ends a line there, the comment does not
- **AND** `//go:build` and `// go:build` are other comments
  [@test] crates/file-diffs/src/marks_test.rs (`lines_that_changed_in_their_comments_alone_are_a_run_that_is_a_label`, `what_changes_the_code_is_never_formatting`)

### Requirement: Runs: A block is cut at the line boundaries both sides share

A block of several changes MUST be cut where both sides have a line boundary, crossed by no token, with the same
tokens of the code before it. Between the cuts, the longest rows of lines whose tokens are the same are runs of
formatting, the blank lines inside them with them; lines between such rows that differ in their comments alone are a
run of comments. A run MUST have lines on both sides and lie inside one block.

#### Scenario: Lines wrapped into a block, a change beside lines indented again
- **WHEN** lines are wrapped into a new `if` and indented, or indented lines stand beside a line that really changed
- **THEN** the indented lines are a quiet run, and the reading of the block starts at the `if`, at the changed line, at a comment added between them, or at the old line removed between them
- **AND** a comment worded again among them is a run of comments of its own
- **AND** a comment that moved to a line of its own is in the run of formatting, and comments worded again around an added comment are one run of comments
  [@test] crates/file-diffs/src/marks_test.rs (`a_block_is_cut_into_runs_at_the_line_boundaries_both_sides_share`)

#### Scenario: No boundary both sides share
- **WHEN** a wrapped call and a changed call stand on one line of a side
- **THEN** the block has no run
  [@test] crates/file-diffs/src/marks_test.rs (`a_block_is_cut_into_runs_at_the_line_boundaries_both_sides_share`)

#### Scenario: The template of the E2E tests
- **WHEN** the commit of the `commit_marks` template is read
- **THEN** each of its cases is a block of its own: the wrapped call a quiet run, the comment a label, the moved lines, the renamed names and the real change blocks with a start
  [@test] crates/file-diffs/src/marks_test.rs (`the_marks_template_has_each_case_in_a_block_of_its_own`)

### Requirement: Runs: A run at an end of its block needs the code around the block to read the same

A run that reaches an end of its block MUST be a run only when the code around the block reads the same there: the
token before the block closes the same nodes, the token after it is the same token. Blank lines at an end of a block
MUST join the run beside them only then. At its other ends a run stands beside lines of its block that are no quiet
run, where the reader stops.

#### Scenario: The code before or after the lines is another
- **WHEN** the lines of a wrapped call are asked as a block of their own while the statement before them, or after them, is another on the two sides
- **THEN** they are no run
- **AND** with the lines that differ inside the block, the call is a run in it
  [@test] crates/file-diffs/src/marks_test.rs (`a_run_at_an_end_of_its_block_needs_the_code_around_the_block_to_read_the_same`)

### Requirement: Never marked: A side of no lines, blank lines alone and a block the policy cut off

A block with a side of no lines (added lines, deleted lines, an added comment), lines that are blank on both sides
and a block the policy cut off MUST have no run: a quiet run needs tokens that are the same on both sides. The first
two MUST stay stops.

#### Scenario: Added and deleted lines, blank lines
- **WHEN** a block adds lines, deletes lines, adds a comment on a line of its own, adds a blank line, or has blank lines alone on both sides
- **THEN** it has no run and its reading starts at its first line, under the policies that keep it a block
- **AND** a blank line `IgnoreBlankLines` cut off has no run and no start
  [@test] crates/file-diffs/src/marks_test.rs (`a_block_with_a_side_of_no_lines_or_of_blank_lines_alone_is_never_marked`)

### Requirement: Never marked: A pair without one grammar, a text without tokens and a side over the size of the inner ranges

Only a pair whose two sides are read by one grammar MUST have runs. A file of no grammar, a rename between two
grammars or to and from a file of none, a text without tokens and a pair with a side over the size the inner ranges
of a diff are compared for (20,000 lines, 1 MiB) MUST have none, and every block of them stays the stop it is: a file
without a grammar keeps its pale block of whitespace.

#### Scenario: No grammar, two grammars
- **WHEN** a text file, a Python file and a Go file change in their whitespace alone
- **THEN** they have no run, and the block stays a stop, pale under the policy that calls it equal
- **AND** a rename from `.kt` to `.kts`, from `.ts` to `.tsx`, from `.txt` to `.kt` and back is no pair of one grammar
  [@test] crates/file-diffs/src/marks_test.rs (`a_file_without_a_grammar_and_a_pair_of_two_grammars_have_no_runs`)

#### Scenario: No tokens, a side over the size
- **WHEN** a side has no tokens, the tokens count other lines than the diff, or the sides have more lines than the inner ranges are compared for
- **THEN** the diff has no run and every block its start, and the pair over the size reads no tokens
- **AND** the same change in texts of ten lines less is a run
  [@test] crates/file-diffs/src/marks_test.rs (`a_text_without_tokens_and_a_side_over_the_size_of_the_inner_ranges_have_no_runs`)

### Requirement: Limits: The time of a comparison changes no mark

The marks of a diff MUST be a function of its blocks, its two texts and their grammar, the same on every machine and
under every load. A diff the time cut short (`lineLevelOnly` within the size of the inner ranges) MUST have the
marks of its blocks: the runs read the blocks and the tokens, never the inner ranges.

#### Scenario: A diff the time cut short
- **WHEN** the inner ranges of a diff run out of their time, or are not asked for
- **THEN** the diff is by lines alone and has the runs and the starts of the diff in time, from the comparison and from memory
- **AND** a diff whose lines ran out of time too is one block of both texts, with the marks of that block
  [@test] crates/file-diffs/src/marks_test.rs (`a_diff_the_time_cut_short_has_the_marks_of_its_blocks`)
  [@test] crates/file-diffs/src/service_test.rs (`the_marks_of_a_diff_out_of_time_are_those_of_the_diff_in_time`)

### Requirement: Limits: The marks are bound by work, never by time

The limits of the marks MUST be of work, never of time. A block whose sides are the same code needs no comparison.
Else the tokens of its code are compared only within `MAX_BLOCK_TOKENS` a side, `MAX_TOKEN_EDITS` tokens that differ
and the work left of its file (`MAX_FILE_TOKEN_WORK`); a block over a limit MUST have no run cut out of it.

#### Scenario: A block over a limit
- **WHEN** the sides of a block differ in more tokens than the limit, a side has more tokens than the limit, or the work of the file is spent
- **THEN** no run is cut out of the block, and its reading starts at its first line
- **AND** the same block within the limits has its run, and lines of the same code are a run without a comparison
  [@test] crates/file-diffs/src/marks_test.rs (`a_block_over_a_limit_of_work_has_no_run_cut_out_of_it`)

#### Scenario: The comparison gives up by its edits
- **WHEN** two sequences differ in more elements than the edits allowed
- **THEN** the comparison answers "too big" after work bound by the edits, and within them the changes of the unbound comparison
  [@test] crates/text-diff/src/lcs/diff_test.rs (`the_changes_within_a_number_of_edits_are_the_changes_or_too_big`)

### Requirement: Before the tokens: A file whose blocks cannot hold a run is not parsed

A block MUST be asked a question of its characters before the tokens of its file are read: whether a line of one side
starts and a line ends as a line of the other, their blanks aside, or a comment may have a character in its lines.
Every block with a run answers yes, so the question MUST change no mark; a file whose blocks all answer no is not
parsed for its marks. The tokens of a blob MUST be read once per grammar and kept in memory.

#### Scenario: Blocks that cannot hold a run
- **WHEN** the blocks of a file change values and names, or add and delete lines
- **THEN** the tokens of neither side are read
- **AND** a wrapped call and a comment worded again read each side once, whatever the number of blocks
  [@test] crates/file-diffs/src/marks_test.rs (`a_file_whose_blocks_cannot_hold_a_run_is_not_parsed`)

#### Scenario: The question changes no answer
- **WHEN** the runs of a diff are read with the question and without it, for every case of the tests and for files of every grammar edited at random, with their own line ends, with `\r\n`, with lone `\r` among them and without the last one
- **THEN** the runs are the same
- **AND** every line a comment of a grammar is in answers that it may hold a comment: a line of a block comment, of a nested one, after a `/*/`, a `#!` line, an HTML-like comment of a script, a line of a comment of Razor in a C# file, the line after a lone `\r` that a line comment runs over
  [@test] crates/file-diffs/src/marks_test.rs (`the_marks_of_edited_files_are_proven_by_their_tokens`, `every_line_of_a_comment_may_hold_a_comment_before_the_tokens_are_read`)

#### Scenario: The tokens of a blob are kept
- **WHEN** a commit is read under another whitespace policy, and then the next commit of its file
- **THEN** only the blob no diff has met is parsed, and the others are read by the tokens memory has of them
  [@test] crates/file-diffs/src/service_test.rs (`the_tokens_of_a_blob_are_read_once_per_grammar`)

### Requirement: Views: A block shows part by part, a note before the lines of each run

A view MUST show a block in the order of both its sides, cut at its runs: the lines between two runs as the block
shows without marks, and each run after a row of words, its note, which says what the lines changed in and how many
they are ("Formatting only · 3 lines"). A run that lies in no block MUST change no row.

#### Scenario: A block of several parts
- **WHEN** a block adds a line, indents three lines again and adds another line, or has two runs with a changed line between them
- **THEN** each layout shows the added line, the note of the run, the lines of the run, then the lines after it
- **AND** the lines between two runs are rows of the block, each run has its own note
  [@test] src/lib/diff/rows.test.ts (`cuts a block at its runs in the order of both sides: the lines between them show as the block does, a run after its note`)
  [@test] src/lib/diff/block-parts.test.ts (`is one part of lines for a block without a run, in the shape of its layout`, `counts the rows of each part: a note a row, a quiet run of Inline its new lines until its note is opened`, `gives each row by its part, and the row of each line of a side`, `says the lines its rows cover, the lines a note stands for with them`)

#### Scenario: The pairs of Inline
- **WHEN** a quiet run lies between similar pairs of a block, or moves the lines after it apart on the two sides
- **THEN** Inline shows a pair as one row only where the engine pairs line i of both sides, and the other lines as Unified does
  [@test] src/lib/diff/rows.test.ts (`pairs the lines of a part of Inline only where the engine pairs line i of both sides`)
  [@test] src/lib/diff/block-parts.test.ts (`lists the pairs of lines that are one row each in Inline`)

#### Scenario: A run in no block
- **WHEN** the marks hold a run before every block, one that reaches past its block, or one after the last block
- **THEN** no row changes: every line of every block shows
  [@test] src/lib/diff/rows.test.ts (`leaves a run that lies in no block out, so every line of every block shows`)
  [@test] src/lib/diff/block-parts.test.ts (`gives each block its runs, and none to a run that lies in no block`)

#### Scenario: The words of a note
- **WHEN** a note words a run of formatting, of comments or of a renamed name
- **THEN** it says the kind ("Formatting only", "Comments only", "Renamed only") and the new lines of the run ("3 lines", "1 line")
  [@test] src/presenters/diff-marks.test.ts (`says what the lines of a run changed in and how far it reaches, by the new lines`)

### Requirement: Views: Inline shows a quiet run once, by its new lines

Inline MUST show the lines of a quiet run once: its new lines alone, after its note, in the quiet colour, neither
inserted nor unchanged, with no changed part in them. Its old lines have no row.

#### Scenario: A wrapped call
- **WHEN** Inline shows the call of the `commit_marks` template that the commit wraps over three lines
- **THEN** the three new lines show once after the note "Formatting only · 3 lines", quiet, with their new line numbers alone
- **AND** no row shows the old line, and no part of the three lines is marked as changed
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`Inline shows a wrapped call once, played down after its note, and a click on the note shows and hides its old line`)
  [@test] src/lib/diff/rows.test.ts (`shows a quiet run of Inline once, by its new lines after a note, and with its old lines once the note is opened`)
  [@test] src/lib/diff/row-cells.test.ts (`types a line that Inline shows once as its own cell, keeps the type of a quiet line elsewhere, and draws no range in a quiet line`)

### Requirement: Views: The note of a quiet run of Inline opens its old lines

The note of a quiet run of Inline MUST be a button: a click shows the old lines of the run before its new ones, all
of them quiet, and a second click hides them. ⌥-click (Alt-click) MUST open every such note of the file, on an open
note close them all. The diffs keep the focus, and a view keeps what it opened while the diff is kept, as the levels
of its folds.

#### Scenario: A click
- **WHEN** the user clicks the note of the wrapped call, then clicks it again
- **THEN** the old line shows before the three new ones, each of its type and quiet, and the note says it is expanded
- **AND** the second click hides the old line again
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`Inline shows a wrapped call once, played down after its note, and a click on the note shows and hides its old line`)
  [@test] src/presenters/diff-marks.test.ts (`names the button of a note that opens by what a press does to the old lines`)

#### Scenario: ⌥-click
- **GIVEN** a file with two wrapped calls, each a quiet run
- **WHEN** the user clicks the first note, ⌥-clicks the second, then ⌥-clicks the first
- **THEN** the click opens its note alone, the diffs keeping the focus
- **AND** the first ⌥-click opens every note of the file, the second closes them all
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`a click on a note opens it and ⌥-click opens and closes every note of its file`)

#### Scenario: Kept with the diff
- **WHEN** the notes a view opened are stored for its diff
- **THEN** they are read back by the diff, beside the levels of its folds
  [@test] src/lib/diff/rows.test.ts (`keeps the open notes of a diff beside the levels of its folds`)

#### Scenario: Kept as the scopes of the file come
- **GIVEN** a note opened, and a fold and a note of a section of a review, before the scopes of a text the caches lacked came
- **WHEN** the scopes come and the store replaces the content of the file
- **THEN** the note is as open as it was, and the section keeps its key, its fold and its note
  [@test] src/stores/file-diffs.test.ts (`keeps the notes a view opened across the late scopes of a text, and the folds and the notes of its sections`)

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

### Requirement: Views: Unified and Split keep every line of a quiet run, played down

Unified and Split MUST show every line of a quiet run, each of its type, in the quiet colour and with no changed
part; Split fills the shorter side of the run. Their note opens nothing.

#### Scenario: The wrapped call in Unified and Split
- **WHEN** the user switches the preview to Unified, then to Split
- **THEN** Unified shows the old line and the three new ones, deleted and inserted, all quiet; a block to read keeps its colour
- **AND** Split shows the old line beside the first new one and fills the old side beside the others
- **AND** the note is plain words in both
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`Unified and Split keep every line of a wrapped call, played down, after a note that opens nothing`)
  [@test] src/lib/diff/rows.test.ts (`keeps every line of a quiet run in Unified and Split, played down, after a note that opens nothing`)
  [@test] src/lib/diff/row-cells.test.ts (`types a line that Inline shows once as its own cell, keeps the type of a quiet line elsewhere, and draws no range in a quiet line`)

### Requirement: Views: A run that is a label has a note and is never played down

The lines of a run that is not quiet MUST show as the lines of their block, with its tints and its changed parts,
after a note that opens nothing: no layout hides a line of it.

#### Scenario: A comment worded again
- **WHEN** a view shows the comment of the `commit_marks` template that the commit words again
- **THEN** the note "Comments only · 1 line" is plain words, no button
- **AND** the comment is a similar pair of Inline, pale, its old words struck through before the new ones, never quiet
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`a run of comments is a label: its note opens nothing, its lines show as any change`)
  [@test] src/lib/diff/rows.test.ts (`labels a run of comments with a note and plays nothing of it down`)

### Requirement: Views: The words of a run that starts its block join the heading of the change

In a section of a review, a run that starts a block with a heading MUST say its words in the row of that heading,
after it, and open from there: no second row of words stands under the heading. A run after the first lines of the
block keeps its own note.

#### Scenario: A heading and a run
- **WHEN** a block with a heading starts with a quiet run, or holds one after its first line
- **THEN** the first has one row of words, the heading then the words of the run, which opens the old lines
- **AND** the second has the heading, the first line, then the note of the run
  [@test] src/lib/diff/rows.test.ts (`says the words of a run that starts its block in the heading of the change, one row of words`)

#### Scenario: A review with its plan
- **GIVEN** a function whose first statement is wrapped over three lines and whose result changes, one block, in a review with its plan
- **WHEN** the review shows the block, and the user clicks the words of the run
- **THEN** one row of words stands before it, "total — body changed · Formatting only · 3 lines"
- **AND** the click shows the old line of the statement, and no second row of words comes
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`a run that starts its block says its words in the heading of the change, and opens its old line from there`)

### Requirement: Views: An old line shown once is found at the note that stands for it

An old line of a quiet run that Inline shows once MUST resolve to the note of its run: the scroll position keeps a
note at the top by that line, and a reveal of a name in such a line opens the note, as it expands a fold, and selects
the name in the line. A row of words that stands for no line MUST stay at the top across a new layout of the same
rows.

#### Scenario: The row of an old line
- **WHEN** the row of an old line of a quiet run is asked, the note closed, opened, and in Unified
- **THEN** it is the note while Inline shows the run once, and the row of the line where it has one
  [@test] src/lib/diff/rows.test.ts (`gives the note that stands for an old line shown once, and the row of the line once the note is open`)

#### Scenario: The scroll position
- **WHEN** a note is at the top of the view and the rows are laid out again, the note opened, or in Unified
- **THEN** the top stays at the old line the note stood for
  [@test] src/lib/diff/layout.test.ts (`keeps a note of a quiet run at the top by the old line it stands for, and lays a note and a line shown once out as a row each`)

#### Scenario: A row of words that stands for no line
- **WHEN** the note of a run whose lines all show (Unified, Split, an opened note, a label) or the heading of a change is at the top of the view, and the same rows are laid out again
- **THEN** the top stays where it was: the row is held by the line of the row after it
- **AND** in a layout where the note stands for that line, or is gone, the top is the row of the line, never the row above it
  [@test] src/lib/diff/layout.test.ts (`keeps a row of words that stands for no line at the top across a new layout, by the row after it: a note of Unified and Split, an opened note, a label, a heading`)

#### Scenario: Back to a name of a hidden line
- **GIVEN** a jump from a name of the old line of the wrapped call, its note opened, then closed again
- **WHEN** the user goes Back
- **THEN** the note opens and the name is selected in the old line
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`Back to a name of an old line that a closed note stands for opens the note and selects the name in the line`)

### Requirement: Views: The find searches the old lines a note stands for

The find MUST search the old lines of a quiet run that Inline shows once, as hits of the old side before the hits of
its new lines. A step to such a hit lands on the note, which names the hits it holds ("1 match in the old line") and
marks the current one. Once the note is opened the same hits are in the old lines.

#### Scenario: The hits of a hidden line
- **WHEN** the query is in the old line of a quiet run, in Inline, the note closed and opened, and in Unified and Split
- **THEN** the hit is of the old side, before the hits of the new lines, at the row of the note while it stands for the line
  [@test] src/lib/diff/find.test.ts (`searches the old lines that Inline shows once, at the note that stands for them, before the new lines`, `finds the same hits by the rows of the old lines once the note is opened, and where every line stays`)
  [@test] src/presenters/diff-marks.test.ts (`names the hits in the old lines a note stands for`)

#### Scenario: A step lands on the note
- **GIVEN** a file of two wrapped calls below the first screen of the review
- **WHEN** the user finds a text only the old line of the first call holds, presses Enter, then opens the note
- **THEN** the step scrolls the view to the note, which names "1 match in the old line", marked as the current hit
- **AND** opened, the hit is marked in the old line, still the current one, and the note names none
- **AND** a text of both forms of the call is found at the note and in the new line
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`the find searches the old line a note stands for and lands on the note; opened, the hit is in the line`)

### Requirement: Views: A copy takes the lines the view shows

A copy MUST give the lines the rows show: of a quiet run of Inline its new lines, its old lines too once its note is
opened and where every line stays. A note gives no text.

#### Scenario: A selection over a quiet run
- **WHEN** the user selects from the note of the wrapped call to the end of its last line and copies, the note closed, then opened
- **THEN** the copy is the three new lines, then the old line and the three new ones
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`a copy takes what a quiet run shows: its new lines, and its old line once the note is opened`)
  [@test] src/lib/diff/copy-text.test.ts (`copies what a quiet run shows: its new lines alone in Inline, its old lines too once its note is opened or where every line stays`)

### Requirement: Views: A quiet line and a note are named to a screen reader

A line of a quiet run MUST say what it is to a screen reader after its type ("inserted, formatting only"; a line
shown once "formatting only"), as a changed line says its type. A note that opens MUST be a button named by its words
and by what a press does, with its expanded state.

#### Scenario: The wrapped call
- **WHEN** Inline shows the wrapped call
- **THEN** its lines carry the kind of their run, which the generated content of the line reads out
- **AND** the note is the button "Formatting only, 3 lines: Show the old line", not expanded; opened, "… Hide the old line", expanded
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`Inline shows a wrapped call once, played down after its note, and a click on the note shows and hides its old line`)

### Requirement: Views: N and P stop where the reading of a block starts

N and P MUST stop only at the starts of a diff: at the first row of lines of what is read from the line `starts`
names. Rust cuts a block by its quiet runs alone, so lines deleted before a label are read from the first of them,
and one column shows the old lines of a part first. A block without a start is passed, and so are a block of the
imports alone in a review with its plan and a block another section shows.

#### Scenario: The stops of the template
- **WHEN** the user presses N through the review of the `commit_marks` template, then P
- **THEN** N stops in Ledger.kt as many times as its header says is left to read, fewer than its eleven blocks, which hold the wrapped call, then three times in Report.kt and once in Store.kt
- **AND** each stop is a changed line four rows below the header of its file, after a row that is none, further down than the stop before it; no line of the wrapped call is one
- **AND** P goes back through the same stops, each at the row N showed
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`N and P stop where the reading of a block starts and pass the wrapped call; the header counts the same stops`)

#### Scenario: A file of quiet runs alone
- **GIVEN** a file whose two changes are wrapped calls, between a file changed before it and one after it
- **WHEN** the user presses N, then P
- **THEN** N goes from the first file to the third, and P back, past the file
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`N and P pass a file whose every change is a quiet run, which says that nothing of it is to read`)

#### Scenario: The row of a stop
- **WHEN** a block starts at an added line before a quiet run, at a deleted line after one, at a run of comments, or is quiet as a whole
- **THEN** the stop is the first row of lines since the last quiet run before that line: an old line where one column shows the old lines of the part first
- **AND** a block of quiet lines alone has no stop, and a start that names no line of its block stops at the last row of the block
  [@test] src/lib/diff/rows.test.ts (`stops N and P where Rust starts the reading of a block, at the first row of that part, never at a block of quiet lines alone`, `starts N and P at each block of changed lines, a deleted one too, never at a block the policy ignores`, `plays down a block of the imports alone in a review: pale, and N and P pass it; elsewhere it is a change as any`)
  [@test] src/lib/diff/block-parts.test.ts (`reads the stops as Rust gives them, and counts them against the blocks of changed lines`, `gives each row by its part, and the row of each line of a side`)

#### Scenario: Lines deleted before a label
- **GIVEN** a block that deletes six lines and words the comment of the line after them again, a run of comments, so Rust names the comment, the first new line of the block
- **WHEN** the rows are built in each layout, and the user presses N in the review
- **THEN** the stop is the first deleted line, where the block stopped before it had a label, in Inline, Unified and Split, also under a heading and after a quiet run
- **AND** N shows the first deleted line four rows below the header of its file, the deletion in view from its start
  [@test] src/lib/diff/rows.test.ts (`stops N and P where Rust starts the reading of a block, at the first row of that part, never at a block of quiet lines alone`)
  [@test] src/lib/diff/block-parts.test.ts (`starts the reading of a block at its first row of lines to read, in every layout, whatever labels cut it`)
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`N stops at the first line a block deletes before a comment worded again, never at the comment below them`)

#### Scenario: A file without a grammar
- **WHEN** a file no grammar reads indents two lines again, between a file changed before it and one after it
- **THEN** its lines are pale with the indentation in the full colour, without a note or a quiet line, and N stops at them
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`stays the pale stop it was: no note, no quiet line, and N stops at it`)

### Requirement: Views: A file says how many of its changes are left to read

Where a file shows its counts (its row in the tree of the files, the header of its card in a review), it MUST say how
many of its blocks of changed lines are stops of N and P, where they are fewer than all: "10 to read", "Nothing to
read", in full in its hover and its description ("10 of 11 changes to read"). A card counts the stops of its section.

#### Scenario: The row of a file
- **WHEN** the preview of the `commit_marks` template lists its files
- **THEN** Ledger.kt says how many of its eleven changes are left to read ("10 to read"), fewer than all, and in full in its hover and its description ("10 of 11 changes to read")
- **AND** Report.kt, whose every block is a stop, says nothing
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`the row of a file says how many of its changes are left to read, and nothing where every change is read`)
  [@test] src/lib/diff/view-files.test.ts (`counts the stops of N and P among the changes of a file where some change is quiet, and nothing where every change is read`, `counts no block without a line: the stand-in of a use a change left outside is no change`)

#### Scenario: The header of a card
- **WHEN** the review of the template, and of a file of quiet runs alone, shows its cards
- **THEN** the header of Ledger.kt and its row in the tree say the same count ("10 to read"), the header and the row of the other file "Nothing to read"
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`N and P stop where the reading of a block starts and pass the wrapped call; the header counts the same stops`, `N and P pass a file whose every change is a quiet run, which says that nothing of it is to read`)

#### Scenario: A review with its plan
- **WHEN** a file has a quiet run and a block of the imports alone, in a review with its plan
- **THEN** neither is left to read, as N and P pass both there
  [@test] src/lib/diff/view-files.test.ts (`passes a block of the imports alone too in a review with its plan, as N and P do there`)

#### Scenario: The words
- **WHEN** ten of eleven changes, one of two, none of three and none of one are left to read
- **THEN** the count is "10 to read", "1 to read", "Nothing to read", and in full "10 of 11 changes to read", "None of the 3 changes needs reading", "The change needs no reading"
  [@test] src/presenters/diff-marks.test.ts (`says how many changes of a file are left to read, short beside its counts and in full for a hover`)

### Requirement: Views: The rows of the marks keep the layout arithmetic

A note MUST be one row, and a line shown once as many visual lines as its new line takes, so every offset of the
view is known before a paint. The one column of Inline MUST scroll sideways no further for a pair that a quiet run
shows once.

#### Scenario: Heights
- **WHEN** a quiet run of Inline is laid out, with soft wraps off and on
- **THEN** its note and each of its lines are a row, a wrapped line as many visual lines as its new line
  [@test] src/lib/diff/layout.test.ts (`keeps a note of a quiet run at the top by the old line it stands for, and lays a note and a line shown once out as a row each`)

#### Scenario: The widest row of Inline
- **WHEN** a pair of a block lies in a quiet run
- **THEN** the widest row of Inline counts the row of the other pairs alone
  [@test] src/lib/diff/inline-rows.test.ts (`counts no row of a pair in a quiet run, which Inline shows once by its new line`)

### Requirement: Views: The quiet colour is one token of both themes

A line of a quiet run and the note before it MUST be drawn in one colour of their own, the token `--diff-quiet` of
each theme: no colour of a change, weaker than each of them, with the number of the line readable on it.

#### Scenario: The light and the dark theme
- **WHEN** the wrapped call shows in the light theme, and in the dark one
- **THEN** its lines and its note have the colour the token names, light in the light theme and dark in the dark one
- **AND** an inserted line and an unchanged one have another
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`a quiet line and its note have the quiet colour of the light theme, which is no colour of a change`, `a quiet line and its note have the quiet colour of the dark theme, which is no colour of a change`)

## User Experience

- A note: the kind in `font-medium`, then ` · ` and the new lines of the run, in the font of the text, muted, at the
  size of the code: "Formatting only · 3 lines", "Comments only · 1 line". Its words start where the code of the
  first column does. Before the lines of a quiet run the row has the quiet colour, as they have.
- A note that opens (Inline, a quiet run) is a button with a chevron in the room of the line numbers, pointing right
  while the old lines are hidden and down while they show; its words turn to the foreground under the pointer. Its
  name and hover: "Show the old line", "Show the 40 old lines", "Hide the old line". A click toggles it, ⌥-click
  (Alt-click) every such note of the file. It is not in the Tab order, as a fold.
- H in a review window (the hidden lines): the old lines of the notes in view show, or go again once they all show.
  Such a note shows the key after its words, as the key of the line at the end of a card (`Kbd`: `bg-muted`,
  `text-xs font-medium`, muted), and names it to a screen reader (`aria-keyshortcuts`); in the preview of a commit,
  which has no such key, no note shows one. O stays the pull request's in every review window.
- A quiet line: `--diff-quiet` over the line and its number ([UI Theme](../../../../docs/design/ui-theme.md#diffs)),
  the text in its syntax colours, no changed part. To a screen reader: "formatting only" after the type of the line.
- The hits of the find in the old lines a note stands for, after its words, marked as a hit of a line: "1 match in
  the old line", "3 matches in the old lines"; the current hit as the current hit of a line.
- The count to read, before the `+/−` counts of a file in the tree of the files and in the header of its card, muted
  `text-xs`: "10 to read", "Nothing to read"; its hover and the description of the row: "10 of 11 changes to read",
  "None of the 3 changes needs reading", "The change needs no reading".
- N and P: the next and the previous stop ([Commit Review](../commit-review/spec.md#requirement-the-sidebar-and-the-diffs-n-and-p-show-the-next-and-the-previous-change)).

## Testing / Local Run

`pnpm test:unit src/lib/diff src/stores/file-diffs.test.ts src/presenters/diff-marks.test.ts` and
`pnpm e2e tests/e2e/commit-files/quiet-changes.spec.ts` for the views.
`cargo nextest run -p file-diffs -E 'test(/marks_test/)'` and `-p text-diff -E 'test(/model_test/)'`.
`MARKS_EDITS=600 cargo nextest run --release -p file-diffs -E 'test(the_marks_of_edited_files)'` edits each file of
the corpus 600 times (12 by default). The E2E template of the marks is `commit_marks` (`crates/test-utils/src/repo_template.rs`):
`src/Ledger.kt`, `src/Report.kt` and `src/Store.kt` with a case of every kind of mark, each in a block of its own.

## Measurements

`cargo bench -p file-diffs --bench marks_bench`: the tokens of a text, `marks_of` of a pair, a block at its limits and
a subject of 200 files through the service; the numbers and what they decide are in
[Diff View](../../../../docs/design/diff-view.md#marks).

## Open Questions / Risks

- Quiet is switched on for formatting alone. A measurement on real ranges of a large repository (a sample of the
  quiet runs read by a person) may switch a kind off in `is_quiet`.
- A run inside a block is proven by its own tokens; the lines beside it changed and are read. A run at an end of
  its block is asked with the two tokens of the code around the block, not with the whole text
  ([Equal by Tokens](../equal-by-tokens/spec.md#requirement-equality-a-range-of-lines-is-asked-with-the-code-around-it)).
- The alignment of the tokens only finds where a block may be cut: a subsequence that pairs a token with another of
  its likes loses a run, never proves a wrong one.
- A comment line beside changed code with no token in common is no run of its own: nothing between two paired tokens
  can be cut.
- The blocks of a diff the time cut short may be coarser than those of the diff in time (the comparison by words
  splits a block the comparison by lines keeps whole); its marks are those of its blocks, and memory keeps it for the
  run ([Commit Files](../commit-files/spec.md#requirement-file-diffs-the-result-does-not-depend-on-the-load-of-the-machine)).

## References

- [Equal by Tokens](../equal-by-tokens/spec.md): the rule of the tokens
- [Commit Files](../commit-files/spec.md#file-diffs): the diffs, their stream and their caches
- [Diff View](../../../../docs/design/diff-view.md#marks) ("Marks", "The parts of a block"), [Text Diff Engine](../../../../docs/design/text-diff.md) ("Model")
- [Commit Review](../commit-review/spec.md): N, P and the find of a review window
