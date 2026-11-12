## ADDED Requirements

### Requirement: Renames: A name that is not local says whether other code declares it now

Of a rename that is not local the pair MUST say whether the code that declares the name changed in more than names
(`redeclared`). It is the same code only when every changed line of the statements of its declarations is in a run
of formatting or of comments, in a run of the kind `renamed` of this name and of locals the scopes prove, or holds
no token of code. A declaration that makes more of its name, and a block the work of the file does not pay for, MUST
count as other code.

#### Scenario: Another value, another body, another type
- **WHEN** a constant gets another value with its other name, a value goes on over a line that changed, a property gets another getter, a function another body or a parameter a call may name under another name, an object another supertype
- **THEN** the name is declared by other code
- **AND** the same value, a value wrapped otherwise, the same body, a body whose local got another name with it, and an object whose member changed are the same code: a type is its head
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_name_declared_by_other_code_is_another_thing_and_the_index_is_not_asked_of_it`)

### Requirement: Subject: A name declared by other code is another thing, and the index is not asked of it

A rename the pair of its declaration says other code declares (`redeclared`) MUST NOT be asked of the index, and
its proof MUST be `otherDeclaration` in the file that declares it and in every file of the subject that uses it,
whatever an index would answer: a line that reads it does something else with the same tokens. No run of it is
confirmed.

#### Scenario: A constant with another value and its uses in another file
- **WHEN** `const val TIMEOUT = 5` becomes `const val LIMIT = 10`, a file calls `pause(TIMEOUT)` at two places that became `pause(LIMIT)`, and the index lists the declaration and both calls in both trees, complete and without a conflict
- **THEN** no declaration is asked, the proof of the name is `otherDeclaration` in both files, and the two calls are runs that are labels, each a stop
- **AND** the same change with the same value is asked, confirmed by that answer, and its calls are passed; the declaration is read either way
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_name_declared_by_other_code_is_another_thing_and_the_index_is_not_asked_of_it`)

## MODIFIED Requirements

### Requirement: Renames: A run of a renamed name is quiet only for a value of its body alone

A run of the kind `renamed` MUST be proven, and so quiet, only when every place in it is of a name bound inside a
body that is a value of that body alone on both sides, every name of which the scopes bind, and when the member
around that name changed in names alone: each changed line of it is in a run of formatting, of comments or of proven
renames, changed in the name of its own declaration alone, or holds no token of code. Any other rename MUST have
runs that are labels.

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
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_rename_of_a_local_is_proven_where_only_quiet_lines_changed_between_its_places`)

#### Scenario: The member around the name
- **WHEN** a line of the function of a renamed local changed otherwise, before its first place or after its last: what the lambda of a renamed parameter is given to (the value at the start of its chain, the body of a lambda before it), the type in the head of the function, the second line of an assignment in a loop or of a declaration below the function that reads the name, a statement of the loop of its places after the last of them, another parameter of its function, any other statement of it; or a blank line was put into a string of several lines the name is given, in Kotlin, Java, TypeScript and Rust
- **THEN** the lines that changed in the name alone are labels, each read: nothing says that the line gives the name nothing
- **AND** a line that changed in another function, a comment of the function worded again, a call of it wrapped otherwise and another local of it renamed too leave the runs quiet; the comment and the other function are read
- **AND** a function that got another name with a local of its body leaves that local proven, and one that calls itself by its new name does not
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_rename_of_a_local_is_proven_only_where_its_member_changed_in_names_alone`)

#### Scenario: A comment beside a renamed name
- **WHEN** a comment of the function of a renamed local is worded again with the name
- **THEN** the comment is a run that is a label, read, and the lines of the name are quiet
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_comment_that_changed_beside_a_renamed_name_is_read`)

### Requirement: Renames: A place that makes more of its name than its binding is read

A place where the name is more than the name of its binding MUST be a place of the rename and in no run: a shorthand
property, a pattern that takes a value apart, a name only the types or what runs tell apart, a name whose spelling
is a value where the code runs (`nameof`, a delegate, the name a function takes, the text C# may hand to what is
called), a name among the tokens of a macro or captured by a format string. The other places of the rename keep
their runs, as labels.

#### Scenario: Such places
- **WHEN** a local declared by a destructuring, a record pattern, an array pattern or a tuple pattern, read by a shorthand property of TypeScript, Rust or C#, passed to a macro, captured by a format string, called as a value or spelled by `nameof` is renamed
- **THEN** the line of that place is in no run and is read, and the lines of its other places are runs that are labels
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_place_that_makes_more_of_its_name_than_its_binding_is_read`)

#### Scenario: A name that is a value where the code runs
- **WHEN** a Kotlin local declared with a delegate is renamed (`val total by map`, `val integrationTest by registering(…)` of a build script, `by lazy` in an `init`), or a local of TypeScript or JavaScript that is given a function or a class written without a name (`const total = () => 1`, `const Total = class {}`)
- **THEN** the line of its declaration is in no run and is read: the delegate reads another key, the task is registered under another name, the function has another `name`; the lines of its uses are runs that are labels
- **AND** a use of a JavaScript local inside the body of a `with` is in no run and is read: there the name may be a property of the object
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_place_that_makes_more_of_its_name_than_its_binding_is_read`)

#### Scenario: The text of an argument of C#
- **WHEN** a local of C# is renamed that is passed to `ArgumentNullException.ThrowIfNull`, checked by `Debug.Assert` and has a method called on it
- **THEN** the lines of those three places are in no run and are read: the exception, the message of the assertion and what the method reports may name another text
- **AND** the lines of its declaration and of its `return` are runs that are labels
  [@test] crates/file-diffs/src/marks_renames_test.rs (`a_place_that_makes_more_of_its_name_than_its_binding_is_read`)

### Requirement: Subject: An answer that is no proof leaves the rename a label and names why

Every other answer MUST leave the rename a label and name why: a use outside the change (`usesOutside`), a name that
means something else (`otherMeaning`), two places that are not one declaration, by the index or by the code that
declares the name (`otherDeclaration`), an answer that does not know every use or names a line the diff has not
(`incomplete`), no answer (`unavailable`), `asked` until then. No answer MUST fail the diffs or read as "no use";
only a whole and final one is kept.

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

### Requirement: Views: The note of a rename says how far the index proved it

The note of a rename that is not local MUST say after its words what the index answered of its uses, where the name
is declared and where it is used: "all uses in this change" for a proof, else "uses outside this change", "changes
what another name means", "more than the name changed" or "uses not all checked". It MUST say nothing while the
index is still to be asked and where none answers; of a name declared by other code it says "more than the name
changed" at once, with no index.

#### Scenario: Every use in the change
- **GIVEN** the template, the index answering that the declaration of the store and its two calls are every use of the name in both trees
- **WHEN** the preview shows Report.kt and Store.kt, once the index answered
- **THEN** the notes say "Renamed openStore → connectStore · 2 sites · declared in Store.kt · all uses in this change" and "Renamed openStore → connectStore · 3 sites in 2 files · all uses in this change"
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`a note says after its words that every use of the name is in the change, where the name is declared and where it is used`)

#### Scenario: A use the change left alone
- **GIVEN** the same, the index listing a call of the old name in a file the commit did not change
- **WHEN** the preview shows the two files
- **THEN** both notes end with "uses outside this change"
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`a note says that the name is used outside the change where the index lists a use the change left alone`)

#### Scenario: No index
- **WHEN** the index of the repository answers nothing of the name
- **THEN** the notes are what they were before it was asked, and say nothing of the uses
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`a note says nothing of the uses of a name where no index answers: the rename is named as before it was asked`)

#### Scenario: The words of each answer
- **WHEN** a note of one rename, of two and of three is worded with each answer of the index
- **THEN** each answer has its words, none for a name still to be asked, without an index or that the index is not asked of
- **AND** in a note of several renames the words name the rename they are about by its new name; a rename the note counts says them in its line of the hover and to a screen reader
  [@test] src/presenters/diff-marks.test.ts (`says how far the index proved a rename: every use in the change, uses outside it, another meaning, more than the name, not all checked`, `tells of which rename of a note of several the subject speaks, by its new name, and of a counted one in its line of the hover`)

#### Scenario: A name declared by other code
- **WHEN** a constant gets another value with its other name, and no index is asked or none answers
- **THEN** the proof of the name is `otherDeclaration` in the file that declares it and in the file that uses it, which the note words as "more than the name changed"
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_name_declared_by_other_code_is_another_thing_and_the_index_is_not_asked_of_it`)
  [@test] src/presenters/diff-marks.test.ts (`says how far the index proved a rename: every use in the change, uses outside it, another meaning, more than the name, not all checked`)
