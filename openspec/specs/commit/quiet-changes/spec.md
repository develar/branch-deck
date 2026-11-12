---
name: Quiet Changes
description: Requirements for the marks of a diff that say what a reviewer may pass; the runs of lines that changed in their formatting alone, proven by the tokens of both sides and quiet, the runs that changed in their comments alone, a label, a block cut into runs at the line boundaries both sides share, the names renamed within a file and the lines that changed in such a name alone, proven by the bindings of both sides for a value of its body alone and labels until a measurement switches their kind on, the lines that moved within a file, exact, indented or with a few edited lines, never quiet, what the files of a subject say of each other (the lines that moved between two files, labels at both ends, the names one file declares and another uses, and what the index of the repository proves of them), the line the reading of each block starts at, what is never marked, the question a block is asked before the tokens of its file are read, the limits by work, which the time of a comparison never changes, and what the views draw of the marks; a note before the lines of each run, a quiet run shown once in Inline and played down elsewhere, the key that shows its old lines, the lines of a move shown once where they stand now with a note at each end that shows the other, the stops of N and P, the count of what is left to read, the find, the copy, the quiet colour and the colour of a move.
style: plain-1
targets:
  - crates/text-diff/src/model.rs
  - crates/text-diff/src/moves.rs
  - crates/text-diff/src/moves_across.rs
  - crates/text-diff/src/inline.rs
  - crates/text-diff/src/lcs/diff.rs
  - crates/file-diffs/src/marks.rs
  - crates/file-diffs/src/marks_runs.rs
  - crates/file-diffs/src/marks_renames.rs
  - crates/file-diffs/src/marks_subject.rs
  - crates/file-diffs/src/leftovers.rs
  - crates/file-diffs/src/accio.rs
  - crates/object-cache/src/file_diff.rs
  - crates/file-diffs/src/service.rs
  - crates/app-api/src/reads/diffs.rs
  - crates/app-api/src/session/windows.rs
  - crates/file-diffs/benches/marks_bench.rs
  - crates/test-utils/src/repo_template.rs
  - src/lib/diff/block-parts.ts
  - src/lib/diff/rows.ts
  - src/lib/diff/row-cells.ts
  - src/lib/diff/code-menu.ts
  - src/presenters/diff-marks.ts
  - src/components/diff/diff-painter.ts
  - src/components/diff/DiffView.tsx
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
comments alone (a label), which names the change renamed and which lines changed in such a name alone (a label;
proven for a value of its body alone, which a reader will pass once a measurement of real ranges switched the kind
on), which lines moved within the file (a label: where lines stand is behaviour), what the files of a subject say
of each other (the group Subject: the lines that moved from one file to another and the names one file declares and
another uses, labels, and what the index of the repository proves of such a name), and where the reading of each
block starts; and what a view draws of it (the group Views): a note before the lines of each run, a quiet run played
down, the lines of a move in a colour of their own with a note at each end that shows the other, the stops of N and P
and the count of what is left to read. The page words what Rust decided and decides nothing. A run of a renamed name
shows as any label: a note before its lines; the words of a rename ("Renamed total → sum · 12 sites" at its
declaration) are not drawn yet, and neither is anything of the marks of a subject but the runs it adds: the words
of a move between files ("Moved to Report.kt:12") and of a name another file declares ("Renamed … · declared in
Store.kt"), with what the index said of it, are the next stage of
[Diff View](../../../../docs/design/diff-view.md#marks).

## Goals

- Quiet only by proof: the tokens of the lines are the same on both sides
  ([Equal by Tokens](../equal-by-tokens/spec.md)), for a renamed name once its two spellings are read as one, which
  the bindings of both texts allow only for a value of its body alone ([Name Bindings](../name-bindings/spec.md)).
  And only of a kind a measurement has switched on: a sample of its proven runs of real ranges, read by a person,
  with no change of behaviour among them. The runs of a renamed name wait for theirs as labels. Everything else is a
  label or nothing, and stays a stop.
- One source of where a reader stops, `starts`, for the navigation of changes and the count of what is left to read.
- The marks change no block: the scans, the patches and the review plan read the blocks they read before.
- The same marks of the same blocks on every machine and under every load: limits by work, never by time.
- A file that cannot hold a run costs no parse, a file without a name that stands where another stood no parse of
  its names, and the moves of a file cost none at all.
- A function that moved is read once, where it stands now, with what changed in it on the way.
- Every line that is hidden can be shown where it is: only Inline hides a line, the old form of a quiet run and the
  lines a move took, each behind a note that opens.
- The two ends of a move are one step apart: the note at each end shows the other.
- No new setting: the marks are drawn in every layout as the layout shows lines.

## Non-goals

- A run in a file of no grammar (Python, Go, YAML, Markdown): its block of whitespace stays the pale stop it is. It
  has its moves.
- A move between two files of a commit, and a move that the comparison shows as one block: lines that cross no line
  that stays.
- A rename across files: a name the file does not declare is handed out unmarked, for the pass over the files of a
  change. The uses of a name that is not local stay labels until that pass confirms them. A name renamed in lines
  that moved.
- A block with a side of no lines: an added comment is an added line.
- The one-sided diffs of a conflict: they have their starts and no runs.
- The marks in the patches of `brd` for agents.

## Concepts

- **A run** is a row of lines inside one block, with lines on both sides, that changed in one way alone
  (`formatting`, `comments`, `renamed`).
- **A rename** is one binding of each text with the same places, under two names: a declaration inside a body with
  every name bound to it (`local`), else every name of a spelling that no body binds. **A place** of it is a name
  of the binding in each text; a place whose line changed in nothing else is in a run of the kind `renamed`.
- **Quiet** is a run that provably changes no behaviour: what the navigation of changes passes and the Inline view
  shows by its new lines alone. Whether a kind is quiet is decided in one function (`is_quiet`); a run that is not
  quiet is a label. **Proven** is a run whose lines are shown to be the same program (`has_proof`: every run of
  formatting, a run of a renamed name whose places are values of their bodies alone); a proven run is quiet once its
  kind is switched on; until then the pair names it (`PairMarks::proven`), and `PairMarks::as_proven` gives the
  marks as they are then.
- **A move** is a row of lines one block deletes and another block adds: the same lines by their text without the
  whitespace at their ends, with at most a quarter of edited lines between exact ones, and the blank lines that left
  with them counted beside them. Never quiet: the lines are read where they stand now, and are nothing to read where
  they left. Under the policy that counts every character, whitespace at an end of a line that changed otherwise
  than the move says is a range of an edited line.
- **A subject** is what a view shows the diffs of: a commit, a range, the working tree. **What a pair leaves** is
  what its diff cannot explain alone: the changed lines no mark took and the names it declares none of. **A move
  between files** is a row of exact lines one file of a subject deleted and another added: a label at both ends,
  never quiet, no stop taken. **A name another file declares** is a rename of the file that uses it, by the two
  spellings; **proven** it is when the index of the repository lists every use of its declaration in both trees and
  each is a place of the rename or a line read anyway (`confirmed`); else the marks name why not.
- **A start** is the line the reading of a block starts at: its first line that is in no quiet run and, of its old
  lines, that no move took.
- **A note** is a row of words between the lines of a diff that says what the lines after it changed in, or, at an
  end of a move, where its other end is. **A stop** is where N and P land: the row of a start in the layout.
- **The ends of a move**: where its lines left (its old lines, in the block that deletes them) and where they stand
  now (its new lines, in the block that adds them).

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
- **AND** a rename is `old`, `new`, `sites`, `local`, `declared`, `oldSites` and `newSites`
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
blocks: the quiet runs cut a block into parts, and the reading starts at the first part with a line to read, at its
first new line (side 1), at its first old line that no move took (side 0) when it has no new one. A block the policy
cut off and a block without a line to read MUST have no entry. `starts` MUST be the one source of where a reader
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

#### Scenario: The lines a move took
- **WHEN** a move took old lines of a block
- **THEN** the reading of the block starts at its first old line that no move took, at its new lines when it has some, and nowhere when the moves, or the moves and the quiet runs, leave no line
- **AND** the blank lines that left with a move are nothing to read either
- **AND** the block the lines stand in now starts at its first new line
  [@test] crates/text-diff/src/model_test.rs (`the_old_lines_of_a_move_are_nothing_to_read`)

#### Scenario: Any blocks and runs
- **WHEN** any blocks have any runs in the order of both sides, and moves took any of their old lines
- **THEN** a block has an entry exactly when it is not cut off and a line of it is to read, a new line in no quiet run or an old line in no quiet run that no move took, and the entry names the first such line of the first part that has one
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
- **THEN** each of its cases is a block of its own: the wrapped call a quiet run, the comment a label, the renamed names, the real change and the lines that left for another file blocks with a start
- **AND** `accounts` is a move that counts the blank line that moved with it, and `describe` a move indented by two columns with one edited line: the blocks they stand in have a start, the blocks they left have none
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

### Requirement: Renames: A binding whose every name changed alike is a rename

A rename MUST be one binding of each text with the same places: as many names on both sides, name `i` of the old text
in the block of name `i` of the new one and nowhere the alignment of the tokens put another name, the declarations at
the same places. A name bound inside a body is `local`; any other is every name of its spelling that no body binds.
A rename MUST name its two names, its places on both sides and which of them declare it.

#### Scenario: A local in each grammar
- **WHEN** a local of a function is renamed at its five places, in Kotlin, a Kotlin script, Java, C#, Starlark, Rust, TypeScript, TSX and JavaScript
- **THEN** the diff has one rename, `local`, of five places, declared at its first
- **AND** every changed line is in a run of the kind `renamed` that is proven: with the kind switched on, no block has a start
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_local_renamed_at_every_place_is_proven_in_each_grammar`)

#### Scenario: The places
- **WHEN** a local is renamed on a line with a character of two UTF-16 units before it, and its call is wrapped
- **THEN** each place is `[line, start, end]` of its text, in UTF-16 columns, place `i` of the old text beside place `i` of the new one, and `declared` names the first
  [@test] crates/file-diffs/src/marks_renames_test.rs (`the_places_of_a_rename_are_lines_and_columns_of_both_texts`)

#### Scenario: Two bindings of one spelling
- **WHEN** two loop variables of one name get two names, or the same name, or the locals of two functions take the name of each other
- **THEN** each binding is a rename of its own, with its own places
  [@test] crates/file-diffs/src/marks_renames_test.rs (`two_bindings_of_one_spelling_are_two_renames`)

#### Scenario: With the content
- **WHEN** the diffs of a commit that renames a local stream, and are read again from memory and from the store of another app
- **THEN** the content of the file has the rename and its three runs, labels with a start each, before the event of its scopes, the same at each read
  [@test] crates/file-diffs/src/service_test.rs (`the_renames_of_a_text_come_with_its_content`)

#### Scenario: The template of the marks
- **WHEN** the diffs of the `commit_marks` template are read
- **THEN** `Ledger.kt` has the rename of its local `total` with three places, each a run that is a label with its start, and `Store.kt` the rename of `openStore` with its declaration alone, which is read
  [@test] crates/file-diffs/src/marks_test.rs (`the_marks_template_has_each_case_in_a_block_of_its_own`)

### Requirement: Renames: A use left behind, a name taken and two names swapped are no rename

A change MUST be no rename, and mark nothing, when the old name still stands at a place of the binding, when a place
got a third name, when two bindings became one, or when the other name changes a binding of either text: it takes a
name of another declaration, a nearer declaration takes a name of this one, or the language binds it without a
declaration. Every block of such a change MUST stay a stop.

#### Scenario: A use left behind in each grammar
- **WHEN** a local is renamed at four of its five places, in each grammar
- **THEN** the diff has no rename and no run, and every block its start
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_local_with_a_use_left_behind_is_no_rename_in_each_grammar`)

#### Scenario: A name taken
- **WHEN** two locals of one body take the name of each other, each declared and read in a block of its own, in Kotlin, Java, C#, Starlark, TypeScript and Rust; a local gets the name of a parameter, a field or a constant its body reads; a nearer declaration of the new name takes a use; two locals become one name; a use gets a third name; the new name is `it`; a member keeps its old name at one use; one of two calls of a function declared elsewhere changes
- **THEN** nothing is a rename, and every block has its start
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_use_left_behind_a_name_taken_and_two_names_swapped_are_no_rename`)

### Requirement: Renames: Lines that changed in a renamed name alone are a run

The blocks that hold a place of a rename MUST be read again with the two names of each place read as one. Lines whose
tokens are then the same, comments too, and that hold a place MUST be a run of the kind `renamed`, however the lines
were wrapped. A place on a line with another change, or with a comment that changed, MUST be in no run: that line is
read.

#### Scenario: A line with another change
- **WHEN** a local is renamed at its five places and the line of one of them changes in an operator too, in each grammar
- **THEN** the rename has its five places, the line of the other change is in no run and starts the reading of its block, and the other lines are proven runs
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_place_on_a_line_with_another_change_is_read_in_each_grammar`)

#### Scenario: Wrapped lines
- **WHEN** a call with the renamed name is wrapped over four lines, the name stands in a string template, or a blank line is put between two lines of the name
- **THEN** the lines are one run of the kind `renamed`
  [@test] crates/file-diffs/src/marks_renames_test.rs (`lines_wrapped_with_a_renamed_name_are_one_run`)

#### Scenario: A comment that changed
- **WHEN** the comment at the end of the line of a place changes with the name
- **THEN** the line is in no run and is read; a comment on a line of its own is a run of comments beside the run of the rename, also beside such a line
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_comment_that_changed_beside_a_renamed_name_is_read`)

### Requirement: Renames: A run of a renamed name takes no line of formatting

Lines that are the same tokens with the names of a rename read as one and hold no place MUST be the run of formatting
they are without the rename, quiet whatever the rename is, with the blank lines beside them. The second reading of a
block MUST take no run: a run the first reading found MUST lie in a run of its kind, and a run of the second reading
that shares a line with it without holding it, or crosses it, is none.

#### Scenario: Lines of formatting beside the name
- **WHEN** a call is wrapped between or before the lines of a renamed local, two calls are wrapped each before a line of it, or twelve statements are indented inside a new `if`, one of them a use of the local
- **THEN** the wrapped calls and the indented lines are quiet runs of formatting, each row of lines of the name between them is a run of the kind `renamed`, and the reading of the last block starts at the `if`
- **AND** a blank line put before or after the wrapped call is in its run of formatting
- **AND** beside a rename that is a label for good (the use of a member, a parameter a call may name, a local of a C# file with an `#if`, a local of a text with a syntax error) the wrapped call is a quiet run of formatting too, and the reading of the block starts at the line of the name
  [@test] crates/file-diffs/src/marks_renames_test.rs (`lines_of_formatting_beside_a_renamed_name_are_a_run_of_their_own`)

#### Scenario: The two readings pair the lines otherwise
- **WHEN** a line with three places of a renamed local changes places with a call that is wrapped: the tokens of the texts pair the call, and with the names read as one the line of the places pairs with itself
- **THEN** the wrapped call is the quiet run of formatting the first reading found, and the line of the places is in no run and is read
- **AND** of the runs of two readings, a run of the second that holds a run of the first in its kind stays with it, and one that shares a line with a run of the first it does not hold, or crosses it, is none
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_run_of_the_first_reading_stays_where_the_second_aligns_the_lines_otherwise`)

#### Scenario: The runs of the first reading
- **WHEN** the files of the corpus are edited at random, or have a name renamed, and the blocks of each diff are read once more from the tokens of the texts as they are
- **THEN** every run that reading finds lies in a run of its kind of the marks
  [@test] crates/file-diffs/src/marks_test.rs (`the_marks_of_edited_files_are_proven_by_their_tokens`, `the_renames_of_files_with_a_name_renamed_are_proven_by_their_bindings`)

### Requirement: Renames: A run of a renamed name is quiet only for a value of its body alone

A run of the kind `renamed` MUST be proven, and so quiet once its kind is, only when every place in it is of a name
bound inside a body that is a value of that body alone on both sides, every name of which the scopes bind: a
variable, a parameter no call may name and no annotation or attribute macro may read, the variable of a loop, a
`catch` or a lambda, in a text the scopes read whole. Any other rename MUST have runs that are labels.

#### Scenario: A local other code may name
- **WHEN** a parameter of a named function of Kotlin, C# or Starlark, a Java parameter under an annotation a framework may read, a Rust parameter under an attribute macro, or a function declared in a body is renamed
- **THEN** its runs are labels and every block of them has its start
- **AND** the parameter of a lambda of Kotlin and a parameter of Java, TypeScript and Rust without such a reader are proven
- **AND** the name of a named argument is no name of the parameter: it is left to the other files
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_local_that_code_outside_its_body_may_name_is_a_label`)

#### Scenario: A text the scopes read in part
- **WHEN** a local is renamed in a C# file with an `#if`, where a branch the parser does not take reads the new name, or holds nothing of either name; in a Rust function whose two declarations of the name stand under two `#[cfg]`; in a script that calls `eval` by its name
- **THEN** its runs are labels and every block of them has its start: the bindings of such a text are those of one build of it, or of a part of what runs
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_local_that_code_outside_its_body_may_name_is_a_label`)

#### Scenario: Files with a name renamed
- **WHEN** a word of a file of each grammar is written otherwise wherever it stands, alone and beside random edits, sometimes but for one place, sometimes as a word the file has
- **THEN** each rename holds when the navigation is asked again of each side, each run of the kind `renamed` has the same tokens with its places read as one and holds no place that makes more of its name, and a proven one only places of values of their bodies alone
- **AND** the marks are the same with and without the question before the tokens
  [@test] crates/file-diffs/src/marks_test.rs (`the_renames_of_files_with_a_name_renamed_are_proven_by_their_bindings`)

### Requirement: Renames: The runs of a renamed name are labels until their kind is switched on

The kind `renamed` MUST NOT be quiet before a measurement of real ranges has read a sample of its proven runs and
found no change of behaviour among them: until `is_quiet` switches it on, every run of it MUST be a label and every
block of it a stop. The pair MUST name the proven runs that are labels so (`PairMarks::proven`), and the marks of
a view MUST be the marks as they are proven (`PairMarks::as_proven`) but for the quiet of those runs and the starts
that follow.

#### Scenario: A renamed local as a view gets it
- **WHEN** a local renamed at its five places is read as a view reads it, in each grammar
- **THEN** the diff has the rename and its runs, none of them quiet and each named as proven, and every block its start at its first line
- **AND** as proven the same runs are quiet, and no block has a start
  [@test] crates/file-diffs/src/marks_renames_test.rs (`the_runs_of_a_renamed_local_are_labels_until_their_kind_is_switched_on`)

#### Scenario: Every diff of the tests
- **WHEN** the files of the corpus are edited at random, or have a name renamed, and each diff is read as proven and as a view reads it
- **THEN** the marks of the view are the proven ones with the runs of a kind that is not switched on as labels, and the starts of those runs
  [@test] crates/file-diffs/src/marks_test.rs (`the_marks_of_edited_files_are_proven_by_their_tokens`, `the_renames_of_files_with_a_name_renamed_are_proven_by_their_bindings`)

### Requirement: Renames: A place that makes more of its name than its binding is read

A place where the name is more than the name of its binding MUST be a place of the rename and in no run: a shorthand
property, a pattern that takes a value apart, a name only the types or what runs tell apart, a name whose spelling is
a value where the code runs (`nameof`, a delegate, the name a function takes), a name among the tokens of a macro or
captured by a format string. The other places of the rename keep their runs.

#### Scenario: Such places
- **WHEN** a local declared by a destructuring, a record pattern, an array pattern or a tuple pattern, read by a shorthand property of TypeScript, Rust or C#, passed to a macro, captured by a format string, called as a value or spelled by `nameof` is renamed
- **THEN** the line of that place is in no run and is read, and the lines of its other places are proven runs
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_place_that_makes_more_of_its_name_than_its_binding_is_read`)

#### Scenario: A name that is a value where the code runs
- **WHEN** a Kotlin local declared with a delegate is renamed (`val total by map`, `val integrationTest by registering(…)` of a build script, `by lazy` in an `init`), or a local of TypeScript or JavaScript that is given a function or a class written without a name (`const total = () => 1`, `const Total = class {}`)
- **THEN** the line of its declaration is in no run and is read: the delegate reads another key, the task is registered under another name, the function has another `name`; the lines of its uses are proven runs
- **AND** a use of a JavaScript local inside the body of a `with` is in no run and is read: there the name may be a property of the object
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_place_that_makes_more_of_its_name_than_its_binding_is_read`)

### Requirement: Renames: A name that is not local is a label, and its declaration is read

A name no body binds, declared in the file, MUST be a rename that is not `local` when every name of its spelling
changed: its uses are runs that are labels, its declaration is in no run. A name that changed at every place of the
file and is declared in none of them MUST mark nothing, and the pair MUST hand it out for the pass over the files
of its subject, with the runs its blocks have were it a rename.

#### Scenario: A member, a function, a class
- **WHEN** a property, a field, a function or a class is renamed with its uses in the file, in Kotlin, Java, TypeScript and Rust
- **THEN** the rename is not `local`, `declared` names its declaration, each use is a run that is a label, and the block of the declaration has its start
- **AND** a parameter of a TypeScript constructor that declares a property (`public total`) is such a rename, with its uses in the constructor and through `this`
- **AND** a property and a local of its spelling renamed alike are two renames, the local one proven
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_name_that_is_not_local_is_a_label_and_its_declaration_is_read`)

#### Scenario: A name the file does not declare
- **WHEN** the two calls of a function declared in another file get another name
- **THEN** the diff has no rename and no run, and the pair names the two names with their places, without a declaration
- **AND** it keeps the run each call would be were the name a rename, which the pass over its subject takes once another file declares it
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_name_the_file_does_not_declare_is_left_to_the_other_files`)
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_pair_keeps_the_runs_a_name_it_declares_none_of_would_have`)

### Requirement: Moves: Lines deleted in one block and added in another are a move

Rows of lines that one block of a diff deletes and another block adds MUST be a move (`moves`), found from the lines
of the two texts alone: the same lines by their key, the text of a line without the spaces and tabs at its ends, the
whitespace inside it kept; a blank line pairs with any blank line. A move MUST count only when its exact lines hold
24 characters that are no whitespace, and its two ends MUST lie in two blocks. The empty last line of a text MUST be
no line of a move.

#### Scenario: A function at another place
- **WHEN** a function stands below another function now, with the blank line after it
- **THEN** its lines are one move, from its old lines to its new lines, the blank line with it, whatever ends the lines of the texts
- **AND** moved to the end of the file, where the comparison deletes the blank line after it and adds one before it, it is one move that counts that blank line beside its lines
- **AND** the same text, added lines alone and deleted lines alone have no move
  [@test] crates/text-diff/src/moves_test.rs (`lines_deleted_in_one_block_and_added_in_another_are_a_move`)

#### Scenario: The empty last line of a text
- **GIVEN** the empty last line of a text is the line after its last line end, or the one line of an empty text, which the comparison does not count
- **WHEN** a function leaves, with the blank line after it or before it, for the end of a text, and only the new text ends with a line end, so the block at the end of the texts holds its empty last line
- **THEN** the move is the function alone: the deleted blank line is a line to read, with its start, no pair of the empty last line and no blank line that left with the move, as where both texts end with a line end and where neither does
- **AND** the empty last line of the old text is no line that moved with the function before it
- **AND** a blank line that is a line of both texts moves with the function; a text of one line that left and an empty text have no move
  [@test] crates/text-diff/src/moves_test.rs (`the_empty_last_line_of_a_text_is_no_line_of_a_move`)

#### Scenario: Blank lines inside
- **WHEN** the lines that moved hold blank lines, with other whitespace on the two sides
- **THEN** they are lines of the one move, the whitespace of a blank line that changed a range of it where every character counts
- **AND** a blank line indented with the lines of its move is the same line
- **AND** a blank line that pairs with a line of text ends the move, and blank lines alone are no move
  [@test] crates/text-diff/src/moves_test.rs (`the_blank_lines_inside_a_move_are_lines_of_it`)

#### Scenario: A copy
- **WHEN** a function stays where it was and stands a second time, or one of its two copies is deleted
- **THEN** the diff has no move: no line was deleted and added
  [@test] crates/text-diff/src/moves_test.rs (`a_copy_is_no_move`)

#### Scenario: One long line, short lines
- **WHEN** one line of 24 characters that are no whitespace moves, and one of 23
- **THEN** the first is a move and the second none
- **AND** short lines count together, a blank line counts nothing, and the characters of an edited line do not count
  [@test] crates/text-diff/src/moves_test.rs (`one_long_line_is_a_move_and_short_lines_are_none`)

#### Scenario: Lines that cross nothing
- **WHEN** two functions change places and are indented, so one block holds both with no line between them that stays
- **THEN** the block has no move, also beside another block of the file
- **AND** the same lines in two blocks are a move
  [@test] crates/text-diff/src/moves_test.rs (`lines_that_cross_nothing_are_no_move`)

#### Scenario: Two blocks that change places
- **WHEN** two functions change places
- **THEN** the one the comparison deletes and adds is a move, with the blank line between them, the other stays
  [@test] crates/text-diff/src/moves_test.rs (`two_blocks_that_change_places_are_one_move`)

#### Scenario: The whitespace inside a line
- **WHEN** a line moves with other whitespace inside it
- **THEN** it is no exact line: alone no move, among exact lines an edited line whose ranges are its whitespace
  [@test] crates/text-diff/src/moves_test.rs (`the_whitespace_inside_a_line_is_part_of_its_key`)

#### Scenario: Any texts
- **WHEN** any two texts are compared under each policy, and lines move with a few changes
- **THEN** every move has its old lines in one block that is a change and its new lines in another, all of them lines their text counts, each line in one move, exact ends, and lines that are not edited the same on both sides but for the indent of the move
- **AND** each range of an edited line is a text that is another on the other side, of the kind its two sides say, with the same text between the ranges: none is empty on both sides
- **AND** where every character counts, no whitespace at an end of a line changed without a range, but by the indent of the move
  [@test] crates/text-diff/src/moves_test.rs (`the_moves_of_any_texts_are_moves`, `the_moves_of_moved_lines_are_moves`)

### Requirement: Moves: A move says the columns its lines were indented by

`indent` of a move MUST be the columns of its shift, negative for lost ones: the same whitespace added to the indent
of each of its lines, or taken from it, at one end of the indent. An indent is a text, not a number of columns.
Where the indentation of the file has a meaning and the policy counts the whitespace at the ends of a line, a line
whose indent did not move by the shift MUST be an edited line of the quarter of its move; elsewhere `indent` is the
shift most of its exact lines agree on.

#### Scenario: Lines indented into a block
- **WHEN** a function moves into a block and is indented by two spaces, by a tab, or out of it
- **THEN** the move says `2`, `1`, or the negative number
- **AND** of lines indented unevenly it says the columns of most of them, and a line that stayed at its indent is the same line
- **AND** tabs for the spaces of every line are no shift: the move says `0` and each indent is a range
- **AND** whitespace added before the text of each line, after its tab, is a shift as whitespace added before the tab is
  [@test] crates/text-diff/src/moves_test.rs (`a_move_says_the_columns_its_lines_were_indented_by`)

#### Scenario: A file whose indentation has a meaning
- **WHEN** lines move in such a file, all at the same indent or all shifted alike
- **THEN** the move is the same as in any file, with its shift
- **AND** a line at another indent than the others is an edited line whose first range is its indent, before the ranges of its text; more than a quarter of such lines, or one at an end, are no lines of the move
- **AND** a line that stayed at its indent while the others shifted, at the first column or at an indent of its own, is an edited line without a range, never one with a range that is empty or the same text on both sides; two such lines of six end the move
- **AND** a line of a recipe whose tab became a space is an edited line whose range is its indent; when every line of the recipe did, only its target moved
- **AND** two lines that gained two columns, one of spaces and one of tabs, did not move alike: the second is an edited line
- **AND** under a policy that leaves the whitespace at the ends of a line out, no indent is a change of a move
  [@test] crates/text-diff/src/moves_test.rs (`where_the_indentation_has_a_meaning_a_line_at_another_indent_is_edited`)

### Requirement: Moves: The whitespace at the ends of a line that changed otherwise than its move is a range

Under the policy that counts every character, a line of a move whose indent is another text and did not move by the
shift, or whose whitespace after its text is another, MUST be an edited line of the move with that whitespace as a
range, a blank line too: a view that shows the old lines of a move once hides no change. Where the indentation has
no meaning it MUST count in no quarter and end no move. Under a policy that leaves that whitespace out it MUST be no
change of a move.

#### Scenario: Under each policy
- **WHEN** a function moves and two of its five lines are indented by two more columns than the others, a line of a raw string by eight more, a line lost or got its indent, got tabs for its spaces, or the whitespace after its text was added, lost or changed
- **THEN** under the policy that counts every character it is one move, each such line an edited line whose range is its indent or the whitespace that ends it, beside the ranges of its text
- **AND** two such lines of five, and such a line at an end of the move, are lines of the move
- **AND** under `Trim`, `Ignore` and `IgnoreBlankLines` the move has no such range, where the indentation has a meaning too
  [@test] crates/text-diff/src/moves_test.rs (`the_whitespace_at_the_ends_of_a_line_that_changed_otherwise_than_its_move_is_a_range`)

### Requirement: Moves: A move holds at most a quarter of edited lines between exact ones

A move MUST hold edited lines only between exact ones, at most a quarter of its lines that are not blank: a pair of
lines that is similar by word, the rule of a row of the Inline view. `edited` MUST name the edited lines of a move by
their place in it; `oldRanges` and `newRanges` MUST give what changed in each as the ranges of a block, each a text
that is another on the other side. A move none of whose lines has a range has empty lists.

#### Scenario: An edited line
- **WHEN** a function moves with an argument added to one of its five lines, a word changed, or a word removed
- **THEN** it is one move, and the line has its ranges, typed as those of a block, in the offsets of its own line where the lines were indented too
- **AND** two edited lines of five, an edited line at an end, and a line that is another line are no lines of the move: the exact lines beside them are asked alone
  [@test] crates/text-diff/src/moves_test.rs (`a_move_holds_at_most_a_quarter_of_edited_lines_between_exact_ones`)

### Requirement: Moves: The longest move is taken first, and a line is in one move

A line of a side MUST be in one move at most. Of the moves the lines allow, the longest MUST be taken first, then the
one with fewer edited lines, then the first by its new and its old lines; what is left of a move that shares lines
with a taken one is asked again. The moves MUST be a function of the two texts, the blocks, the runs, the whitespace
policy of the diff and whether the indentation of the file has a meaning, in the order of their new lines.

#### Scenario: Two places
- **WHEN** deleted lines stand at two places of the new text, or lines of two old places at one
- **THEN** the first place takes them, and the other lines are added or deleted lines
- **AND** a place with all the lines is taken before one with a part of them, and one without an edited line before one with it
  [@test] crates/text-diff/src/moves_test.rs (`of_two_places_the_longest_move_is_taken_then_the_first`)

#### Scenario: What is left of a longer move
- **WHEN** three functions leave one block, the first two for one place and the last two for another
- **THEN** the first place takes its two functions, and the other place the one that is left
  [@test] crates/text-diff/src/moves_test.rs (`a_line_is_in_one_move_and_the_rest_of_a_longer_one_is_asked_again`)

### Requirement: Moves: A move counts the blank lines that moved with it

Blank lines beside the old lines of a move, in their block, and as many beside its new lines at its other end, in
their block, MUST be counted by the move (`blanksBefore`, `blanksAfter`) and be no entry of `moves`: the comparison
deletes a function with the blank line before it and adds it with the one after it. A blank line that none of the
other block pairs with, or one of another text where the policy counts its whitespace, MUST stay a deleted or an
added line.

#### Scenario: The blank line on the other side
- **WHEN** a function is deleted with the blank line before it and added with the blank line after it, or the other way round
- **THEN** the move of the function counts the blank line, before its old lines and after its new ones (`blanksBefore`) or the other way round (`blanksAfter`), and the block the lines left has no start
- **AND** on the same side of both it is a line of the move
- **AND** of two deleted blank lines and one added, one moved and the other is a deleted line with a start; a line between the blank line and the move keeps it where it is
- **AND** a blank line of spaces deleted where an empty one is added is a deleted line with a start where every character counts, and moved with the function under a policy that leaves its whitespace out
  [@test] crates/text-diff/src/moves_test.rs (`the_blank_lines_that_moved_with_a_move_are_counted_beside_it`)

### Requirement: Moves: A move is never quiet and needs no grammar

A move MUST never be quiet: its lines are in no run and in no block that is pale or cut off, so the block its new
lines stand in always has a start. Every pair of texts MUST have its moves, with its content, whatever its grammar;
whether the indentation of a path has a meaning is part of what the diff is kept by.

#### Scenario: The lines of a run
- **WHEN** a run holds lines of a block, quiet or not
- **THEN** a move takes only the lines beside it
- **AND** the lines of a block that is pale under its policy, or that the policy cut off, are in no move
  [@test] crates/text-diff/src/moves_test.rs (`the_lines_of_a_run_and_of_a_block_that_is_no_change_are_in_no_move`)

#### Scenario: With the content, whatever the grammar
- **WHEN** the diffs of a commit stream, a rule moved in a Python file, a text file and a Starlark file of the same two blobs
- **THEN** the content of each file has its move and its starts, from the comparison, from memory, from the store of another app and for a scan
- **AND** the line of the rule that stayed at its indent while the others shifted is an edited line in the Python file and the Starlark file alone
  [@test] crates/file-diffs/src/service_test.rs (`the_moves_of_a_text_come_with_its_content_and_are_those_of_its_indentation`)

#### Scenario: Files edited at random
- **WHEN** files of every grammar are edited at random and some of their lines stand at another place
- **THEN** no line of a move is in a run or in another move, and the block of its new lines has a start
  [@test] crates/file-diffs/src/marks_test.rs (`the_marks_of_edited_files_are_proven_by_their_tokens`)

### Requirement: Subject: A pair leaves the changed lines no mark took and the names it declares none of

Beside its diff a pair of blobs MUST keep what it leaves to the other files of a subject: the changed lines no mark
took, as hashed keys in rows; the names it declares none of, with the runs of the kind `renamed` their blocks have
were they renames, each with every name it leans on; the places of its declarations as the index takes them;
whether a side has a lone carriage return. The pass over a subject MUST read no blob.

#### Scenario: The lines no mark took
- **WHEN** a file loses a function, a file is added, and a line of a deleted function is in a run or the function moved within its file
- **THEN** the pair leaves the lines of the function as one row of the old side, with the key of each line (its text without the spaces and tabs at its ends, hashed; with its indent where the indentation of the file has a meaning) and its characters that are no whitespace, and an added file every line but the empty one after its last line end
- **AND** a line of a run and a line of a move within the file are left by no pair
  [@test] crates/text-diff/src/moves_across_test.rs (`a_pair_leaves_the_lines_of_its_changes_that_no_mark_took`)

#### Scenario: A row too small for a move between files
- **WHEN** a pair changes one long line, three closing braces, blank lines alone, or two short lines
- **THEN** it leaves no row: a row is kept from two lines that are not blank and 48 characters that are no whitespace
  [@test] crates/text-diff/src/moves_across_test.rs (`a_row_that_cannot_hold_a_move_between_files_is_not_kept`)

#### Scenario: The runs of a name the pair declares none of
- **WHEN** the calls of a function another file declares get another name, one of them beside a call that is wrapped
- **THEN** the pair keeps, of each such block read with the two names as one, the run of the kind `renamed` of the line of the name, with the name; the run of formatting of the wrapped call is the pair's own and is not kept
- **AND** the runs of the pair itself are as they were, and a kept run that shares a line with a move of the file is none

#### Scenario: A run that leans on the name after its block
- **WHEN** a line that changed in a name another file declares ends its block, and the first token of the code after the block is a place of another name the file declares none of
- **THEN** the kept run names both: it is a run only while the code after the block reads the same, which the second name read as one makes it
  [@test] crates/file-diffs/src/marks_subject_test.rs (`the_subject_adds_labels_alone_never_a_quiet_run_or_a_stop`)
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_pair_keeps_the_runs_a_name_it_declares_none_of_would_have`)

#### Scenario: The places of a declaration
- **WHEN** a file declares a renamed function after two characters outside the basic plane on its line, has a lone carriage return, or stands at a path with a tab
- **THEN** the declaration is asked by its line and its character from 1 in each text, and the text with the lone carriage return and the path with the tab ask nothing
  [@test] crates/file-diffs/src/marks_subject_test.rs (`the_declarations_are_asked_by_the_lines_and_the_characters_of_their_texts`)

#### Scenario: A store of its own
- **WHEN** a diff is stored, and a store holds a diff without what it left
- **THEN** what it left is an entry of its own store under the key and the version of its diff, which a read of the diff decodes nothing of
- **AND** a view compares the pair again and writes both, where a scan answers the stored diff
  [@test] crates/object-cache/src/cache_test.rs (`what_a_diff_left_is_read_by_the_key_of_its_diff_from_a_store_of_its_own`)
  [@test] crates/file-diffs/src/service_test.rs (`a_diff_stored_without_what_it_left_is_compared_again_for_a_view`)

### Requirement: Subject: Lines one file deleted and another added are a move between files

Rows of the same lines among what the pairs of a subject left, deleted in one file and added in another, MUST be a
move between the two files: exact lines only, by their text without the spaces and tabs at their ends, from two
lines that are not blank and 48 characters that are no whitespace. Where the indentation has a meaning, a line MUST
pair only with a line at its depth. Lines that left a file and stand again in it are the moves of its pair.

#### Scenario: A function moved to another file
- **WHEN** a file loses a function that another file of the subject gets, changed or added, whichever comes first
- **THEN** the lines of the function are a move from the first file to the second
  [@test] crates/text-diff/src/moves_across_test.rs (`lines_that_left_one_file_and_stand_in_another_are_a_move`)
  [@test] crates/file-diffs/src/marks_subject_test.rs (`lines_that_moved_between_two_files_are_a_label_at_both_ends`)

#### Scenario: Blank lines inside it, another indent
- **WHEN** two functions with a blank line between them move into an object of another file, indented by two
- **THEN** the nine lines are one move
  [@test] crates/text-diff/src/moves_across_test.rs (`a_move_between_files_keeps_its_blank_lines_inside_and_is_at_another_indent`)

#### Scenario: An edited line
- **WHEN** the first line of a moved function changed on the way, or its second
- **THEN** the three lines after the first are the move, and with the second line changed nothing is: the line before it and the two after it are each too little
  [@test] crates/text-diff/src/moves_across_test.rs (`an_edited_line_ends_a_move_between_files`)

#### Scenario: Boilerplate and the lines of one file
- **WHEN** two files have the same short end of a class, a function moves within its file beside another file, or a line stands at seventeen places of two files
- **THEN** nothing is a move between files
  [@test] crates/text-diff/src/moves_across_test.rs (`boilerplate_and_lines_that_moved_within_a_file_are_no_move_between_files`)
  [@test] crates/file-diffs/src/marks_subject_test.rs (`lines_that_match_two_places_move_to_the_first_and_boilerplate_moves_nowhere`)

#### Scenario: A line at another depth
- **WHEN** a Python function leaves a file for another, where its last line stands inside the `if` before it; where it stands whole inside a class; where one of its lines is indented by a tab
- **THEN** the three lines at their depth are the move and the fourth is a deleted and an added line; the function at another depth is no move; the line with the tab is no line of the move, and a blank line is one at any indent
- **AND** the same texts in files whose indentation has no meaning are one move of four lines
  [@test] crates/text-diff/src/moves_across_test.rs (`a_line_at_another_depth_is_another_line_where_the_indentation_has_a_meaning`)

### Requirement: Subject: The longest move between files is taken first, and a line is in one

A line of a side MUST be in one move between files at most: the longest row is taken first, then the one with more
characters, then the first by where its lines stand now and by where they left, the files in their order, and what is
left of a row that shares a line with a taken one is asked again. So lines that match two places are a move to the
first of them, and the other place is plain added lines.

#### Scenario: Two places
- **WHEN** two files get the lines one file lost, or two files lose the lines one file got
- **THEN** the lines are one move, of the first of the two files, and the other file has the marks of its pair
  [@test] crates/text-diff/src/moves_across_test.rs (`lines_that_match_two_places_are_a_move_to_the_first`)
  [@test] crates/file-diffs/src/marks_subject_test.rs (`lines_that_match_two_places_move_to_the_first_and_boilerplate_moves_nowhere`)

#### Scenario: The longest first
- **WHEN** a file loses two functions, one file gets the first and another both; or two files lose a function each and one file gets both in a row
- **THEN** the row of both is the move and the file with the one function keeps its added lines; the two functions of two files are two moves, each line once
  [@test] crates/text-diff/src/moves_across_test.rs (`the_longest_move_is_taken_first_and_the_rest_of_a_row_is_asked_again`)

### Requirement: Subject: A move between files is a label at both its ends

The marks of each of the two files MUST name the move (`subject.movedOut` where the lines left, `subject.movedIn`
where they stand now): its lines, the other file by its index among the files of the subject, and its first line
there. It MUST be a label: no run, no start taken away, no line for a view to hide.

#### Scenario: The two ends
- **WHEN** a function moves from a ledger to a report
- **THEN** the marks of the ledger name its four old lines with the index of the report and the line they stand at, those of the report its four new lines with the index of the ledger and the line they left
- **AND** the runs, the moves, the renames and the starts of both files are those of their pairs: every block is read where it was
  [@test] crates/file-diffs/src/marks_subject_test.rs (`lines_that_moved_between_two_files_are_a_label_at_both_ends`)

#### Scenario: The template of the marks
- **WHEN** the commit of the `commit_marks` template is read
- **THEN** `reportTitle` is a move from `Ledger.kt` to `Report.kt` at both, the calls of `openStore` in `Report.kt` are runs of the name `Store.kt` declares, and every block of the three files has its start
  [@test] crates/file-diffs/src/marks_test.rs (`the_marks_template_has_each_case_in_a_block_of_its_own`)

#### Scenario: The shape of the marks of a subject
- **WHEN** a diff with the marks of its subject is written as JSON, and one without
- **THEN** `marks.subject` has `movedOut`, `movedIn`, `declared`, `used` and `confirmed`, every list written, and is `null` in the diff of a pair alone; a diff written without the field reads as one without the marks
  [@test] crates/text-diff/src/model_test.rs (`the_marks_are_camel_case_json_with_every_field`)

### Requirement: Subject: The moves between files are bound by work

The search MUST be bound by work, never by time: a file whose rows do not fit into 262,144 lines beside those of the
files before it takes no part, a line that stands at more than 16 places starts no row, and the search reads
4,194,304 pairs of lines at most, with the moves found until then. What was left out MUST be logged.

#### Scenario: The lines and the pairs
- **WHEN** the rows of a second large file do not fit beside those of the first, and when the search reaches its work
- **THEN** the large file takes no part and the small files after it do, and the search that reached its work answers the moves it found
  [@test] crates/text-diff/src/moves_across_test.rs (`the_search_is_bound_by_the_lines_of_its_files_and_by_the_pairs_it_reads`)

### Requirement: Subject: A name another file of the subject declares is a rename where it is used

A name that changed at every place of a file that declares none of them MUST be a rename of that file once another
file of the subject declares a name that is not local and renamed it so (`subject.used`). The lines that changed in
such a name alone MUST be runs of the kind `renamed`, labels, beside the runs of the pair; a line that leans on a
name no file declares is in none. The pass MUST add labels alone: the quiet runs and the starts stay the pair's.

#### Scenario: The calls of a renamed function
- **WHEN** a store renames its function and a report calls it at two places
- **THEN** the report names the rename with its two places, the store as the file that declares it and the line of the declaration, each call is a run of the kind `renamed` that is a label, and the starts of the report are those of its pair
- **AND** the renames of the marks of the report are still those it declares: none
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_name_another_file_declares_is_a_rename_of_the_file_that_uses_it`)

#### Scenario: A name no file declares
- **WHEN** the report is the one file of its subject, or one of its lines changed in two names of which no file declares the second
- **THEN** the report alone has the marks of its pair, and the line of the two names is in no run and is read, beside the run of the line of the declared name alone
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_name_no_file_of_the_subject_declares_and_a_line_that_needs_one_have_no_run`)

#### Scenario: A line of such a run is in no move
- **WHEN** two lines of a report changed in the renamed name alone and another file got lines that read as they did
- **THEN** without the file that declares the name the two lines moved to the other file, and with it they are a run of the rename and no move
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_run_the_subject_adds_takes_its_lines_from_the_moves_between_files`)

#### Scenario: Labels alone
- **WHEN** a call is wrapped otherwise at the end of its block, before a line that changed in a name alone, whether the store renamed that name or no file declares it; and in every report made of such statements at random
- **THEN** the wrapped lines are no run, as in the pair: with the name read as one they are a run of formatting, which is quiet and leans on a name it does not hold
- **AND** the quiet runs, the starts, the moves and the renames of the file are those of its pair, every run of the pair lies in a run of its kind, and every run the subject added is of the kind `renamed` and no quiet one
- **AND** a line of the renamed name at the end of its block is a run only when the name after the block is one a file of the subject renamed too
  [@test] crates/file-diffs/src/marks_subject_test.rs (`the_subject_adds_labels_alone_never_a_quiet_run_or_a_stop`)

### Requirement: Subject: The index is asked once of every declaration a subject renamed

The declarations of the renamed names of a subject that are not local, in files of Kotlin or Java, MUST be asked of
the index in one process (`accio renames --no-build`), after its diffs and never by their read, by the window that
shows it. Only an accio that answers them as they are read is started; the working tree MUST NOT be asked. An
answer MUST be kept by its trees, and the query killed once the window asks another, closes or loads again.

#### Scenario: One process for a subject
- **WHEN** the diffs of a commit are read where an index is, then the index is asked of the commit, of it again, of a range and of the working tree
- **THEN** the read of the diffs asks nothing and its names say `asked`; the question asks every declaration in one call, between the parent and the commit or the two commits of the range, and reads no blob
- **AND** asked again of the same trees it starts no process, and the working tree is never asked
  [@test] crates/file-diffs/src/service_test.rs (`the_index_is_asked_after_the_diffs_of_a_subject_in_one_call_and_its_answer_is_kept`)
  [@test] crates/file-diffs/src/accio_test.rs (`the_renames_of_a_subject_are_asked_in_one_process`)
  [@test] crates/test-server/src/commit_files_test.rs (`the_diffs_of_a_subject_say_what_its_files_say_of_each_other_and_the_index_is_asked_after_them`)

#### Scenario: No declaration, no question
- **WHEN** a subject renames no name that is not local
- **THEN** the index is asked nothing
  [@test] crates/file-diffs/src/service_test.rs (`an_index_is_asked_only_of_a_declaration_and_one_that_does_not_answer_fails_no_diff`)

#### Scenario: A declaration of another language
- **WHEN** a TypeScript file declares a renamed function another file uses, alone or beside a Kotlin file that renamed another
- **THEN** the index is asked nothing of it: its name is `incomplete` at once, in the file that declares it and in the one that uses it, and the declaration of the Kotlin file is asked
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_name_declared_in_a_language_the_index_proves_nothing_of_is_not_asked`)

#### Scenario: An accio without the answer
- **WHEN** the `versions` of accio names no answer of `renames`, another revision of it, or fails
- **THEN** its `renames` is never started, and the `versions` of one build of accio is asked once
  [@test] crates/file-diffs/src/accio_test.rs (`an_accio_that_answers_no_renames_as_they_are_read_is_not_asked`)

#### Scenario: An accio updated while the app runs
- **WHEN** the executable of accio is replaced by one that answers `renames`, then by one whose answer is of another revision
- **THEN** its `versions` is asked again each time: the first is asked the renames from then on, the second no more
  [@test] crates/file-diffs/src/accio_test.rs (`an_accio_updated_while_the_app_runs_is_asked_again_what_it_answers`)
  [@test] crates/file-diffs/src/accio_test.rs (`what_an_executable_says_of_itself_is_asked_once_of_each_build`)

#### Scenario: The window asks of another subject, closes or loads again
- **WHEN** the window that asked asks of another subject, loads again or closes while the query runs; and when another window does any of it
- **THEN** the query is killed and the question answers nothing, the names still `asked`; a page that is gone asks nothing; what another window does ends nothing
  [@test] crates/app-api/src/reads/diffs_test.rs (`the_question_of_a_window_to_the_index_ends_once_the_window_asks_another_closes_or_loads_again`)
  [@test] crates/app-api/src/session/windows_test.rs (`a_window_shows_the_page_of_a_call_until_it_closes_or_loads_again`)
  [@test] crates/file-diffs/src/accio_test.rs (`renames_without_an_index_answer_no_index_and_a_canceled_query_is_killed`)
  [@test] crates/file-diffs/src/service_test.rs (`an_index_is_asked_only_of_a_declaration_and_one_that_does_not_answer_fails_no_diff`)

### Requirement: Subject: A rename is proven by a complete answer whose every use the subject explains

A declaration MUST be proven renamed (`confirmed`) only when the index answers one declaration of Kotlin or Java,
complete, with no name that means something else, and every use it lists is the declaration, a place of the rename
with its twin listed too, a link of a doc comment, a use without a word or by another word both trees hold alike,
or on a changed line in no run. The runs whose every place is proven MUST be named (`subject.confirmed`).

#### Scenario: Every use is a place of the rename
- **WHEN** the index lists the declaration and the two calls of the report as the uses of the function in both trees, complete and without a conflict
- **THEN** the name of the report and the declaration of the store are `confirmed`, the two runs of the report are named as confirmed and stay labels while the kind `renamed` is off, and every block is a stop
- **AND** with the kind switched on the runs are quiet and the report has no start left, and the block of the declaration keeps its start
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_rename_every_use_of_which_the_index_lists_is_confirmed_and_its_declaration_stays_a_stop`)

#### Scenario: A use on a line that is read anyway
- **WHEN** the change also deletes a call of the function in a third file and adds one there, each on lines of its own, and a doc comment outside the subject names the function
- **THEN** the rename is confirmed all the same; the same use on a line the change left alone is a use outside it
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_use_on_a_line_that_is_read_anyway_and_a_link_of_a_doc_comment_do_not_stand_in_the_way`)

#### Scenario: A use without a word, a use by another word
- **WHEN** the index lists a positional argument that sets a renamed property, on a line the change left alone, as it answers of such a change; and a call by the alias of an import in both trees, on a line the change left alone, in a file of the subject one line further down or in a file outside it
- **THEN** the rename is confirmed all the same: neither writes the name
- **AND** a use by another word that only one tree lists, or that the other tree lists on another line or by another word, leaves the rename `incomplete`, never a verdict of what it changed
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_use_that_writes_no_word_or_the_same_other_word_in_both_trees_does_not_stand_in_the_way`)

#### Scenario: The accio of this machine
- **WHEN** this machine has an accio that answers `renames`, and a repository renames a function with its two calls in another file
- **THEN** the diffs of the commit, with an index of the repository built for the test, confirm the two calls and the declaration
  [@test] crates/file-diffs/src/accio_test.rs (`the_accio_of_this_machine_proves_the_rename_of_a_real_repository`)

### Requirement: Subject: An answer that is no proof leaves the rename a label and names why

Every other answer MUST leave the rename a label and name why: a use outside the change (`usesOutside`), a name
that means something else (`otherMeaning`), two places that are not one declaration (`otherDeclaration`), an answer
that does not know every use or names a line the diff has not (`incomplete`), no answer (`unavailable`), `asked`
until the index answered. No answer MUST fail the diffs or read as "no use"; only a whole one is kept.

#### Scenario: Each reason
- **WHEN** the answer has a use the change left behind, a use in a file outside the subject or on a line it left alone; a conflict of a capture, an overload, an override, a hidden or a namesake declaration, a use without its twin, a use of the new name on a line left alone; another signature; an answer that is not complete, a use tied by its name alone, more uses than listed, no uses of a tree, another language; a pair that could not be asked
- **THEN** the proof is `usesOutside`, `otherMeaning`, `otherDeclaration`, `incomplete` and `unavailable` in that order of the cases, and no run is confirmed
- **AND** a place of the name the index lists for no declaration leaves the name of its file `incomplete`, while the declaration and the use it does list are proven
- **AND** what the index found wrong comes before what it could not say
  [@test] crates/file-diffs/src/marks_subject_test.rs (`every_other_answer_leaves_the_rename_a_label_and_says_why`)

#### Scenario: An index that does not answer
- **WHEN** the index answers no renames, has no index of the repository, fails, or answers another number of pairs than asked
- **THEN** the diffs are read whole and the names are `unavailable`; nothing of it is kept, and the next question asks the index again
  [@test] crates/file-diffs/src/service_test.rs (`an_index_is_asked_only_of_a_declaration_and_one_that_does_not_answer_fails_no_diff`)
  [@test] crates/file-diffs/src/accio_test.rs (`renames_without_an_index_answer_no_index_and_a_canceled_query_is_killed`)

#### Scenario: A text with a lone carriage return
- **WHEN** the index lists a use in a file of the subject whose text ends a line with a carriage return that no line feed follows
- **THEN** the rename is `incomplete`: the comparison counts such a line end and the index does not, so the line the index names is no line of the diff; the same answer of the same file without it confirms the rename
- **AND** a use the index itself found left behind is still `usesOutside`
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_use_in_a_text_with_a_lone_carriage_return_proves_nothing`)

### Requirement: Subject: The marks of a subject enter no cache and are the same whenever they are read

What the files of a subject say of each other MUST be a function of what its pairs are and left, in the order of
git: the same from the comparison, from memory and from the store of another app. It MUST NOT be kept with a pair:
the same pair in another subject has the marks of its pair alone.

#### Scenario: From the comparison, from memory and from the store
- **WHEN** the diffs of a commit with a function that moved between two files and a name another file declares are read, read again, and read by another app with the same store
- **THEN** the three answers are the same, the marks of the subject among them, and the second read costs no process
  [@test] crates/file-diffs/src/service_test.rs (`a_diff_costs_one_list_and_one_cat_file_then_nothing`)
  [@test] crates/file-diffs/src/marks_subject_test.rs (`the_marks_of_a_subject_are_the_same_whenever_they_are_read`)

#### Scenario: The same pair in another subject
- **WHEN** the pair of a report that a subject marked with the name its store declares is the one file of another commit
- **THEN** it has the marks of its pair there, from memory, without a blob read
  [@test] crates/file-diffs/src/service_test.rs (`the_marks_of_the_subject_follow_the_contents_or_come_with_them_from_the_caches`)

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

### Requirement: Never marked: A file of no grammar, a pair of two grammars and lines that moved have no rename

A rename MUST be read from the tokens and the bindings of one grammar: a file of no grammar and a pair of two grammars
have none, by no rule of characters. A name renamed in lines that moved stands in another block than it did and MUST
be none. A text with a syntax error has the renames its parser reads, none of them proven; a text its parser reads
no program of has none.

#### Scenario: No grammar, moved lines
- **WHEN** a name changes at every place of a text file and of a Python file, or in a function that moved
- **THEN** the diff has no rename
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_file_of_no_grammar_and_lines_that_moved_have_no_rename`, `a_pair_of_two_grammars_has_no_rename`)

#### Scenario: A syntax error
- **WHEN** a local is renamed in a Java file where the old name still stands in what the parser gave up on, in a Kotlin, a Java and a Rust file with a broken function after it, in a Kotlin file on a line that is broken on both sides, and in a TypeScript file oxc reads no program of
- **THEN** the first five have the rename with runs that are labels, every block of them with its start: what stands where a parser gave up is no name to the scopes
- **AND** the TypeScript file has none
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_file_with_a_syntax_error_is_read_as_its_parser_reads_it`)

### Requirement: Limits: The time of a comparison changes no mark

The marks of a diff MUST be a function of its blocks, its two texts, its whitespace policy and how its paths are read
(their grammar, the meaning of their indentation), the same on every machine and under every load. A diff the time cut short
(`lineLevelOnly` within the size of the inner ranges) MUST have the marks of its blocks: the runs read the blocks and
the tokens, the moves the blocks and the lines, never the inner ranges.

#### Scenario: A diff the time cut short
- **WHEN** the inner ranges of a diff run out of their time, or are not asked for
- **THEN** the diff is by lines alone and has the runs, the moves and the starts of the diff in time, from the comparison and from memory
- **AND** a diff whose lines ran out of time too is one block of both texts, with the marks of that block
  [@test] crates/file-diffs/src/marks_test.rs (`a_diff_the_time_cut_short_has_the_marks_of_its_blocks`)
  [@test] crates/file-diffs/src/service_test.rs (`the_marks_of_a_diff_out_of_time_are_those_of_the_diff_in_time`, `the_moves_of_a_text_come_with_its_content_and_are_those_of_its_indentation`)

### Requirement: Limits: The marks are bound by work, never by time

The limits of the marks MUST be of work, never of time. A block whose sides are the same code needs no comparison.
Else the tokens of its code are compared only within `MAX_BLOCK_TOKENS` a side, `MAX_TOKEN_EDITS` tokens that differ
and the work left of its file (`MAX_FILE_TOKEN_WORK`); a block over a limit MUST have no run cut out of it. The
blocks of a rename are compared again within what the first reading left of that work.

#### Scenario: A block over a limit
- **WHEN** the sides of a block differ in more tokens than the limit, a side has more tokens than the limit, or the work of the file is spent
- **THEN** no run is cut out of the block, and its reading starts at its first line
- **AND** the same block within the limits has its run, and lines of the same code are a run without a comparison
  [@test] crates/file-diffs/src/marks_test.rs (`a_block_over_a_limit_of_work_has_no_run_cut_out_of_it`)

#### Scenario: The blocks of a rename
- **WHEN** a block holds a renamed name, a wrapped call and a line with another change, and is read again with the names read as one
- **THEN** with the work of the file it has its run of the rename beside the run of the wrapped call; with one step less than the comparison costs it keeps the run of the wrapped call the first reading found, and no run of the rename
- **AND** a block that is the same code once the names are read as one has its runs without a comparison, with no work left too
- **AND** a block the first reading cut nothing out of has the same runs: the wrapped call is a run of formatting there too
  [@test] crates/file-diffs/src/marks_renames_test.rs (`the_blocks_of_a_rename_are_cut_again_within_the_work_the_file_has_left`)

#### Scenario: The comparison gives up by its edits
- **WHEN** two sequences differ in more elements than the edits allowed
- **THEN** the comparison answers "too big" after work bound by the edits, and within them the changes of the unbound comparison
  [@test] crates/text-diff/src/lcs/diff_test.rs (`the_changes_within_a_number_of_edits_are_the_changes_or_too_big`)

### Requirement: Limits: The moves are bound by work

The search for moves MUST be bound by work, never by time. A side with more than 65,536 lines a move may take has no
moves; a key that more than 16 such new lines have starts no move; the search and the choice each end at 262,144
pairs of lines read, each read counted, with the moves found by then. At most 2,048 pairs of lines of at most 512
UTF-16 code units are compared by word, and the chains grow through 16,384 pairs left unread: past either number a
pair of other keys is apart.

#### Scenario: A key that many lines have
- **WHEN** 10,000 lines of eight keys move
- **THEN** no line starts a move and the search reads no pair of lines
- **AND** with a line of its own among every hundred, the lines are one move, each pair read once
- **AND** 16 lines of one key are a move, 17 are none
  [@test] crates/text-diff/src/moves_test.rs (`a_key_that_many_lines_have_starts_no_chain_and_is_a_line_of_one`)

#### Scenario: The work of a search
- **WHEN** 10,000 lines of five keys move, among them fifty lines that each stand sixteen times
- **THEN** the lines are one move, found with fewer than twenty pairs of lines read a line, within the limit, the same when asked again
- **AND** a search that reaches its limit ends with the moves found until then, and so the choice among them
  [@test] crates/text-diff/src/moves_test.rs (`the_work_of_a_search_grows_with_its_lines_and_ends_at_its_limit`)

#### Scenario: The file a search costs most in
- **WHEN** every line of a file stands sixteen times on each side and no two lines follow each other on both, the lines nothing alike or alike but for a number
- **THEN** the search ends at its limit of pairs read (a pair of two lines read for their likeness counts by their length too), with moves that are moves, the same when asked again
  [@test] crates/text-diff/src/moves_test.rs (`the_search_of_a_file_whose_every_line_stands_sixteen_times_ends_at_its_work`)

#### Scenario: A chain of many short moves
- **WHEN** a list of YAML of 2,000 items of five lines moves, indented again by two columns a level in place of four; every second of 6,000 lines of such a file moves under another parent; or 2,500 lines of their own move, each before a line with another number and two blank lines
- **THEN** the chain is read once for all its moves, and the work grows with the lines, not with their square: every move is found, each of a line or two (of the list the name of each item, and its image with its port), with at most six pairs of lines read a line
- **AND** read again for each of its moves, where each pair of what is left of a row counts again, the first chain uses up the work of the search, and less than a tenth of the moves is found
- **AND** where the indentation has no meaning the list is one move
- **AND** at the limit of its work the search ends with the moves found until then
  [@test] crates/text-diff/src/moves_test.rs (`a_chain_of_many_short_moves_is_read_once_for_all_of_them`)

#### Scenario: Read once, or again for each move
- **WHEN** a chain of up to 90 pairs (the same line, a similar one, one alike and not similar, a blank one, one at another indent, another line) is read once for all its moves, and again for each of them
- **THEN** the moves are the same, and so are the pairs compared by word and left unread
  [@test] crates/text-diff/src/moves_test.rs (`the_moves_of_a_chain_are_the_same_read_once_or_again_for_each`)

#### Scenario: A sorted list of lines that are alike
- **WHEN** 2,000 lines that are alike but for a number stand in another order, no two in a row on both sides
- **THEN** every line is found as the move of one line it is, the chains having grown through 16,384 pairs they left unread and no pair compared by word, the search within a quarter of its limit
- **AND** without that limit the chains use up the work of the search and lines are not found
  [@test] crates/text-diff/src/moves_test.rs (`a_sorted_list_of_lines_that_are_alike_is_searched_within_the_pairs_left_unread`)

#### Scenario: Over a limit
- **WHEN** a side has more lines a move may take than the limit, more pairs are similar than are compared, or a line is longer than the comparison reads
- **THEN** the side has no moves, the pairs past the number are no edited lines, and the long line ends its move
- **AND** a pair of lines in a row that is no move whatever the pair is, an edited line at the end of its row, is not compared at all
  [@test] crates/text-diff/src/moves_test.rs (`a_side_over_its_lines_and_pairs_over_their_number_are_not_read`)

### Requirement: Limits: The names of a file are asked within a limit of work

The names asked of a file for its renames MUST be bound by work, never by time: a resolution of a name, one per
spelling and one more per binding, costs a part of the tokens of its text by its grammar, within `MAX_RENAME_WORK`.
The first place of each pair of names MUST be asked before the second of any; a name without an answer is no
rename, and its blocks stay stops. The names MUST be read on the stack of a worker of the diffs.

#### Scenario: Two hundred locals
- **WHEN** 200 locals of a Kotlin file are renamed, 400 places, each local a spelling and a binding of each text
- **THEN** the 200 are renames, by 400 resolutions of each text; with work for 150 resolutions the first 75 locals of the text are, and with work for less than one none is asked
- **AND** in a Java file whose work pays fewer resolutions than its 100 locals need, the first locals of the text are renames and, of their proven runs, the blocks of the others alone have their starts
- **AND** a local of one name in each of 60 functions, renamed alike, is 60 renames by 61 resolutions of each text
  [@test] crates/file-diffs/src/marks_renames_test.rs (`the_names_asked_are_bound_by_work`)

#### Scenario: A deeply nested file
- **WHEN** a local is renamed in a block nested 60 to 3,600 times, in each grammar, on a stack of 2 MiB
- **THEN** a text with tokens has its rename and a text nested deeper than its parser reads has none
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_deeply_nested_file_is_read_on_the_stack_of_a_worker`)

### Requirement: Before the tokens: A file whose blocks cannot hold a run is not parsed

A block MUST be asked a question of its characters before the tokens of its file are read: whether a line of one side
starts and a line ends as a line of the other, their blanks aside, a comment may be in its lines, or one side holds
a word the other has not. The question MUST change no mark: every block with a run answers yes, and a block of the
same words gives no name of a rename. A file whose blocks all answer no is not parsed, and the tokens of a blob MUST
be read once per grammar.

#### Scenario: Blocks that cannot hold a run
- **WHEN** the blocks of a file change values and an operator, or add and delete lines
- **THEN** the tokens of neither side are read
- **AND** a wrapped call, a comment worded again and a changed name read each side once, whatever the number of blocks
  [@test] crates/file-diffs/src/marks_test.rs (`a_file_whose_blocks_cannot_hold_a_run_is_not_parsed`)

#### Scenario: The same words
- **WHEN** the two sides of a block hold the same words: in another order, with other numbers or other marks between them
- **THEN** the block gives no name of a rename, and two locals that took the name of each other in one block parse no side
- **AND** a word one side has more often, or with another letter, `_` or `$`, is a word the other has not
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_block_whose_sides_hold_the_same_words_gives_no_candidate`)

#### Scenario: The question changes no answer
- **WHEN** the runs and the renames of a diff are read with the question and without it, for every case of the tests and for files of every grammar edited at random, with their own line ends, with `\r\n`, with lone `\r` among them and without the last one
- **THEN** the runs and the renames are the same
- **AND** every line a comment of a grammar is in answers that it may hold a comment: a line of a block comment, of a nested one, after a `/*/`, a `#!` line, an HTML-like comment of a script, a line of a comment of Razor in a C# file, the line after a lone `\r` that a line comment runs over
  [@test] crates/file-diffs/src/marks_test.rs (`the_marks_of_edited_files_are_proven_by_their_tokens`, `the_renames_of_files_with_a_name_renamed_are_proven_by_their_bindings`, `every_line_of_a_comment_may_hold_a_comment_before_the_tokens_are_read`)

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

In a section of a review, a run or an end of a move that starts a block with a heading MUST say its words in the row of that heading,
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

#### Scenario: A heading and an end of a move
- **WHEN** a block with a heading is the lines a move took, in Inline and in Unified
- **THEN** one row of words stands before it, the heading then the words of the move, which opens the old lines in Inline; no second row of words comes
  [@test] src/lib/diff/rows.test.ts (`says the words of an end of a move that starts its block in the heading of the change, and leaves a move with an end in no block out`)

### Requirement: Views: An old line shown once is found at the note that stands for it

An old line of a quiet run that Inline shows once, and one a move took, MUST resolve to the note that stands for it: the scroll position keeps a
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

#### Scenario: A line a move took
- **WHEN** the row of an old line of a move is asked, the note where the lines left closed and opened, and of the old line of an edited line of the move
- **THEN** it is the note while it stands for the line, and the row of the line where it left once it has one, never the row of its pair where the lines stand now
- **AND** a note at the top of the view stays there by that line across a new layout, the note opened or in Unified; the note where the lines stand now by the first of them
  [@test] src/lib/diff/rows.test.ts (`finds an old line of a move at the note that stands for it, and lands a jump to an end on its note`)
  [@test] src/lib/diff/layout.test.ts (`keeps the note of the lines a move took at the top by the old line it stands for, and the note of where they stand by the line after it; each a row`)

### Requirement: Views: The find searches the old lines a note stands for

The find MUST search the old lines a note of Inline stands for (of a quiet run shown once, before the hits of its new
lines; of a move, where its lines left), as hits of the old side. A step to such a hit lands on the note, which names the hits it holds ("1 match in the old line") and
marks the current one; once the note is opened the same hits are in the old lines. The row of an edited line of a
move MUST be searched by its new line alone: a hit of its old line is one hit, where the line left.

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

#### Scenario: The lines a move took
- **WHEN** the query is in a line of a move, in Inline with the note where the lines left closed and opened, and in Unified and Split
- **THEN** the line is found where it stands now and, a hit of the old side, at the note that stands for it where it left, or in its own row once the note is opened and where every line stays
  [@test] src/lib/diff/find.test.ts (`searches the lines that left at the note that stands for them in Inline, after the lines where they stand now`, `finds the lines that left by their own rows once the note is opened, and where every line stays`)

#### Scenario: The word an edited line of a move lost
- **WHEN** the query is the word an edited line of a move lost, and text both its lines have, the note where the lines left closed and opened
- **THEN** the word is one hit, of the old line, at the row of the note, which counts one match, or at the row of the old line once the note is opened; never a second hit in the row of the two lines
- **AND** the text both lines have is found in the new line of the row and in the old line where it left, no hit twice
- **AND** the row of the two lines marks of its old line the struck-through parts alone, the current hit among them
  [@test] src/lib/diff/find.test.ts (`searches the lines that left at the note that stands for them in Inline, after the lines where they stand now`, `finds the lines that left by their own rows once the note is opened, and where every line stays`)
  [@test] src/lib/diff/line-parts.test.ts (`keeps of the hits of the old line of a pair its own alone, the current one among them, for the row of an edited line of a move`)

#### Scenario: A step to the word an edited line lost
- **GIVEN** a commit that moves a function from the end of its class to its start, a word of one line another, every function between the ends changed
- **WHEN** the user finds the old word in the review, presses Enter twice, then opens the note
- **THEN** the find counts one result, marked in the row of the two lines where the line stands now, struck through; the step scrolls to the note where the lines left, which names "1 match in the old lines", the current hit, and a second step stays there
- **AND** opened, the hit is marked in the old line, still the one result
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`the find counts the word an edited line of a move lost once, marks it in the row of its two lines and lands on the note where the line left`)

#### Scenario: A step lands on the note of a move
- **WHEN** the user finds "fun accounts" in the review of the `commit_marks` template, presses Enter twice, then opens the note
- **THEN** the first step marks the line where it stands now, the second goes to the note where the lines left, which names "1 match in the old lines", the current hit
- **AND** opened, the hit is marked in the old line, still the current one, and the note names none
- **AND** "val text", which both lines of the edited line of `describe` have, is two results: the first step marks it in the row of the two lines, the second at the note where the lines left, the row marking it as a hit and no more as the current one
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`the find searches the lines a move took at the note that stands for them and lands on it; opened, the hit is in the line`)

### Requirement: Views: A copy takes the lines the view shows

A copy MUST give the lines the rows show: of a quiet run of Inline its new lines, its old lines too once its note is
opened and where every line stays; of a move the lines once, where they stand now, and where they left too once
their note is opened and where every line stays. A note gives no text.

#### Scenario: A selection over a quiet run
- **WHEN** the user selects from the note of the wrapped call to the end of its last line and copies, the note closed, then opened
- **THEN** the copy is the three new lines, then the old line and the three new ones
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`a copy takes what a quiet run shows: its new lines, and its old line once the note is opened`)
  [@test] src/lib/diff/copy-text.test.ts (`copies what a quiet run shows: its new lines alone in Inline, its old lines too once its note is opened or where every line stays`)

#### Scenario: The lines of a move
- **WHEN** the user copies the side of Ledger.kt of the `commit_marks` template in a review, the notes closed, then with the note where `accounts()` left opened
- **THEN** the first copy holds the first line of `accounts()` once and no old line of `describe`, and no words of a note
- **AND** the second holds the line twice, where it stands now and where it left
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`a copy takes the lines of a move once, where they stand now, and the lines that left too once their note is opened`)
  [@test] src/lib/diff/copy-text.test.ts (`copies what a move shows: its lines once where they stand now in Inline, the lines that left too once their note is opened or where every line stays`)

### Requirement: Views: A quiet line and a note are named to a screen reader

A line of a quiet run MUST say what it is to a screen reader after its type ("inserted, formatting only"; a line
shown once "formatting only"), as a changed line says its type. A note that opens MUST be a button named by its words
and by what a press does, with its expanded state. A line of a move MUST say which end of it the line is at and
that it was edited on the way ("moved away", "moved here, edited on the way"), and the place its note names is a
button named by where it goes.

#### Scenario: The wrapped call
- **WHEN** Inline shows the wrapped call
- **THEN** its lines carry the kind of their run, which the generated content of the line reads out
- **AND** the note is the button "Formatting only, 3 lines: Show the old line", not expanded; opened, "… Hide the old line", expanded
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`Inline shows a wrapped call once, played down after its note, and a click on the note shows and hides its old line`)

#### Scenario: A move
- **WHEN** Inline shows the moves of the `commit_marks` template
- **THEN** their lines carry the end of the move they are at, which the generated content of the line reads out
- **AND** the note where `accounts()` left is the button "5 lines moved to line 6: Show the 5 old lines", not expanded; opened, "… Hide the 5 old lines", expanded
- **AND** the places are the buttons "Go to line 6, where the lines stand now" and "Go to line 22, where the lines were"
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`Inline shows the lines of a move once where they stand now, an edited one as the row of its two lines, and one note that opens where they left`, `a click on the place a note of a move names shows its other end and marks the note there, without a key in the preview`)

### Requirement: Views: N and P stop where the reading of a block starts

N and P MUST stop only at the starts of a diff: at the first row of lines of what is read from the line `starts`
names. Rust cuts a block by its quiet runs alone, so lines deleted before a label are read from the first of them,
and one column shows the old lines of a part first; the lines a move took end what is read before them. A block
without a start is passed, and so are a block of the imports alone in a review with its plan and a block another
section shows.

#### Scenario: The stops of the template
- **WHEN** the user presses N through the review of the `commit_marks` template, then P
- **THEN** N stops in Ledger.kt as many times as its header says is left to read, fewer than its eleven blocks, which hold the wrapped call, then three times in Report.kt and once in Store.kt
- **AND** each stop is a changed line four rows below the header of its file, after a row that is none, further down than the stop before it; no line of the wrapped call is one
- **AND** `accounts()` and `describe`, which moved, are stops where they stand now, each at its first line under the note of its move, and no stop where they left
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

#### Scenario: The lines of a move
- **WHEN** a block is the lines of a move where they stand now, a block lost every old line to a move, and a block lost some of its lines, in each layout
- **THEN** the first stops at the first line of the move, under its note; the second has no stop; the third stops at a line that stayed, one column at the old line of that part first
- **AND** a block that starts with lines a move took is read from the line after them, the lines shown or behind their note
  [@test] src/lib/diff/rows.test.ts (`stops N and P where the lines stand now and never where they left, and lists the notes of both ends`)
  [@test] src/lib/diff/block-parts.test.ts (`cuts the lines of no run at the ends of the moves, each on its own side, and reads a block from its first line no move took`)

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

### Requirement: Views: The rows of the marks keep the layout arithmetic

A note MUST be one row, and a line shown once as many visual lines as its new line takes, so every offset of the
view is known before a paint. The one column of Inline MUST scroll sideways no further for a pair that a quiet run
shows once, and as far as the row of an edited line of a move, the pair of its two lines, and as far as the pair of a
block in which a review with its plan draws no move.

#### Scenario: Heights
- **WHEN** a quiet run of Inline is laid out, with soft wraps off and on
- **THEN** its note and each of its lines are a row, a wrapped line as many visual lines as its new line
  [@test] src/lib/diff/layout.test.ts (`keeps a note of a quiet run at the top by the old line it stands for, and lays a note and a line shown once out as a row each`)

#### Scenario: The widest row of Inline
- **WHEN** a pair of a block lies in a quiet run
- **THEN** the widest row of Inline counts the row of the other pairs alone
  [@test] src/lib/diff/inline-rows.test.ts (`counts no row of a pair in a quiet run, which Inline shows once by its new line`)

#### Scenario: The rows of a move
- **WHEN** a move is laid out in Inline, with soft wraps off and on
- **THEN** the note where its lines left and the note where they stand now are a row each, a line of the move as many visual lines as its text takes
  [@test] src/lib/diff/layout.test.ts (`keeps the note of the lines a move took at the top by the old line it stands for, and the note of where they stand by the line after it; each a row`)

#### Scenario: The row of an edited line of a move
- **WHEN** a line of a move was edited on the way, and when it moved as it was
- **THEN** the widest row of Inline counts the first, its new line with the deleted part of its old line, and no row of the second
  [@test] src/lib/diff/inline-rows.test.ts (`counts the row of an edited line of a move, the pair of its old line and its new one where the line stands now`)
  [@test] src/lib/diff/block-parts.test.ts (`lists the edited lines of the moves that stand in a block as rows of two lines, with the ranges of their move`)

#### Scenario: The widest row of Inline in a review with its plan
- **WHEN** the engine pairs the two lines of a block of the imports alone and the marks call its old line a move, or the move stands in such a block
- **THEN** the widest row of Inline counts the row of the pair, which a review with its plan shows since it draws no move there, though outside a review the move takes the old line of the pair
  [@test] src/lib/diff/inline-rows.test.ts (`counts the pairs of a block of the imports alone as a review with its plan shows it too, which draws no move there`)

#### Scenario: The scroll of the column in a review with its plan
- **GIVEN** a commit that gives a long import a longer name in place and adds its old form again below, in a review with its plan, in Inline
- **WHEN** the section of the imports is opened and the scroll bar of its column is at its end
- **THEN** the row of the two lines of the import, wider than its column, shows its end
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`the row of two lines of an import that a review plays down is reached by the scroll of its column, though its old line is a move to the engine`)

### Requirement: Views: The quiet colour is one token of both themes

A line of a quiet run and the note before it MUST be drawn in one colour of their own, the token `--diff-quiet` of
each theme: no colour of a change, weaker than each of them, with the number of the line readable on it.

#### Scenario: The light and the dark theme
- **WHEN** the wrapped call shows in the light theme, and in the dark one
- **THEN** its lines and its note have the colour the token names, light in the light theme and dark in the dark one
- **AND** an inserted line and an unchanged one have another
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`a quiet line and its note have the quiet colour of the light theme, which is no colour of a change`, `a quiet line and its note have the quiet colour of the dark theme, which is no colour of a change`)

### Requirement: Views: A note at each end of a move says where its other end is

A view MUST show a note before the lines of a move at each of its ends: where they left, how many lines moved and to
which line ("14 lines moved to line 210"); where they stand now, from which line, whether they were indented again
and how many were edited on the way ("Moved from line 120 · re-indented · 1 line edited"). The line is the first line
of the other end, the blank lines that moved with the move among them. A move with an end in no block MUST change no
row.

#### Scenario: The words
- **WHEN** a note words where fourteen lines left, where they stand now, and a move that was indented again with two lines edited
- **THEN** it says "14 lines moved" "to line 210" and "Moved" "from line 120", then "re-indented · 2 lines edited", nothing where the lines moved as they were
- **AND** the line is the first line of the other end as a reader counts, a blank line that moved with the lines among them
  [@test] src/presenters/diff-marks.test.ts (`says where the lines that left stand now, by the first line of that end, and what its buttons do`, `says where the lines that stand now were, whether they were indented again and how many of them were edited on the way`)

#### Scenario: The moves of the template
- **WHEN** the preview of the `commit_marks` template shows Ledger.kt, whose `accounts()` moved up in its class as it is and whose `describe` moved into the class, indented, a space added in one line
- **THEN** the notes where they stand now say "Moved from line 22" and "Moved from line 46 · re-indented · 1 line edited"
- **AND** the notes where they left say "5 lines moved to line 6" and "7 lines moved to line 19"
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`Inline shows the lines of a move once where they stand now, an edited one as the row of its two lines, and one note that opens where they left`)

#### Scenario: The ends of a block
- **WHEN** the marks hold a move, one with blank lines beside it, two whose old lines are in another order than their new ones, and one with an end in no block or in a block the policy ignores
- **THEN** each block has the ends of its moves in the order of its lines, the blank lines among the lines of an end and never out of the block
- **AND** the move with an end in no block changes no row: every line of every block shows
  [@test] src/lib/diff/block-parts.test.ts (`gives each block the ends of its moves, the blank lines that moved with them among their lines, and none of a move with an end in no block`)
  [@test] src/lib/diff/rows.test.ts (`says the words of an end of a move that starts its block in the heading of the change, and leaves a move with an end in no block out`)

### Requirement: Views: Inline shows the lines of a move once, where they stand now

Inline MUST show the lines of a move once: where they stand now, after their note, in the colour of a move, an edited
line as the row of its old line and its new one with the ranges of the move. Where they left it MUST show the note
alone, which stands for the old lines and opens as the note of a quiet run does: a click, ⌥-click and H show them
after it, in the colour of a move.

#### Scenario: The moves of the template
- **WHEN** Inline shows Ledger.kt of the `commit_marks` template
- **THEN** the lines of `accounts()` and of `describe` show once after their notes, new lines of the move with their new numbers alone
- **AND** the edited line of `describe` is one row with both its numbers, pale, the space it has more in the full colour and nothing struck through
- **AND** where the lines left a note stands alone, no row of them
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`Inline shows the lines of a move once where they stand now, an edited one as the row of its two lines, and one note that opens where they left`)
  [@test] src/lib/diff/rows.test.ts (`shows the lines of a move once in Inline, where they stand now, and one note that opens where they left`)
  [@test] src/lib/diff/block-parts.test.ts (`shows the lines of a move once where they stand now, after a note, an edited one as the pair of its two lines in Inline`)

#### Scenario: The note where the lines left opens
- **WHEN** the user clicks the note where `accounts()` left, then clicks it again; and in a review window presses H with the note where `describe` left in view, clicks it, presses H twice, then ⌥-clicks it, and ⌥-clicks the note where `accounts()` left
- **THEN** the click shows the blank line and the four lines after the note, deleted rows in the colour of a move, and the second click hides them
- **AND** H shows the seven old lines of `describe`, the edited one pale, the click hides them, H shows and hides them again, the diffs keeping the focus
- **AND** ⌥-click opens every note of the file that opens, the notes where both functions left, and no note where lines stand now; ⌥-click on an open one closes them all
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`Inline shows the lines of a move once where they stand now, an edited one as the row of its two lines, and one note that opens where they left`, `H and a click show the lines a move took after the note that stands for them, and hide them again`)
  [@test] src/lib/diff/block-parts.test.ts (`stands a note for the lines that left in Inline until it is opened, and shows them after it elsewhere`)

#### Scenario: The cells of a move
- **WHEN** the cells of a move are drawn, a line as it was and an edited one, at both ends, in a block with ranges of its own
- **THEN** each is a cell of a move, full as it was and pale where it was edited, with the ranges of the move and never those of its block
  [@test] src/lib/diff/row-cells.test.ts (`types a line of a move as its own cell at both ends in every layout, and draws the ranges of the move in an edited line alone`)

### Requirement: Views: Unified and Split keep every line of a move at both ends

Unified and Split MUST show every line of a move at both ends, in the colour of a move, an edited line pale with the
ranges of its side of the move, after notes that open nothing. Split MUST show the lines that left on the old side
and the lines that stand now on the new side, the other side filled, each note over the column of its lines.

#### Scenario: The moves of the template in Unified and Split
- **WHEN** the user switches the preview of Ledger.kt to Unified, then to Split
- **THEN** Unified shows the lines of both moves as inserted rows of the move where they stand and as deleted rows of the move where they left, the edited line pale at both ends; the notes are plain words
- **AND** Split shows the lines that stand now on the new side and the lines that left on the old side, the other side filled beside them, each note over the column of its lines
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`Unified and Split keep every line of a move at both ends, in the colour of a move, after notes that open nothing`)
  [@test] src/lib/diff/rows.test.ts (`keeps every line of a move in Unified and Split, both ends rows of the move after a note that opens nothing`)
  [@test] src/lib/diff/row-cells.test.ts (`types a line of a move as its own cell at both ends in every layout, and draws the ranges of the move in an edited line alone`)

### Requirement: Views: The lines of no move beside a move show as the block does

A view MUST cut the lines of a block between its runs at the ends of its moves, each end on its own side, and show
the lines of no move as the block shows without marks. In lines the engine pairs, a cut of one side MUST cut the
other at the same line, so the lines beside a move keep their pairs. Every line of a block MUST show once whatever
the marks say: by its own row, or behind the note that stands for it.

#### Scenario: A block that lost some of its lines
- **WHEN** a block changes a line, loses three lines to a move and deletes two more
- **THEN** each layout shows the changed line as the block does, the note of the move (with its lines where the layout keeps them), then the two deleted lines
  [@test] src/lib/diff/block-parts.test.ts (`cuts the lines of no run at the ends of the moves, each on its own side, and reads a block from its first line no move took`)

#### Scenario: Lines the engine pairs
- **WHEN** a block of four similar pairs loses its third old line to a move, or a line of a move stands at its third new line
- **THEN** Inline shows the pairs before and after as a row each, and the line whose pair moved alone
  [@test] src/lib/diff/block-parts.test.ts (`keeps the pairs of the lines beside a move in a block the engine pairs, and shows the line whose pair moved alone`)

#### Scenario: Any marks
- **WHEN** blocks get runs and moves at random, valid or not: overlapping each other, reaching past their block
- **THEN** in every layout, the notes closed and opened, each line of the block shows once: by its own row, or behind the note that stands for it
  [@test] src/lib/diff/block-parts.test.ts (`shows every line of a block once whatever the marks say: by its own row, or behind the note that stands for it`)

### Requirement: Views: A move with an end in a block that is played down is drawn at neither end

A view MUST draw a move at both its ends or at neither. A block of the imports alone that a review with its plan
plays down MUST draw no end of a move: its lines show as they did, pale, every one of them, without a note, and it is
no stop. At the other end of such a move, in a block that is not played down, the lines of the move MUST be lines of
their block, each with its row in every layout, with no note: none is hidden, and an edited one shows what it was.

#### Scenario: Imports that were sorted
- **WHEN** an import moves within the blocks of the imports alone, in a review with its plan and outside one
- **THEN** in the review both blocks show their lines pale, none left out, with no note of a move and no stop; outside it the move shows with its notes and its stop
  [@test] src/lib/diff/rows.test.ts (`draws a move with an end in a block of the imports alone that a review plays down at neither end: every line a row of its block, no note; elsewhere the move shows`)

#### Scenario: An import that moves into a block with code
- **WHEN** an import leaves a block of the imports alone for a block with code beside it, in a review with its plan, in Inline, Unified and Split
- **THEN** the block that is played down shows its line pale, and where the line stands now it is an inserted line of its block, with no note
- **AND** the block with code is read from its first row, as a block without a move
  [@test] src/lib/diff/rows.test.ts (`draws a move with an end in a block of the imports alone that a review plays down at neither end: every line a row of its block, no note; elsewhere the move shows`)

#### Scenario: Imports that leave a block with code, one edited on the way
- **WHEN** four imports leave a block with code for a block of the imports alone, the second edited on the way, in a review with its plan, in Inline, Unified and Split
- **THEN** the block they left shows them as its deleted lines, each with its row, the old line of the edited import among them: no note stands for a line
- **AND** where they stand now they are pale inserted lines, with no note, no row of two lines and no stop; the one stop is the first row of the block with code
- **AND** a move between two blocks with code in the same review shows with its notes
  [@test] src/lib/diff/rows.test.ts (`draws a move with an end in a block of the imports alone that a review plays down at neither end: every line a row of its block, no note; elsewhere the move shows`)
  [@test] src/lib/diff/block-parts.test.ts (`leaves a move with an end in a block a view plays down out of both its blocks, and no other move`)

#### Scenario: A review with its plan
- **GIVEN** a commit that puts four imports of a file in order, two of them moves to the engine, and changes a line of its body
- **WHEN** the review with its plan shows the section of the imports, opened
- **THEN** the imports that left and where they stand now are deleted and inserted lines, pale, none in the colour of a move, and no note of a move stands in the review
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`imports that were sorted stay the pale lines of a block the review plays down, with no note of a move`)

#### Scenario: Imports that left the block of a comment, in a review with its plan
- **GIVEN** a commit that puts four imports before the others, one of them from another package now, and deletes the comment that stood before them
- **WHEN** the review with its plan shows the section of the file and the section of the imports, opened
- **THEN** the comment and the four imports as they were are deleted lines of the section of the file, every one shown
- **AND** the four imports as they are now are pale inserted lines of the section of the imports; no note of a move, no line in its colour and no row of two lines stands in the review
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`imports that left a block with code for the imports, one edited on the way, stay the deleted lines of their block, none behind a note of a move`)

### Requirement: Views: The place a note of a move names shows its other end

The place a note of a move names ("to line 210", "from line 120") MUST be a button: a click shows the note at the
other end a third of the view below its top, marked as where the view landed, its file open. In a review window the
end the jump started from MUST be kept for Back, and Forward goes there again, the diffs holding the focus; the
preview of a commit keeps no history and moves no focus.

#### Scenario: A click in the preview
- **WHEN** the user clicks "from line 22" of the note where `accounts()` stands now, then "to line 6" of the note the view went to
- **THEN** the first click shows the note where the lines left, marked, and the second the note where they stand now, the first one marked no more
- **AND** in Unified and in Split, where the lines stay at both ends, the same clicks go there and back
- **AND** the buttons are named by where they go ("Go to line 22, where the lines were", "Go to line 6, where the lines stand now") and name no key
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`a click on the place a note of a move names shows its other end and marks the note there, without a key in the preview`)

#### Scenario: A click, Back and Forward in a review window
- **WHEN** the user clicks the place of the note where `accounts()` stands now, presses Back, Forward, clicks the place of the note there, and presses Back
- **THEN** the click shows the note where the lines left, marked, the diffs holding the focus; Back returns to the note where they stand, Forward goes to the other again
- **AND** the click there is a jump of its own: Back returns from it to where the lines left
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`a click on the place a note names jumps to the other end of the move, Back returns and Forward goes there again`)

#### Scenario: In a review with its plan
- **GIVEN** a commit that moves a function to the start of its class with one line edited, and one that moves it as it is, each in a review with its plan
- **WHEN** the user clicks the place in the heading of the change of the first, then presses G twice; and in the second expands the folded section of the moves, clicks the place of a note, presses G and Back, folds the section again and presses Forward
- **THEN** the heading says the words of the move after the change ("compute — body changed · Moved from line 121 · 1 line edited") and names the key; the click shows the note where the lines left, marked, G returns to the row of the heading, and G goes there again
- **AND** in the section of the moves the jump goes there and back, and Forward to a place in the section folded again opens it
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`a move with an edited line says its words in the heading of the change: a click on its place and G go to where the lines left and back to the heading`, `a move as it was is in the folded section of the moves: a jump in it goes there and back, and Forward opens the section folded again`)

#### Scenario: The row a jump lands on
- **WHEN** the row of the first line of an end of a move is asked for a jump, the note where the lines left closed and opened, and of any other line; and of the line a note names where blank lines of the move reach out of the block of its other end
- **THEN** it is the note of that end: the row of words right above its first line, or the note that stands for the line; any other line is its own row
- **AND** the line a note names is the first line the other end shows in its block, so the jump lands on the note there in every layout
  [@test] src/lib/diff/rows.test.ts (`finds an old line of a move at the note that stands for it, and lands a jump to an end on its note`, `lands a jump on the note of the other end by the line that end has, also where blank lines of the move reach out of its block`)
  [@test] src/lib/diff/block-parts.test.ts (`gives each block the ends of its moves, the blank lines that moved with them among their lines, and none of a move with an end in no block`)

### Requirement: Views: G shows the other end of a move in view

In a review window G MUST show the other end of a move from one note of an end in view: the note of the end the view
stands at while it is in view (where a jump, Back or Forward landed, or the note N or P stopped under), else the first
one, so two presses go there and back. Only the button of the place of that note MUST name the key, the key MUST move
nothing with no such note in view, and it MUST be the key of no action of the window.

#### Scenario: The key from the tree and from the diffs
- **GIVEN** the review of the `commit_marks` template at the top of Ledger.kt, the focus on its row of the tree
- **WHEN** the user presses G twice, then Back, then shows Report.kt and presses G and N
- **THEN** the first press shows the note where `accounts()` left, marked, and gives the diffs the focus; the second returns to the note where it stands now; Back returns along the second jump
- **AND** in Report.kt, which has no note of a move, G moves nothing: N goes on from the top of the file
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`G jumps from the note of a move in view to its other end and back, from the tree of the files and from the diffs`)

#### Scenario: The hint of the key
- **WHEN** the notes of two moves show in a review window, the user clicks the place of one, and in the preview of a commit
- **THEN** in the review one button of a place names the key G: of the first note of a move in view, then of the note the click landed on; in the preview none does
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`a click on the place a note names jumps to the other end of the move, Back returns and Forward goes there again`, `a click on the place a note of a move names shows its other end and marks the note there, without a key in the preview`)

#### Scenario: One note at a time
- **GIVEN** the review of a commit that moves two lines of a file up, each to a place of its own, so both ends of both moves are in view at once
- **WHEN** the user presses N twice, G twice, clicks the place of the first move, presses Back, then G
- **THEN** one button names the key at a time: of the first note of a move in view, then of the note N stopped under though the first is still in view above it, then of each note a jump or Back landed on
- **AND** G goes from the note that names it: from the note N stopped under to where its line left and back, and after Back from the note Back returned to, never from the one the click landed on
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`one note of a move names the key G at a time: the first one in view, the one N stopped under, the one a jump or Back landed on; the menu names the key on that end alone`)

#### Scenario: The note the key takes
- **WHEN** the note of the key is asked of a layout of two files with notes of moves: the view at the top, scrolled, and standing at an end whose note is in view or out of it
- **THEN** it is the note at the end the view stands at while its row is whole below the stuck header and above the end of the view, else the first note of a move in view, of the next file where the first has none; none with no note in view
  [@test] src/lib/diff/layout.test.ts (`gives the key of the moves one note: the one at the end the view stands at while it is in view, else the first note of a move in view`)

#### Scenario: No key of an action
- **WHEN** the keys of the review are compared with the keys that the actions of a branch and of a pull request name in a review window
- **THEN** G is the key of none of them, and the review answers it by its physical key
  [@test] src/lib/review-keys.test.ts (`takes no letter that an action of a window of the review names as its key: O stays the pull request's`, `answers the action of each of its keys by the physical key, and none for another key`)

#### Scenario: The notes of the moves of a file
- **WHEN** the rows of a file with a move are built in each layout
- **THEN** the notes at both ends are listed with their rows, in the order of the rows
  [@test] src/lib/diff/rows.test.ts (`stops N and P where the lines stand now and never where they left, and lists the notes of both ends`)

### Requirement: Views: The menu of the code on a line of a move shows its other end

The menu of the code MUST have the item "Go to Other End of Move" on a line of a move and on the note at its end,
which shows the other end as a click on the place of the note does; opened by a menu key it is on the line the
selection starts in. The item MUST show the key of the review only where the key takes the note of that end, and on
any other row the menu MUST have no such item.

#### Scenario: The item on a line and on a note
- **WHEN** the user opens the menu of the code on a line of `accounts()` where it stands now and chooses the item, then on the note where the lines left, then on a line of `describe`, the other move in view, then on an unchanged line
- **THEN** the first has the item with the key G and shows the note where the lines left, marked, the second the note where they stand now
- **AND** the item on the line of `describe` names no key: the key takes the note the view landed on
- **AND** the menu of the unchanged line has "Go to Declaration" and no such item
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`the menu of the code on a line of a move and on the note at its end shows the other end`)

#### Scenario: A menu key
- **WHEN** the user selects a word of a line of `accounts()` where it stands now, presses Shift+F10 and chooses the item, then presses Shift+F10 again
- **THEN** the menu opens with the item and its key, which shows the note where the lines left, marked, the diffs holding the focus
- **AND** the jump selected nothing, so the second menu has "Go to Declaration" and no such item, and the note the view landed on names the key
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`the menu of the code a key opens on a line of a move shows the other end, and has no such item once the jump left nothing selected`)

#### Scenario: The key of the item
- **GIVEN** the review of a commit with two moves, every note of them in view
- **WHEN** the user opens the menu on a line of each move, before and after N stops under the note of the second
- **THEN** the item names the key on the line of the move whose note takes the key, and none on the line of the other
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`one note of a move names the key G at a time: the first one in view, the one N stopped under, the one a jump or Back landed on; the menu names the key on that end alone`)

#### Scenario: The item
- **WHEN** the items of the menu are built on a row of no move, on an end of a move with the key, and on one without it
- **THEN** the first has no such item; the others have it, with the key only where one is given, and say where it goes from that end
  [@test] src/lib/diff/code-menu.test.ts (`offers the other end of a move on a line of a move or the note at its end, with the key of the view where it has one`)
  [@test] src/presenters/diff-marks.test.ts (`names the action that shows the other end of a move, and says where it goes from each end`)

### Requirement: Views: The colour of a move is two tokens of both themes

A line of a move and the note at each of its ends MUST be drawn in the colour of the token `--diff-moved` of each
theme, a line edited on the way in `--diff-moved-pale`: the colour of no other line of a diff. A rail down the start
of the lines of a move and of its notes, which no other line has, MUST mark a move without its colour.

#### Scenario: The light and the dark theme
- **WHEN** the moves of the template show in the light theme, and in the dark one
- **THEN** their lines and their notes have the colour `--diff-moved` names, the edited line the one `--diff-moved-pale` names, light in the light theme and dark in the dark one
- **AND** no token of an inserted, a deleted, a modified or a quiet line, pale or full, names either colour
- **AND** the lines and the notes of a move have the rail; an inserted line, an unchanged one and the note of a quiet run have none
  [@test] tests/e2e/commit-files/moved-lines.spec.ts (`the lines of a move and the notes at its ends have the moved colour of the light theme, which is the colour of no other line`, `the lines of a move and the notes at its ends have the moved colour of the dark theme, which is the colour of no other line`)

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
- The note at an end of a move: what happened in `font-medium` ("14 lines moved", "Moved"), then the place of the
  other end, a button with a dotted line under it ("to line 210", "from line 120"), then ` · ` and what else happened
  on the way ("re-indented · 1 line edited"). Its row has the colour of a move, as its lines, its words nearer the
  text colour than the muted words of other notes. Where the lines left in Inline the first words are the button that
  shows them, with the chevron and the name "14 lines moved to line 210: Show the 14 old lines". With a column per
  side the note lies over the column of its lines. The name and the hover of the place: "Go to line 210, where the
  lines stand now", "Go to line 120, where the lines were".
- A line of a move: `--diff-moved` over the line and its number, `--diff-moved-pale` with its edited words in the
  full colour where it was edited on the way, and a rail 2 px wide down its start
  ([UI Theme](../../../../docs/design/ui-theme.md#diffs)). To a screen reader: "moved away", "moved here", with
  ", edited on the way".
- G in a review window (go): the other end of a move, from one note in view: the note of the end the view stands at,
  else the first one. The button of the place of that note alone shows the key after its words, as H on a note that
  opens, so with several notes in view the hint says which one G takes. The item of the menu of the code: "Go to Other
  End of Move", with the key where the menu opened on that end, its hover "Show where the moved lines stand now" or
  "Show where the moved lines were".
- The row a jump lands on: a ring of `--ring` that fades in 1.6 s; it stays until the next scroll, press or key
  where the OS asks for reduced motion.
- N and P: the next and the previous stop ([Commit Review](../commit-review/spec.md#requirement-the-sidebar-and-the-diffs-n-and-p-show-the-next-and-the-previous-change)).

## Testing / Local Run

`pnpm test:unit src/lib/diff src/stores/file-diffs.test.ts src/presenters/diff-marks.test.ts` and
`pnpm e2e tests/e2e/commit-files/quiet-changes.spec.ts tests/e2e/commit-files/moved-lines.spec.ts` for the views
(`pnpm e2e:webkit` with the same files for the focus after a jump and a click on a note).
`cargo nextest run -p file-diffs -E 'test(/marks_test|marks_renames_test|marks_subject_test/)'` and
`-p text-diff -E 'test(/model_test|moves_test|moves_across_test/)'`; the pass over a subject through the service and
the index: `-p file-diffs -E 'test(/service_test|accio_test/)'` (one test asks the accio of the machine, when it
answers `renames`, of a repository it indexes under a home of its own); the question of a window to the index:
`-p app-api -E 'test(/diffs_test/)'`.
`MARKS_EDITS=600 cargo nextest run --release -p file-diffs -E 'test(the_marks_of_edited_files)'` edits each file of
the corpus 600 times (12 by default), `MARKS_RENAMES=200 … -E 'test(the_renames_of_files)'` renames a word of each
200 times, alone and beside edits (twice by default). The E2E template of the marks is `commit_marks` (`crates/test-utils/src/repo_template.rs`):
`src/Ledger.kt`, `src/Report.kt` and `src/Store.kt` with a case of every kind of mark, each in a block of its own.

## Measurements

`cargo bench -p file-diffs --bench marks_bench`: the tokens of a text, `marks_of` of a pair, the renames of a file at
the limit of their work, the moves of a file of 10,000 lines and of the file a search costs most in, a block at its
limits and a subject of 200 files through the service, with renamed locals and moved functions too; the numbers and what they decide (the limit of the search for moves by them) are
in
[Diff View](../../../../docs/design/diff-view.md#marks).

## Open Questions / Risks

- Quiet is switched on for formatting. The kind `renamed` waits for its measurement: a sample of its proven runs
  (`PairMarks::proven`) of real ranges of a large repository, read by a person. `is_quiet` switches it on when the
  sample holds no change of behaviour; the three places of the local of the `commit_marks` template are passed then
  ("5 to read" of `Ledger.kt`), and the uses of a name that is not local follow with the pass that confirms them.
  The same measurement may switch a kind off.
- A rename is proven by scopes read without types and without the other files
  ([Name Bindings](../name-bindings/spec.md), Open Questions): a bare read of a member a class inherits that a renamed
  local would take is not seen, and neither is a reader of names outside the file that leaves no trace in it
  (reflection over the names of the parameters of a Java method under no annotation, a C# method that takes the text
  of its argument by `[CallerArgumentExpression]`).
- The places of a rename are asked to be in one block each and in the order of the texts; only within a run are
  the two tokens of a place proven to stand at one place of the code. A rename with a place on a rewritten line is
  still a rename, and that line is read.
- A file with more names to ask than its work pays for has the renames of the first of them in the text; the others
  stay stops. One of two renames of a file can be marked while the other is not.
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

- The runs the index confirms (`subject.confirmed`) are quiet with the kind `renamed`, not before: the same
  measurement samples them. accio ties a use by the scopes of its file for Kotlin and Java only, and the type of a
  chain past its first name, the overload a call binds and the receiver of a lambda are still the word of its
  resolver (its decision 2026-11-11-rename-read-in-two-trees): what the sample must show before the kind is on.
- The answer of the index comes after the rows of a file: a view that passes a confirmed run still shows each of its
  lines, where a quiet run of the pair is shown once. The same run reads otherwise by where its proof came from.
- A use of the renamed name with another word written (an accessor of a Kotlin property in Java, an alias) on a
  line the change left alone is a use outside the change to this reader, though it still names the declaration: such
  a rename stays a label.
- A file that uses a renamed name and also adds or deletes a use of it is no rename of that file (not every place
  changed in place, [Renames](#requirement-renames-a-use-left-behind-a-name-taken-and-two-names-swapped-are-no-rename)):
  its calls are read; the added or deleted use in another file stands in the way of nothing.
- The working tree is never asked of the index: nothing pins the tree accio reads to the one the diffs show.
- A question to the index whose subject no view shows any more runs to its end when the window asks of no other
  subject: its answer is kept, and one process a window is the most. Ending it as its last view goes would end it
  for a view that is only hidden for a moment, and ask it again when the view is back.
- Where the indentation has a meaning, a block that another file holds whole at another depth (a function that
  became a method) is no move between files: a move between files has no word for the shift a move within a file
  names (`indent`).
- The 48 characters and the two lines of a move between files are first values. The same lines in two files of a
  change (a licence header, generated code) that leave one and enter the other are a move to the first: a label.
- A line renamed in the last line of its file, with lines added after it, is no run: the last token of a text is
  the same token only as the last ([Equal by Tokens](../equal-by-tokens/spec.md)).
- The 24 characters of a move and its quarter of edited lines are first values, to be set by the same measurement.
  A move is a label, so a wrong one costs a reader a look, never a change passed unread.
- A move whose every line has a key that more than 16 changed lines of the new text have is not found (a table of
  rows that repeat): a line of its own in it starts it.
- The whitespace inside a line counts in a move under every policy: under `Ignore` a moved line with other
  whitespace inside is an edited line of its move. Only the whitespace at the ends of a line follows the policy.
- A line that is the same in every character and stayed at its indent while its move shifted is an edited line
  where the indentation has a meaning (a statement at another nesting, but also the text of a string at the first
  column): `edited` names it, and no range can.
- A blank line a block deletes beside lines that moved stays a deleted line, a stop, when the block the lines stand
  in now adds no blank line beside them, or one of another whitespace where every character counts.
- The violet of a move is the blue of a modified line to red-green colour blindness in the light theme: there the
  rail and the words of the note tell a move, not its colour.
- A note names the first line of the other end, a blank line that moved with the lines among them, so the number may
  be of a blank line: it is the line under the note the place shows.
- G takes one note, the one that shows the key: with several moves in view and no end the view stands at, the first.
  Another is reached by a click on its place, or by N to where its lines stand, which gives its note the key.
- The menu a key opens has the item of a move on a selected line of one: after a jump nothing is selected, the menu
  has no such item, and G goes on from the note.
- A move with an end in a block of the imports alone that a review plays down is drawn nowhere in that review: its
  lines are deleted lines where they left and inserted lines where they stand now, with no word that they are the
  same lines. A block with code whose other lines all left with moves has no stop then, though it shows the imports
  that left it: N and P pass them as they pass the imports where they stand.
- In Split an edited line of a move shows at each end with the ranges of its side; only Inline shows its two lines
  in one row.

## References

- [Equal by Tokens](../equal-by-tokens/spec.md): the rule of the tokens
- [Commit Files](../commit-files/spec.md#file-diffs): the diffs, their stream and their caches
- [Diff View](../../../../docs/design/diff-view.md#marks) ("Marks", "The parts of a block"), [Text Diff Engine](../../../../docs/design/text-diff.md) ("Model")
- [Commit Review](../commit-review/spec.md): N, P and the find of a review window
