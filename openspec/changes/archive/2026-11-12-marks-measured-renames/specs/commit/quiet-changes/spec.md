## RENAMED Requirements

- FROM: `### Requirement: Renames: The runs of a renamed name are labels until their kind is switched on`
- TO: `### Requirement: Renames: The kind of a renamed name is quiet, switched on by its measurement`

## MODIFIED Requirements

### Requirement: Renames: A binding whose every name changed alike is a rename

A rename MUST be one binding of each text with the same places: as many names on both sides, name `i` of the old text
in the block of name `i` of the new one and nowhere the alignment of the tokens put another name, the declarations at
the same places. A name bound inside a body is `local`, and a rename only where a line changed in it alone; any other is every name of its spelling that no body binds.
A rename MUST name its two names, its places on both sides and which of them declare it.

#### Scenario: A local in each grammar
- **WHEN** a local of a function is renamed at its five places, in Kotlin, a Kotlin script, Java, C#, Starlark, Rust, TypeScript, TSX and JavaScript
- **THEN** the diff has one rename, `local`, of five places, declared at its first
- **AND** every changed line is in a run of the kind `renamed` that is proven and quiet: no block has a start
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
- **THEN** the content of the file has the rename and its three runs, quiet, and no start, before the event of its scopes, the same at each read
  [@test] crates/file-diffs/src/service_test.rs (`the_renames_of_a_text_come_with_its_content`)

#### Scenario: The template of the marks
- **WHEN** the diffs of the `commit_marks` template are read
- **THEN** `Ledger.kt` has the rename of its local `total` with three places, each a quiet run whose block has no start, and `Store.kt` the rename of `openStore` with its declaration alone, which is read
  [@test] crates/file-diffs/src/marks_test.rs (`the_marks_template_has_each_case_in_a_block_of_its_own`)

#### Scenario: One variable in the place of another
- **WHEN** a local is gone and another stands at each of its places, every line of them changed otherwise too (`val isRoot = …` read once, `val nameComponent = …` read once where it was), in Kotlin, Java, TypeScript and Rust
- **THEN** the diff has no rename and no run of a renamed name, and every block its start
- **AND** a call wrapped beside those lines is the quiet run of formatting it is without them
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_local_none_of_whose_lines_changed_in_its_name_alone_is_no_rename`)

### Requirement: Renames: Lines that changed in a renamed name alone are a run

The blocks that hold a place of a rename MUST be read again with the two names of each place read as one. Lines whose
tokens are then the same, comments too, and that hold a place MUST be a run of the kind `renamed`, however the lines
were wrapped. A place on a line with another change, or with a comment that changed, MUST be in no run: that line is
read.

#### Scenario: A line with another change
- **WHEN** a local is renamed at its five places and the line of one of them changes in an operator too, in each grammar
- **THEN** the rename has its five places, the line of the other change is in no run, and the other lines are runs that are labels, that line standing between the places of the name: every block is read from its first line
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_place_on_a_line_with_another_change_is_read_in_each_grammar`)

#### Scenario: Wrapped lines
- **WHEN** a call with the renamed name is wrapped over four lines, the name stands in a string template, or a blank line is put between two lines of the name
- **THEN** the lines are one run of the kind `renamed`
  [@test] crates/file-diffs/src/marks_renames_test.rs (`lines_wrapped_with_a_renamed_name_are_one_run`)

#### Scenario: A comment that changed
- **WHEN** the comment at the end of the line of a place changes with the name
- **THEN** the line is in no run and is read, and where it declares the name the lines that read it are labels; a comment on a line of its own is a run of comments beside the run of the rename, also beside such a line
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_comment_that_changed_beside_a_renamed_name_is_read`)

### Requirement: Renames: A run of a renamed name takes no line of formatting

Lines that are the same tokens with the names of a rename read as one and hold no place MUST be the run of formatting
they are without the rename, quiet whatever the rename is, with the blank lines beside them. The second reading of a
block MUST take no run: a run the first reading found MUST lie in a run of its kind, and a run of the second reading
that shares a line with it without holding it, or crosses it, is none.

#### Scenario: Lines of formatting beside the name
- **WHEN** a call is wrapped between or before the lines of a renamed local, two calls are wrapped each before a line of it, or twelve statements are indented inside a new `if`, one of them a use of the local
- **THEN** the wrapped calls and the indented lines are quiet runs of formatting, and each row of lines of the name between them is a run of the kind `renamed`, quiet beside the wrapped calls
- **AND** beside the new `if`, a line that is read between the places of the name, the lines of the name are labels and the reading of the block starts at the first of them
- **AND** a blank line put before or after the wrapped call is in its run of formatting
- **AND** beside a rename that is a label for good (the use of a member, a parameter a call may name, a local of a C# file with an `#if`, a local of a text with a syntax error) the wrapped call is a quiet run of formatting too, and the reading of the block starts at the line of the name
  [@test] crates/file-diffs/src/marks_renames_test.rs (`lines_of_formatting_beside_a_renamed_name_are_a_run_of_their_own`)

#### Scenario: The two readings pair the lines otherwise
- **WHEN** a line with three places of a renamed local changes places with a call that is wrapped: the tokens of the texts pair the call, and with the names read as one the line of the places pairs with itself
- **THEN** the wrapped call is the quiet run of formatting the first reading found, and the line of the places is in no run and is read, so the two other lines of the name are labels
- **AND** of the runs of two readings, a run of the second that holds a run of the first in its kind stays with it, and one that shares a line with a run of the first it does not hold, or crosses it, is none
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_run_of_the_first_reading_stays_where_the_second_aligns_the_lines_otherwise`)

#### Scenario: The runs of the first reading
- **WHEN** the files of the corpus are edited at random, or have a name renamed, and the blocks of each diff are read once more from the tokens of the texts as they are
- **THEN** every run that reading finds lies in a run of its kind of the marks
  [@test] crates/file-diffs/src/marks_test.rs (`the_marks_of_edited_files_are_proven_by_their_tokens`, `the_renames_of_files_with_a_name_renamed_are_proven_by_their_bindings`)

### Requirement: Renames: A run of a renamed name is quiet only for a value of its body alone

A run of the kind `renamed` MUST be proven, and so quiet, only when every place in it is of a name bound inside a
body that is a value of that body alone on both sides, every name of which the scopes bind, and when from the first
place of that name to its last every line that changed is quiet: in a run of formatting, in a run of proven renames,
or blank. Any other rename MUST have runs that are labels.

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

#### Scenario: The lines between the places
- **WHEN** the declaration of a renamed local got another initializer or another type, on its line or on a later line of it, a use of it stands on a line with another change, or a local it is given to is renamed with it, in Kotlin, Java, TypeScript and Rust
- **THEN** the lines that changed in the name alone are labels, each read: the name may hold another value there
- **AND** a line that changed after the last place of the name, and a blank line put between its places, leave its runs quiet; two locals renamed in the lines of each other are proven together, or labels together
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_rename_of_a_local_is_proven_where_only_quiet_lines_changed_between_its_places`)

### Requirement: Renames: The kind of a renamed name is quiet, switched on by its measurement

A kind of run MUST be quiet only once a measurement of real ranges has read a sample of its proven runs and found no
change of behaviour among them (`is_quiet`): `formatting` and `renamed` are. Of a kind that is off every run MUST be
a label, and the pair MUST name its proven runs (`PairMarks::proven`): the marks of a view are the marks as proven
(`PairMarks::as_proven`) but for the quiet of those runs and the starts that follow.

#### Scenario: A renamed local as a view gets it
- **WHEN** a local renamed at its five places is read as a view reads it, in each grammar
- **THEN** the diff has the rename and its runs, quiet, and no block has a start
- **AND** the pair names no run as proven and a label: the marks as proven are the marks of the view
  [@test] crates/file-diffs/src/marks_renames_test.rs (`the_runs_of_a_renamed_local_are_quiet_since_their_kind_is_switched_on`)

#### Scenario: Every diff of the tests
- **WHEN** the files of the corpus are edited at random, or have a name renamed, and each diff is read as proven and as a view reads it
- **THEN** the marks of the view are the proven ones with the runs of a kind that is not switched on as labels, and the starts of those runs
  [@test] crates/file-diffs/src/marks_test.rs (`the_marks_of_edited_files_are_proven_by_their_tokens`, `the_renames_of_files_with_a_name_renamed_are_proven_by_their_bindings`)

### Requirement: Renames: A place that makes more of its name than its binding is read

A place where the name is more than the name of its binding MUST be a place of the rename and in no run: a shorthand
property, a pattern that takes a value apart, a name only the types or what runs tell apart, a name whose spelling is
a value where the code runs (`nameof`, a delegate, the name a function takes), a name among the tokens of a macro or
captured by a format string. The other places of the rename keep their runs, as labels: a line that is read stands between them.

#### Scenario: Such places
- **WHEN** a local declared by a destructuring, a record pattern, an array pattern or a tuple pattern, read by a shorthand property of TypeScript, Rust or C#, passed to a macro, captured by a format string, called as a value or spelled by `nameof` is renamed
- **THEN** the line of that place is in no run and is read, and the lines of its other places are runs that are labels
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_place_that_makes_more_of_its_name_than_its_binding_is_read`)

#### Scenario: A name that is a value where the code runs
- **WHEN** a Kotlin local declared with a delegate is renamed (`val total by map`, `val integrationTest by registering(…)` of a build script, `by lazy` in an `init`), or a local of TypeScript or JavaScript that is given a function or a class written without a name (`const total = () => 1`, `const Total = class {}`)
- **THEN** the line of its declaration is in no run and is read: the delegate reads another key, the task is registered under another name, the function has another `name`; the lines of its uses are runs that are labels
- **AND** a use of a JavaScript local inside the body of a `with` is in no run and is read: there the name may be a property of the object
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_place_that_makes_more_of_its_name_than_its_binding_is_read`)
