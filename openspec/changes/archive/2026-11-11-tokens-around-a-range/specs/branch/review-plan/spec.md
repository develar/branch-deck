## MODIFIED Requirements

### Requirement: Mechanical Changes: Whitespace is told by the tokens of a grammar

A hunk of a file of a grammar MUST change whitespace alone only when its lines are equal by tokens, code and comments,
with the code around them ([Equal by Tokens](../../commit/equal-by-tokens/spec.md)); one of a file of no grammar when
its text without its whitespace, line ends included, is the same. A file of a grammar without tokens, and one of a
grammar on one side alone, MUST have no change of whitespace.

#### Scenario: A space inside a string, a line break that ends a statement
- **WHEN** a hunk of a Kotlin file changes the spaces inside a string, joins a `return` with the call of the next line, turns `a - -b` into `a--b`, or `//go:build` into `// go:build`
- **THEN** the file stays real work
- **AND** so does a TSX file whose hunk changes the spaces inside a line of the text of an element
  [@test] crates/review-plan/src/files_test.rs (`a_change_of_whitespace_is_told_by_the_tokens_of_a_grammar`)

#### Scenario: A line break that is a syntax error
- **WHEN** a hunk of a TypeScript file moves the `=>` of an arrow function to the next line, which changes no node of the tree and no longer parses
- **THEN** the file stays real work
  [@test] crates/review-plan/src/files_test.rs (`a_change_of_whitespace_is_told_by_the_tokens_of_a_grammar`)

#### Scenario: A call wrapped, a chain continued, the text of an element on its own lines
- **WHEN** a hunk wraps the arguments of a call, continues a chain of calls on the next lines, or moves the text of a JSX element to lines of its own
- **THEN** the file is `whitespace`
  [@test] crates/review-plan/src/files_test.rs (`a_change_of_whitespace_is_told_by_the_tokens_of_a_grammar`, `a_wrapped_line_and_a_line_in_an_import_block_are_mechanical`)

#### Scenario: A byte order mark removed
- **WHEN** a hunk only removes the byte order mark a Kotlin file starts with
- **THEN** the file is `whitespace`: the mark is a blank to its parser, as it is to the check before the tokens
  [@test] crates/review-plan/src/files_test.rs (`a_change_of_whitespace_is_told_by_the_tokens_of_a_grammar`)

#### Scenario: A blank line that changes the lines around it
- **WHEN** a hunk adds a blank line after the `\` that continues a line of a `.bzl` file, or before the `#!` a Kotlin script, a Rust file, a C# file or a JavaScript file starts with
- **THEN** the file stays real work: the tokens of the lines around the blank line are others, or the new text has none
- **AND** a file whose hunks add blank lines between two declarations of Kotlin, after the line of a `#!`, or inside the brackets of a `.bzl` file is `whitespace`
  [@test] crates/review-plan/src/files_test.rs (`a_blank_line_is_whitespace_only_where_the_lines_around_it_read_the_same`)

#### Scenario: A zero width space between two tokens
- **WHEN** a hunk of a JavaScript or a TypeScript file puts a zero width space or U+0085 between two tokens
- **THEN** the file stays real work: the new text is a syntax error to an engine of JavaScript
  [@test] crates/review-plan/src/files_test.rs (`a_blank_line_is_whitespace_only_where_the_lines_around_it_read_the_same`)

#### Scenario: A file of a grammar on one side alone
- **WHEN** a file is renamed from `notes.txt` to `Notes.kt`, or from `Source.kt` to `source.txt`, and a hunk of it wraps a line
- **THEN** the file stays real work: nothing proves its two sides the same
- **AND** the same hunk of a file renamed from `.txt` to `.txt`, and from `.kt` to `.kt`, is `whitespace`
  [@test] crates/review-plan/src/files_test.rs (`a_file_of_a_grammar_on_one_side_alone_has_no_change_of_whitespace`)

#### Scenario: A file of no grammar
- **WHEN** a hunk of a file of no grammar has the same text without its whitespace
- **THEN** the file is `whitespace`
  [@test] crates/review-plan/src/files_test.rs (`a_change_of_whitespace_is_told_by_the_tokens_of_a_grammar`)

#### Scenario: A script the parser reads only after a recovery
- **WHEN** a hunk adds a blank line to a script that oxc reads only with the region of an error blanked
- **THEN** the file stays real work: it has no tokens to tell by
  [@test] crates/review-plan/src/files_test.rs (`a_change_of_whitespace_is_told_by_the_tokens_of_a_grammar`)

## ADDED Requirements

### Requirement: Mechanical Changes: A hunk is read by the lines of the diffs

A hunk is a block of the diffs, and its text and its tokens MUST be read by the lines of the diffs: a line ends at
`\n`, `\r\n` or a lone `\r`, where git ends a line at `\n` alone.

#### Scenario: A change below a lone carriage return
- **WHEN** a hunk changes the code of a line below a lone `\r`, or of a file whose lines end with `\r\r\n`
- **THEN** the file stays real work: the hunk is read by the lines of the diffs, never by the lines git counts there
- **AND** a hunk of such a file that wraps a call or adds spaces inside its brackets, and one of a file of no grammar that changes spaces alone, are `whitespace`
  [@test] crates/review-plan/src/files_test.rs (`a_hunk_is_read_by_the_lines_of_the_diffs`)
  [@test] crates/review-plan/src/whitespace_test.rs (`the_lines_of_a_text_are_those_of_the_diffs`)
