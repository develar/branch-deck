## ADDED Requirements

### Requirement: Subject: A declaration the index says moved whole is a move between files, in another layout too

A declaration that the index answers moved whole to another file or under other owners with the same words (`accio
moves`) MUST be a move between files of its lines at both ends (`declaration`: its name, the lines of the other end),
where each end is the changed lines of one block that no run or move of its pair took. One that a row of the same
lines holds as it is MUST stay that row; of a row it shares lines with, only the parts that are a row by themselves
stay. A label.

#### Scenario: A function moved to another file in another layout
- **WHEN** a ledger loses a function that a report gets with its parameters one a line, and the index says the function moved whole from its four old lines to its seven new ones
- **THEN** without the answer the last three lines alone are a move; with it the ledger names the four old lines and the report the seven new ones as one move of `reportTitle`, each with the lines of the other end, and the runs, the moves, the renames and the starts of both are those of their pairs
- **AND** an end that holds a line the change left alone, starts before its first line, or names a file the subject does not change moves nothing: the rows of the same lines are what they were
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_declaration_the_index_says_moved_whole_is_a_move_between_files_in_another_layout_too`)

#### Scenario: A file of declarations moved as they are
- **WHEN** a ledger loses two functions that a report gets as they are, one row of the same lines, and the index names each of them as moved whole
- **THEN** the row stays the one move it was, and no end names a declaration
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_row_of_the_same_lines_that_holds_declarations_the_index_says_moved_whole_as_they_are_stays_one_move`)

#### Scenario: A doc comment beside it
- **WHEN** a function moves with its doc comment, its last line wrapped otherwise, and the index counts the function from its head
- **THEN** without the answer the doc, the head and the line after it are one row; with it the function is the move of the declaration, and a doc of two lines and enough characters a move of its own
- **AND** a doc too short to be a row by itself is no move: only the declaration moved
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_row_of_the_same_lines_keeps_its_parts_beside_a_declaration_of_another_layout_that_are_a_row_by_themselves`)

#### Scenario: Under another owner within its file
- **WHEN** a function leaves its class for the top of its file, on two lines, and the index says it moved under other owners
- **THEN** the file names both ends, each with the file itself as the other file
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_declaration_moved_under_another_owner_within_its_file_is_a_move_to_the_file_itself`)

#### Scenario: The accio of this machine
- **WHEN** this machine has an accio that answers `moves` of the revision 2, and a commit moves a function to another file with its parameters one a line
- **THEN** the diffs of the commit, with an index of the repository built for the test, name the four old lines and the seven new ones as one move of `reportTitle`
- **AND** the subject read again through the same diffs starts no accio
  [@test] crates/file-diffs/src/accio_test.rs (`the_accio_of_this_machine_says_a_declaration_moved_whole_in_another_layout`)

### Requirement: Subject: The index is asked once of the moves of a subject

The declarations a subject of two trees moved whole MUST be asked of the index in one process (`accio moves
--no-build`), after its diffs, in the question of the window (`get_index_marks`), only where a file deletes a line and
one adds one, and only once that question told its renames: it answers in stages, the renames, the moves, the uses
that follow a move. Only an accio whose `moves` is of the revision 2 is started. The answer MUST be kept by the two
trees.

#### Scenario: One process, kept by the trees
- **WHEN** the diffs of a commit that moved a function are read with an index there, without one, and of the working tree; then the index is asked of the commit twice, and of a commit that only adds a file
- **THEN** only the first read says the index is to be asked; the first question asks the moves between the parent and the commit and the ledger and the report get the move of the declaration; the second starts no process, and the commit that adds a file asks nothing
  [@test] crates/file-diffs/src/service_test.rs (`the_moves_of_a_subject_are_asked_after_its_diffs_once_by_its_trees`)
  [@test] src/stores/file-diffs.test.ts (`ends the read of a subject with its diffs, and asks the index of its renamed names once a view shows it`)

#### Scenario: The renames before the moves
- **WHEN** a commit moves a function from one object to another and a file writes the new owner in front of its call, and the index is asked of the commit twice
- **THEN** the stages come in their order: the renames, told before the moves are asked; the moves, which say nothing of the file of the call; the uses that follow the move, which prove it and give that file its run, quiet
- **AND** the second question tells the same stages from memory and starts no process
  [@test] crates/file-diffs/src/service_test.rs (`the_stages_of_the_answer_of_the_index_tell_the_renames_before_the_moves_are_asked`)
  [@test] src/stores/file-diffs.test.ts (`takes each stage of the answer of the index as it comes, the proofs of the renames before the moves`)

### Requirement: Subject: The owner in front of the uses of a declaration moved under another owner follows the move

A name that changed at every place of a file that declares neither, from the old owner of a declaration the index
says moved whole under another owner with its name to the new one, may follow the move: such declarations MUST be
asked of the index after the moves, in one process (`accio renames`). The name MUST be a mark (`UsedRename::moved`,
its runs quiet) only where the declaration moved with its words and the move explains every use the index lists and
every place of the name.

#### Scenario: A call whose owner the change wrote anew
- **WHEN** a function moves from the object `Ledger` to the object `Report` (`owner`, the `parent` of both places), a file changes `Ledger.title(account)` to `Report.title(account)`, and the index says the function moved
- **THEN** without the moves of the index, and with the moves alone, the file says nothing; the move is asked by the line of the name of each declaration (`Subject::move_questions`)
- **AND** an answer that says the function moved with its words and its name (`moved`, not `renamed`, `body_equal`, only `owner` in `differs`), complete, and lists the declaration and the call behind each owner (at most two characters between them) in both trees proves it: the file names `Ledger` → `Report` as following the move of `title` to the report, confirmed, one run of the kind of a rename, quiet, and its block passed
- **AND** a use on a line read anyway, or on a line the change left alone with its twin at the same place, proves as much
- **AND** another declaration than the one that moved, a use left in a file the change did not touch, an answer that is not complete, and a use the other tree does not list behind the new owner prove nothing, and the file says nothing then; nor does an owner the file names where no use of the declaration stands behind it
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_use_whose_owner_the_change_wrote_anew_after_a_move_under_another_owner_is_quiet_once_the_index_proves_it`)

#### Scenario: An accio without the answer
- **WHEN** the `versions` of accio names no answer of `moves`, its revision 1 or 3, or is no JSON; and when it answers 2, and when its index is not usable
- **THEN** its `moves` is never started; of 2 it is asked `moves --no-build --format json --project <root> --base <base> --ref <head> --limit 500` and its answer read; without a usable index the answer is no index
  [@test] crates/file-diffs/src/accio_test.rs (`an_accio_that_answers_no_moves_as_they_are_read_is_not_asked`)

### Requirement: Views: The note of a declaration that moved whole names it at both ends

A note at an end of a move of a declaration (`declaration`) MUST say its name before the place of the other end
("Moved summary to Report.kt:12", "Moved summary from Ledger.kt:7"), its button naming the declaration, and the lines
of the other end MUST be those of the declaration there, its layout another too. A note of the owner that follows a
move MUST say so and where the declaration went ("Follows a move", "title moved to Report.kt").

#### Scenario: The function the index says moved to the report in another layout
- **WHEN** the index of the `declaration_moves` repository says `summary` moved whole from its four lines of the ledger to its seven lines of the report, its parameters one a line, and `reportTitle` as it is; the preview shows both files
- **THEN** before the answer the title is the one move of the ledger, "Moved to Report.kt:7", and nothing says the summary moved
- **AND** once the index answered the ledger shows "Moved summary to Report.kt:12" beside the title, the same note as before; its button shows the report at the note "Moved summary from Ledger.kt:7" before the seven lines of the function, every one a line of the move, and the button there shows the ledger again
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`a declaration of another layout is a move once the index says it moved: a click on the place its note names shows its seven lines, whose note shows the four old ones again; the row of a declaration moved as it is stays the note it was`)

#### Scenario: The words and the ends
- **WHEN** an end of the move of a declaration of seven new lines is worded and made into the parts of its block
- **THEN** it says "Moved reportTitle", "to Report.kt:18" or "from Ledger.kt:55" and names the declaration in its button; its other end is the seven lines, and an end of a row of the same lines names no declaration
  [@test] src/presenters/diff-marks.test.ts (`names a declaration the index says moved whole at both ends, by its name, before the file and the line of the other end`)

#### Scenario: The owner that follows a move
- **WHEN** a note words `Ledger` → `Report` of a file that follows the move of `title` to the report, proven, with the report shown and not shown, and beside a rename
- **THEN** it says "Follows a move", "title moved to Report.kt" with a button "Go to title in src/Report.kt", plain words "title stands in src/Report.kt now" where the view does not show the report, and "all uses in this change"; beside a rename the note is of renames
  [@test] src/presenters/diff-marks.test.ts (`says of the owner of a declaration that moved under another owner that the change follows the move, and where the declaration went`)
  [@test] src/lib/diff/block-parts.test.ts (`gives the block of the lines an end of the move, which names the file and the lines of the other end; none for lines no block holds whole`)

