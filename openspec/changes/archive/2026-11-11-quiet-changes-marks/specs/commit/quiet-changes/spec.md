## ADDED Requirements

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
