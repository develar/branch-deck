## ADDED Requirements

### Requirement: Subject: A pair leaves the changed lines no mark took and the names it declares none of

Beside its diff a pair of blobs MUST keep what it leaves to the other files of a subject: the changed lines of each
side that are in no run and in no move, as hashed keys in rows that can each hold a move between files, the names it
declares none of with the runs their blocks have were they renames, and the places of its declarations as the index
takes them. The pass over a subject MUST read no blob for it.

#### Scenario: The lines no mark took
- **WHEN** a file loses a function, a file is added, and a line of a deleted function is in a run or the function moved within its file
- **THEN** the pair leaves the lines of the function as one row of the old side, with the key of each line (its text without the spaces and tabs at its ends, hashed) and its characters that are no whitespace, and an added file every line but the empty one after its last line end
- **AND** a line of a run and a line of a move within the file are left by no pair
  [@test] crates/text-diff/src/moves_across_test.rs (`a_pair_leaves_the_lines_of_its_changes_that_no_mark_took`)

#### Scenario: A row too small for a move between files
- **WHEN** a pair changes one long line, three closing braces, blank lines alone, or two short lines
- **THEN** it leaves no row: a row is kept from two lines that are not blank and 48 characters that are no whitespace
  [@test] crates/text-diff/src/moves_across_test.rs (`a_row_that_cannot_hold_a_move_between_files_is_not_kept`)

#### Scenario: The runs of a name the pair declares none of
- **WHEN** the calls of a function another file declares get another name, one of them beside a call that is wrapped
- **THEN** the pair keeps the runs of each such block read with the two names as one, each with the names it needs: the line of the name a run of the kind `renamed`, the wrapped call the run of formatting it is
- **AND** the runs of the pair itself are as they were, and a kept run that shares a line with a move of the file is none
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
  [@test] crates/file-diffs/src/service_test.rs (`a_diff_stored_with_the_tokens_of_other_parsers_is_not_read`)

### Requirement: Subject: Lines one file deleted and another added are a move between files

Rows of the same lines among what the pairs of a subject left, deleted in one file and added in another, MUST be a
move between the two files: exact lines only, by their text without the spaces and tabs at their ends, the blank
lines inside it with it, from two lines that are not blank and 48 characters that are no whitespace. Lines that left
a file and stand again in it are the moves of its pair.

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
file of the subject declares a name that is not local and renamed it so (`subject.used`, with the file and the line
of the declaration). The lines that changed in such a name alone MUST be runs of the kind `renamed`, labels, beside
the runs of the pair, which stay; a line that needs a name no file declares is in none.

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

### Requirement: Subject: The index is asked once of every declaration a subject renamed

The declarations of the renamed names of a subject that are not local MUST be asked of the index of the repository
in one process (`accio renames --no-build` between the two trees of the subject, the pairs of their places on its
standard input), and only when the subject has one. Only an accio that says it answers them as they are read is
started, and the working tree MUST NOT be asked: its answer could be of another text than the one shown.

#### Scenario: One process for a subject
- **WHEN** the diffs of a commit, of the same commit again, of a range and of the working tree are read with an index
- **THEN** the index is asked once per read of the commit and of the range, every declaration in one call, between the parent and the commit or the two commits of the range, and never of the working tree
  [@test] crates/file-diffs/src/service_test.rs (`the_index_is_asked_once_of_the_names_a_subject_renamed_and_its_answer_comes_last`)
  [@test] crates/file-diffs/src/accio_test.rs (`the_renames_of_a_subject_are_asked_in_one_process`)

#### Scenario: No declaration, no question
- **WHEN** a subject renames no name that is not local
- **THEN** the index is asked nothing
  [@test] crates/file-diffs/src/service_test.rs (`an_index_is_asked_only_of_a_declaration_and_one_that_does_not_answer_fails_no_diff`)

#### Scenario: An accio without the answer
- **WHEN** the `versions` of accio names no answer of `renames`, another revision of it, or fails
- **THEN** its `renames` is never started, and the `versions` of one accio is asked once
  [@test] crates/file-diffs/src/accio_test.rs (`an_accio_that_answers_no_renames_as_they_are_read_is_not_asked`)

#### Scenario: The page went away
- **WHEN** the query is canceled, or the window of the page that asked closes or loads again
- **THEN** the query is killed and the stream ends with what it sent
  [@test] crates/file-diffs/src/accio_test.rs (`renames_without_an_index_answer_no_index_and_a_canceled_query_is_killed`)
  [@test] crates/file-diffs/src/service_test.rs (`an_index_is_asked_only_of_a_declaration_and_one_that_does_not_answer_fails_no_diff`)
  [@test] crates/app-api/src/session/windows_test.rs (`a_window_shows_the_page_of_a_call_until_it_closes_or_loads_again`)

### Requirement: Subject: A rename is proven by a complete answer whose every use the subject explains

A declaration MUST be proven renamed (`confirmed`) only when the index answers that its two places are one
declaration of Kotlin or Java, complete and with no name that means something else, and every use it lists in either
tree is the declaration, a place of the rename with its twin of the other tree listed too, a link of a doc comment,
or on a changed line in no run. The runs whose every place is proven MUST be named (`subject.confirmed`); the
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

#### Scenario: The accio of this machine
- **WHEN** this machine has an accio that answers `renames`, and a repository renames a function with its two calls in another file
- **THEN** the diffs of the commit, with an index of the repository built for the test, confirm the two calls and the declaration
  [@test] crates/file-diffs/src/accio_test.rs (`the_accio_of_this_machine_proves_the_rename_of_a_real_repository`)

### Requirement: Subject: An answer that is no proof leaves the rename a label and names why

Every other answer MUST leave the rename a label and name why, for a view to word: a use outside the change
(`usesOutside`), a name that means something else (`otherMeaning`), two places that are not one declaration
(`otherDeclaration`), an answer that does not know every use (`incomplete`), no answer (`unavailable`), and `asked`
while the index runs. No answer MUST fail the diffs or read as "no use".

#### Scenario: Each reason
- **WHEN** the answer has a use the change left behind, a use in a file outside the subject or on a line it left alone; a conflict of a capture, an overload, an override, a hidden or a namesake declaration, a use without its twin, a use of the new name on a line left alone; another signature; an answer that is not complete, a use tied by its name alone, more uses than listed, no uses of a tree, another language; a pair that could not be asked
- **THEN** the proof is `usesOutside`, `otherMeaning`, `otherDeclaration`, `incomplete` and `unavailable` in that order of the cases, and no run is confirmed
- **AND** a place of the name the index lists for no declaration leaves the name of its file `incomplete`, while the declaration and the use it does list are proven
- **AND** what the index found wrong comes before what it could not say
  [@test] crates/file-diffs/src/marks_subject_test.rs (`every_other_answer_leaves_the_rename_a_label_and_says_why`)

#### Scenario: An index that does not answer
- **WHEN** the index answers no renames, has no index of the repository, fails, or answers another number of pairs than asked
- **THEN** the diffs are read whole and the names are `unavailable`
  [@test] crates/file-diffs/src/service_test.rs (`an_index_is_asked_only_of_a_declaration_and_one_that_does_not_answer_fails_no_diff`)
  [@test] crates/file-diffs/src/accio_test.rs (`renames_without_an_index_answer_no_index_and_a_canceled_query_is_killed`)

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
