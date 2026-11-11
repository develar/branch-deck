## ADDED Requirements

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
file and is declared in none of them MUST mark nothing, and the pair MUST hand it out for a reader of the other files.

#### Scenario: A member, a function, a class
- **WHEN** a property, a field, a function or a class is renamed with its uses in the file, in Kotlin, Java, TypeScript and Rust
- **THEN** the rename is not `local`, `declared` names its declaration, each use is a run that is a label, and the block of the declaration has its start
- **AND** a parameter of a TypeScript constructor that declares a property (`public total`) is such a rename, with its uses in the constructor and through `this`
- **AND** a property and a local of its spelling renamed alike are two renames, the local one proven
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_name_that_is_not_local_is_a_label_and_its_declaration_is_read`)

#### Scenario: A name the file does not declare
- **WHEN** the two calls of a function declared in another file get another name
- **THEN** the diff has no rename and no run, and the pair names the two names with their places, without a declaration
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_name_the_file_does_not_declare_is_left_to_the_other_files`)

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

## MODIFIED Requirements

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
