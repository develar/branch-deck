## MODIFIED Requirements

### Requirement: Subject: The index is asked once of every declaration a subject renamed

The declarations of the renamed names of a subject that are not local, in files of Kotlin or Java, MUST be asked of
the index in one process (`accio renames --no-build`), after its diffs and never by their read, by the window that
shows it. Only an accio that answers them as they are read is started; the working tree MUST NOT be asked. A
final answer MUST be kept by its trees and questions, another asked again, and the query killed once the window
asks another, closes or loads again.

#### Scenario: One process for a subject
- **WHEN** the diffs of a commit are read where an index is, then the index is asked of the commit, of it again, of a range and of the working tree
- **THEN** the read of the diffs asks nothing and its names say `asked`; the question asks every declaration in one call, between the parent and the commit or the two commits of the range, and reads no blob
- **AND** asked again of the same trees it starts no process, and the working tree is never asked
  [@test] crates/file-diffs/src/service_test.rs (`the_index_is_asked_after_the_diffs_of_a_subject_in_one_call_and_its_answer_is_kept`)
  [@test] crates/file-diffs/src/accio_test.rs (`the_renames_of_a_subject_are_asked_in_one_process`)
  [@test] crates/test-server/src/commit_files_test.rs (`the_diffs_of_a_subject_say_what_its_files_say_of_each_other_and_the_index_is_asked_after_them`)

#### Scenario: An answer the query cut short
- **WHEN** the index answers that the clock of its call cut the pair (`cut: time`, not `final`), before the pair as accio does (one declaration, not complete, the uses of neither tree) or in its scan (the uses of both trees, not complete), or answers without `final`, as an accio before it; and the subject is asked again
- **THEN** the names are `incomplete` both times and the index is asked both times: such an answer says what its query got to and is not kept
- **AND** an index that answers to the end then confirms the rename, and that answer is kept
- **AND** a final answer is all the index says of the pair, complete or not: a pair that could not be asked, two places that are not one declaration, a use tied by the name alone, a side cut by the work; kept, and asked again it starts no process
  [@test] crates/file-diffs/src/service_test.rs (`an_index_is_asked_only_of_a_declaration_and_one_that_does_not_answer_fails_no_diff`)

#### Scenario: Only the pairs whose answer was not final are asked again
- **WHEN** a call the clock cut answers a declaration of a subject not final and a pair it could not ask final, and the subject is asked again, then once more
- **THEN** the second question asks the first declaration alone and keeps the answer of the other; once both are final, the third starts no process
  [@test] crates/file-diffs/src/service_test.rs (`a_question_asked_again_asks_only_the_pairs_whose_answer_was_not_final`)

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
or on a changed line in no run. A line a move took as it is MUST count as neither. The runs whose every place is
proven MUST be named (`subject.confirmed`).

#### Scenario: Every use is a place of the rename
- **WHEN** the index lists the declaration and the two calls of the report as the uses of the function in both trees, complete and without a conflict
- **THEN** the name of the report and the declaration of the store are `confirmed`, the two runs of the report are named as confirmed and are quiet, the report has no start left, and the block of the declaration keeps its start
- **AND** while the index is asked, and without an answer, the same runs are labels and every block is a stop
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_rename_every_use_of_which_the_index_lists_is_confirmed_and_its_declaration_stays_a_stop`)

#### Scenario: A use on a line that is read anyway
- **WHEN** the change also deletes a call of the function in a third file and adds one there, each on lines of its own, and a doc comment outside the subject names the function
- **THEN** the rename is confirmed all the same; the same use on a line the change left alone is a use outside it
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_use_on_a_line_that_is_read_anyway_and_a_link_of_a_doc_comment_do_not_stand_in_the_way`)

#### Scenario: A use on a line a move took as it is
- **WHEN** a function that calls the renamed function only moved, within its file or to another file, and a new function of the old name answers its call now, as accio answers of such a change: the call listed in the old tree, none in the new one, complete and without a conflict
- **THEN** the rename is `usesOutside`: a view shows the line as the same line at another place, and the old name stands on it
- **AND** the new name on a line that only moved is `otherMeaning`; a place of the rename whose old line a move took as it is, and that stands again at the other end of the move, is no place; a line a move edited in its whitespace alone is the line it was
- **AND** a call renamed on a line of a move is read where it stands now, a changed line, and the rename is confirmed
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_use_on_a_line_a_move_took_as_it_is_is_not_read_anyway`)

#### Scenario: A use on a line of a pale block
- **WHEN** a third file joins a `return` and the call of the old name on the line after it, read under the policy that counts no whitespace and under the one that counts every character
- **THEN** under the first the block is pale, the lines it was, and the rename is `usesOutside`; under the second the two lines are a change no run takes, and the rename is confirmed
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_use_on_a_line_of_a_pale_block_is_not_read_anyway`)

#### Scenario: A use without a word, a use by another word
- **WHEN** the index lists a positional argument that sets a renamed property, on a line the change left alone, as it answers of such a change; and a call by the alias of an import in both trees, on a line the change left alone, in a file of the subject one line further down or in a file outside it
- **THEN** the rename is confirmed all the same: neither writes the name
- **AND** a use by another word that only one tree lists, or that the other tree lists on another line or by another word, leaves the rename `incomplete`, never a verdict of what it changed
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_use_that_writes_no_word_or_the_same_other_word_in_both_trees_does_not_stand_in_the_way`)

#### Scenario: The accio of this machine
- **WHEN** this machine has an accio that answers `renames`, and a repository renames a function with its two calls in another file
- **THEN** the diffs of the commit, with an index of the repository built for the test, confirm the two calls and the declaration
- **AND** the answer is final: the commit read again through the same diffs starts no accio
  [@test] crates/file-diffs/src/accio_test.rs (`the_accio_of_this_machine_proves_the_rename_of_a_real_repository`)

### Requirement: Subject: An answer that is no proof leaves the rename a label and names why

Every other answer MUST leave the rename a label and name why: a use outside the change (`usesOutside`), a name that
means something else (`otherMeaning`), two places that are not one declaration, by the index or by the code that
declares the name (`otherDeclaration`), an answer that does not know every use or names a line the diff has not
(`incomplete`), no answer (`unavailable`), `asked` until then. No answer MUST fail the diffs or read as "no use";
only a final answer of a pair is kept.

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
