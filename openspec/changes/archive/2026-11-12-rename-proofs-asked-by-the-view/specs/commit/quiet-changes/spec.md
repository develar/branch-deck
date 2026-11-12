## MODIFIED Requirements

### Requirement: Subject: A pair leaves the changed lines no mark took and the names it declares none of

Beside its diff a pair of blobs MUST keep what it leaves to the other files of a subject: the changed lines of each
side that are in no run and in no move, as hashed keys in rows that can each hold a move between files, the names it
declares none of with the runs of the kind `renamed` their blocks have were they renames, each with every name it
leans on, the places of its declarations as the index takes them, and whether a side ends a line with a lone
carriage return. The pass over a subject MUST read no blob for it.

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
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_pair_keeps_the_runs_a_name_it_declares_none_of_would_have`)

#### Scenario: A run that leans on the name after its block
- **WHEN** a line that changed in a name another file declares ends its block, and the first token of the code after the block is a place of another name the file declares none of
- **THEN** the kept run names both: it is a run only while the code after the block reads the same, which the second name read as one makes it
  [@test] crates/file-diffs/src/marks_subject_test.rs (`the_subject_adds_labels_alone_never_a_quiet_run_or_a_stop`)

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
move between the two files: exact lines only, by their text without the spaces and tabs at their ends, the blank
lines inside it with it, from two lines that are not blank and 48 characters that are no whitespace. Where the
indentation of a file has a meaning, a line MUST pair only with a line at its depth: its indent is part of it.
Lines that left a file and stand again in it are the moves of its pair.

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

### Requirement: Subject: A name another file of the subject declares is a rename where it is used

A name that changed at every place of a file that declares none of them MUST be a rename of that file once another
file of the subject declares a name that is not local and renamed it so (`subject.used`, with the file and the line
of the declaration). The lines that changed in such a name alone MUST be runs of the kind `renamed`, labels, beside
the runs of the pair, which stay; a line that leans on a name no file declares is in none. The pass MUST add labels
alone: the quiet runs and the starts of a file are those of its pair, which went out with its rows.

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

The declarations of the renamed names of a subject that are not local MUST be asked of the index of the repository
in one process (`accio renames --no-build` between the two trees of the subject, the pairs of their places on its
standard input), after the diffs of the subject and never by their read, by the window that shows it
(`get_rename_proofs`), and only when the subject has one in a file of Kotlin or Java. Only an accio that says it
answers them as they are read is started, asked again what it answers once its executable is another; the working
tree MUST NOT be asked: its answer could be of another text than the one shown. An answer MUST be kept by its two
trees and its questions, and the query MUST be killed once the window asks of another subject, closes or loads
again.

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

#### Scenario: The window asks of another subject, closes or loads again
- **WHEN** the window that asked asks of another subject, loads again or closes while the query runs; and when another window does any of it
- **THEN** the query is killed and the question answers nothing, the names still `asked`; a page that is gone asks nothing; what another window does ends nothing
  [@test] crates/app-api/src/reads/diffs_test.rs (`the_question_of_a_window_to_the_index_ends_once_the_window_asks_another_closes_or_loads_again`)
  [@test] crates/app-api/src/session/windows_test.rs (`a_window_shows_the_page_of_a_call_until_it_closes_or_loads_again`)
  [@test] crates/file-diffs/src/accio_test.rs (`renames_without_an_index_answer_no_index_and_a_canceled_query_is_killed`)
  [@test] crates/file-diffs/src/service_test.rs (`an_index_is_asked_only_of_a_declaration_and_one_that_does_not_answer_fails_no_diff`)

### Requirement: Subject: A rename is proven by a complete answer whose every use the subject explains

A declaration MUST be proven renamed (`confirmed`) only when the index answers that its two places are one
declaration of Kotlin or Java, complete and with no name that means something else, and every use it lists in either
tree is the declaration, a place of the rename with its twin of the other tree listed too, a link of a doc comment,
a use that writes no word, a use by another word that the other tree lists at the same place of a line the change
left alone, or on a changed line in no run. The runs whose every place is proven MUST be named (`subject.confirmed`); the
declaration is in no run.

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

Every other answer MUST leave the rename a label and name why, for a view to word: a use outside the change
(`usesOutside`), a name that means something else (`otherMeaning`), two places that are not one declaration
(`otherDeclaration`), an answer that does not know every use or lists one on a line the diff cannot name
(`incomplete`), no answer (`unavailable`), and `asked` until the index answered. No answer MUST fail the diffs or
read as "no use", and none but a whole answer is kept.

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
