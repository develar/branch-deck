## MODIFIED Requirements

### Requirement: Renames: A run of a renamed name is quiet only for a value of its body alone

A run of the kind `renamed` MUST be proven, and so quiet, only for a name bound inside a body that is a value of
that body alone on both sides, every name of which the scopes bind, whose code (its member with the heads of the
types that hold it; its file for a name without a member) changed in names alone: each changed line is in a run of
formatting, of comments or of proven renames, changed in the name of its own declaration alone, or holds no token of
code. Any other rename is a label.

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
- **WHEN** the declaration of a renamed local got another initializer or another type, on its line or on a later line of it, a use of it stands on a line with another change, a line of its function changed after its last place, or a local it is given to is renamed with it, in Kotlin, Java, TypeScript and Rust
- **THEN** the lines that changed in the name alone are labels, each read: the name may hold another value there
- **AND** a blank line put between its places leaves its runs quiet; two locals renamed in the lines of each other are proven together, or labels together
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_rename_of_a_local_is_a_label_where_its_declaration_or_a_line_of_a_use_changed_otherwise`)

#### Scenario: The member around the name
- **WHEN** a line of the function of a renamed local changed otherwise, before its first place or after its last: what the lambda of a renamed parameter is given to (the value at the start of its chain, the body of a lambda before it), the type in the head of the function, the second line of an assignment in a loop or of a declaration below the function that reads the name, a statement of the loop of its places after the last of them, another parameter of its function, any other statement of it; or a blank line was put into a string of several lines the name is given, in Kotlin, Java, TypeScript and Rust
- **THEN** the lines that changed in the name alone are labels, each read: nothing says that the line gives the name nothing
- **AND** a line that changed in another function, a comment of the function worded again, a call of it wrapped otherwise and another local of it renamed too leave the runs quiet; the comment and the other function are read
- **AND** a function that got another name with a local of its body leaves that local proven, and one that calls itself by its new name does not
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_rename_of_a_local_is_proven_only_where_its_member_changed_in_names_alone`)

#### Scenario: The head of the type around the member
- **WHEN** the parameter of the primary constructor of a Kotlin class got another type above the value of a property that reads it, the component of a Java record above its compact constructor, or the bound of a type parameter of a Rust `impl` or of a TypeScript class above a method that names it, and a local of that member is renamed
- **THEN** the lines that changed in the name alone are labels, each read: the local may read another thing
- **AND** a line that changed in another member of the type leaves the runs quiet
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_rename_of_a_local_is_proven_only_where_its_member_changed_in_names_alone`)

#### Scenario: A function without a member of its own
- **WHEN** a local is renamed in a function that is the value of an `export default`, in a method of an object that is such a value, or in a lambda given to a constant of a Java enum, to the constructor of the supertype of a Kotlin class or to a decorator of a TypeScript class, and the statement beside its lines gives it another value
- **THEN** the lines that changed in the name alone are labels, each read: the statement of the body that holds the name is not its member
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_rename_of_a_local_is_proven_only_where_its_member_changed_in_names_alone`)

#### Scenario: A comment beside a renamed name
- **WHEN** a comment of the function of a renamed local is worded again with the name
- **THEN** the comment is a run that is a label, read, and the lines of the name are quiet
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_comment_that_changed_beside_a_renamed_name_is_read`)

## ADDED Requirements

### Requirement: Views: A selection stays on its lines as the rows are painted again

A view MUST keep the selection a reader made in its rows while a new model paints them again (the scopes of a file,
the marks of a subject, what the index proved of a name): each end on the row that shows its line now, at the same
place of the text of that row, also where rows came above it. A side of a file that Select All selected MUST stay
selected. An end whose row is gone or shows another text ends the selection.

#### Scenario: Notes that come above a selection
- **GIVEN** the review of the template whose Report.kt says nothing of its subject until the index answers, two lines of `reportTitle` selected and copied
- **WHEN** the answer comes with the marks of the file, two notes above the selected lines
- **THEN** the selection is on the same two lines, in the rows painted for them, and a copy gives the same two lines
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`a selection a reader made in a file stays on its lines through the notes that come above them, and a copy takes those lines before and after`)

#### Scenario: A selection from one file to the next
- **GIVEN** the same review, a selection from a line of `reportTitle` in Report.kt to the end of the declaration in Store.kt, copied
- **WHEN** the answer comes with the marks of Report.kt, two notes between the two ends
- **THEN** the two ends are on the same two lines, and a copy gives what it gave
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`a selection from a line of a file to a line of the next file stays on both lines through the notes that come between them`)

#### Scenario: A row found again by its line
- **WHEN** a row of a line, the note above the lines of a move and a line that a closed note or a fold hides are looked for in other rows of their file
- **THEN** the row of a line and the note above lines are the rows they were; a line no row shows has no row; a note whose next line is another one now is told by the text its row shows
  [@test] src/lib/diff/rows.test.ts (`finds a row again in other rows of its file by a line beside it: a line, the note above lines, and no row a fold took`)

### Requirement: Views: The rows of a file stay attached across the renders of its section

The rows of a file MUST stay attached to their section across its renders: a render paints no row again by itself,
whatever it changes of the section (the height of its rows, its header), and drops no selection.

#### Scenario: A section that grows by two notes
- **GIVEN** two lines of Report.kt selected in a review
- **WHEN** the marks of the file come and its section renders with the height of two rows more
- **THEN** the rows the new model painted are the rows of the section, and the selection put on them stays
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`a selection a reader made in a file stays on its lines through the notes that come above them, and a copy takes those lines before and after`)

#### Scenario: A copy over a quiet run
- **WHEN** the user selects the lines of a quiet run in a review and copies at once, while the scopes of the file, its tokens and the answer of the index may still come
- **THEN** the copy is the lines of the run: the test selects once and waits for none of them
  [@test] tests/e2e/commit-files/quiet-changes.spec.ts (`a copy takes what a quiet run shows: its new lines, and its old line once the note is opened`)
