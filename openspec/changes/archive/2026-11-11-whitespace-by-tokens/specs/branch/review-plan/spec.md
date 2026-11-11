## MODIFIED Requirements

### Requirement: Since Your Review: brd review-plan prints the interdiff and its schema version

`brd review-plan` MUST print the branch, `base`, `head` and the `interdiff` (`reviewed_base`, `reviewed_tip`, `commits`
with `id`, `reviewed_id`, `subject`, `state`, `new_only`, `reviewed_only`, `message_change`, and `scope` with `base`,
`head` and `carried`), with `"schema": 32`, and its output MUST be in `brd schema`; the other outputs of `brd` keep
`"schema": 2`.

#### Scenario: The plan since the review
- **WHEN** `brd review-plan` runs on a marked branch
- **THEN** it prints the branch, `base`, `head` and the `interdiff` with `"schema": 32`
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_commits_since_the_review`)

#### Scenario: The output is in the schema
- **WHEN** `brd schema` runs
- **THEN** it lists the output of `brd review-plan`, and the output matches it
  [@test] crates/bd-cli/src/schema_test.rs (`schema_lists_every_output`, `outputs_match_schema`)

#### Scenario: The versions of the schema
- **WHEN** a reader of an older plan reads the versions
- **THEN** version 3 has `files` in place of `risk` and `usages` in place of `fan_in`, version 4 the `units` ([Review Units](../review-units/spec.md)), version 5 the `generated` files with why, the lock files among them, version 6 a deletion as real work (no `mechanical: "deleted"`; the units `deletedFile` and `deletedFiles`), version 7 the `concerns` ([Review Concerns](../review-concerns/spec.md)), version 8 their `elsewhere`, version 9 the name `file`, version 10 the `steps` and the `brief` ([Review Tour](../review-tour/spec.md)), version 11 the `api` and `test` of a unit ([Review Units](../review-units/spec.md)) in place of its `public`, version 12 the `still_bound` and `by_name` usages and `unbroken` in place of `updated`, version 13 the `reached` and `unreached` tests of a step ([Review Tour](../review-tour/spec.md)) in place of `named` and `unnamed`, version 14 the story of each commit once (`steps.commits`, [Review Tour](../review-tour/spec.md#the-story-of-a-commit)) in place of the `body` of a commit of a step, and the `newApi` and `moduleLinks` of a step, version 15 the `parts` of a story where they are `told` in place of its `toldIn`, the `joined` units and the `marks` of a stop, the `shownElsewhere` units of a step, the state `reworded`, the `message_change` of a commit, the `change` of a story and the `removals` of its parts, version 16 the `paragraphs` of an entry of the brief, whole and changed by word, with their `links` and `excerpt`, in place of its `added` and `removed`, and its `blocks` in place of its `block` ([Review Tour](../review-tour/spec.md#brief)); version 19 added repeated replacements to the brief, version 20 a widening as an `added` entry with `widened`, the reach in the forms and the `verdict`, version 21 the form `row` of a paragraph of the brief, the `excerpt` of every changed paragraph of several sentences and no paragraph of front matter or metadata ([Review Tour](../review-tour/spec.md#brief)), version 22 the `table` block of a body ([Commit Row](../../commit/commit-row/spec.md#message-body)) and its `summary`, version 23 the `modules` of the changed files ([Modules](#modules)), the `module` of a file and the `moduleEdges` of a step in place of its `moduleLinks`, version 24 the `groups` of a large step ([Review Tour](../review-tour/spec.md#groups-of-a-step)) in place of the `concern` of a stop, version 25 the `registrations` ([Registrations](#registrations)), version 26 the `texts` of the UI ([UI Texts](#ui-texts)), version 27 the `folder` of each file ([Modules](#modules)), version 28 the `tested` of each group of a large step ([Groups of a Step](../review-tour/spec.md#groups-of-a-step)), version 29 the `entries` of a module the change adds ([Modules](#modules)), version 30 the `title` of a step, of a commit of a step and of a story as `{prefix, text}`, version 31 the forms `before` and `after` of the `api` with each parameter type as the source writes it and a Java parameter as Java writes it, version 32 the `whitespace` of a file of a grammar by its tokens ([Mechanical Changes](#requirement-mechanical-changes-whitespace-is-told-by-the-tokens-of-a-grammar)) and the fingerprints of the units by them ([Review Units](../review-units/spec.md#identity))

### Requirement: Mechanical Changes: A rename, whitespace or imports alone is mechanical

Else, from the text of every hunk of every commit that changes it, a file MUST be `renamed` when each change is a
rename with no line added or removed; `whitespace` when each hunk changes whitespace alone, a line wrapped or joined
too; `importsOnly` when, in a source file, each other hunk has only blank lines and lines of import statements, at
least one.

#### Scenario: A rename, a whitespace change, an imports change
- **WHEN** each change of a file is a rename with no line added or removed, or each hunk changes whitespace alone, or each other hunk of a source file has only blank lines and lines of import statements
- **THEN** it is `renamed`, `whitespace` or `importsOnly`
  [@test] crates/review-plan/src/files_test.rs (`imports_whitespace_renames_and_the_build_model_are_mechanical_a_deletion_is_not`)

#### Scenario: A wrapped line
- **WHEN** a hunk only wraps or joins a line
- **THEN** it changes whitespace alone, and the file is `whitespace`
  [@test] crates/review-plan/src/files_test.rs (`a_wrapped_line_and_a_line_in_an_import_block_are_mechanical`)

### Requirement: Mechanical Changes: A meaningful indent is never whitespace

A file of no grammar whose whitespace has a meaning MUST never be `whitespace`: Python (`*.py`, `*.pyi`), YAML
(`*.yml`, `*.yaml`) and Makefiles (`Makefile`, `GNUmakefile`, `*.mk`). In Starlark (`*.bzl`, `*.star`, `*.bazel`,
`BUILD`, `WORKSPACE`), read by its grammar, the indent of a logical line MUST be real work, and a line continued
inside its brackets whitespace.

#### Scenario: An indent change of a Python file
- **WHEN** the commits only change the indent of a statement of Python or Starlark, of a key of YAML, of a line of a Makefile
- **THEN** it is no `whitespace` change, and its indent stays real work
  [@test] crates/review-plan/src/files_test.rs (`an_indent_where_the_indentation_has_a_meaning_is_no_whitespace_change`)

#### Scenario: A call of Starlark wrapped inside its brackets
- **WHEN** the commits only move the arguments of a call of a `.bzl` file to lines of their own inside its brackets
- **THEN** the file is `whitespace`
  [@test] crates/review-plan/src/files_test.rs (`a_change_of_whitespace_is_told_by_the_tokens_of_a_grammar`)

## ADDED Requirements

### Requirement: Mechanical Changes: Whitespace is told by the tokens of a grammar

A hunk of a file of a grammar MUST change whitespace alone only when its lines are equal by tokens, code and comments
([Equal by Tokens](../../commit/equal-by-tokens/spec.md)); one of a file of no grammar when its text without its
whitespace, line ends included, is the same. A file of a grammar without tokens, and one of a grammar on one side
alone, MUST have no change of whitespace.

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
