## MODIFIED Requirements

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

#### Scenario: Reordered in another layout
- **WHEN** a method of three lines moves below another of its class on one line, and the index says it was put in another order there
- **THEN** the file names both ends, the three old lines and the one new line, each with the file itself as the other file: a declaration the index reordered is such a move where no move of its pair holds it
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_move_within_a_file_of_declarations_whose_order_is_free_is_a_reorder_of_them`)

#### Scenario: The accio of this machine
- **WHEN** this machine has an accio that answers `moves` of the revision 2, and a commit moves a function to another file with its parameters one a line
- **THEN** the diffs of the commit, with an index of the repository built for the test, name the four old lines and the seven new ones as one move of `reportTitle`
- **AND** the subject read again through the same diffs starts no accio
  [@test] crates/file-diffs/src/accio_test.rs (`the_accio_of_this_machine_says_a_declaration_moved_whole_in_another_layout`)

## ADDED Requirements

### Requirement: Subject: A move of declarations whose order is free says so

A move within a file, of its pair or of a declaration to the file itself, MUST name the declarations it holds
(`reordered`, by its two ends) where every declaration the index reordered whose lines meet an end stands within both
ends with an order that is free (`order: "free"`), and one does; else it MUST name none. A label: the lines are read
where they stand now, and no run, move or stop changes. `brd commits --mark-stats --index` counts them
(`moves.free_order`).

#### Scenario: A function moved below another in its class
- **WHEN** a method moves below another of its class as it is, a move of the pair, and the index says both were reordered with an order that is free
- **THEN** the move names the method by its two ends, and the moves of the pair are those they were
- **AND** where the index says its order matters, or reordered only the other method, the move names nothing; and with a property beside it whose order matters it names nothing either
  [@test] crates/file-diffs/src/marks_subject_test.rs (`a_move_within_a_file_of_declarations_whose_order_is_free_is_a_reorder_of_them`)
  [@test] crates/diff-marks/src/index_test.rs (`a_move_of_declarations_whose_order_is_free_is_a_mark_and_one_that_holds_an_order_that_matters_is_none`)

#### Scenario: The accio of this machine
- **WHEN** this machine has an accio that answers `moves` of the revision 2, and a commit moves a function below another in its class and an enum entry below the others
- **THEN** the diffs of the commit, with an index of the repository built for the test, name the move of the function and not that of the entry
  [@test] crates/file-diffs/src/accio_test.rs (`the_accio_of_this_machine_says_a_function_moved_below_another_in_its_class_is_a_reorder_of_free_order`)

#### Scenario: Through the server
- **WHEN** the `free_reorder` template is read through the server, then the index is asked of its commit
- **THEN** the diffs give two moves of the pair, the function and the enum entry; the stage of the moves names the move of the function alone
  [@test] crates/test-server/src/commit_files_test.rs (`a_move_of_a_declaration_whose_order_the_index_says_is_free_is_a_reorder_once_it_answered`)

### Requirement: Views: The note of a move of declarations whose order is free says so

A note at an end of a move that names declarations of free order (`reordered`) MUST say their names before the place
of the other end and that their order does not matter after it ("Moved total from line 4 · order does not matter",
"Moved total to line 8 · order does not matter", of a declaration of another layout "Moved total from Ledger.kt:4 ·
order does not matter"); its button and the key of the notes do what they do of any move.

#### Scenario: The words
- **WHEN** both ends of a move within the file that names `total`, one that names `total` and `count` and was indented again, one that names none, and a declaration of another layout moved within its file are worded
- **THEN** they say "Moved total" "to line 210" "order does not matter", "Moved total and count" "from line 120" "order does not matter · re-indented", "Moved", and "Moved total" "from Ledger.kt:4" "order does not matter"
  [@test] src/presenters/diff-marks.test.ts (`names the declarations of a move within the file the index says were put in another order where their order is free, and says it does not matter`)
  [@test] src/lib/diff/block-parts.test.ts (`names the declarations of free order of a move within the file at both its ends, by the ends the subject names, and of no other move`)

#### Scenario: The move of the template
- **WHEN** the preview of the `free_reorder` template shows Ledger.kt, whose `total` moved below `count` and whose `LOW` moved below the other entries, and the index answers
- **THEN** before the answer the notes say "5 lines moved to line 8" and "Moved from line 4"; after it "Moved total to line 8 · order does not matter" and "Moved total from line 4 · order does not matter", whose button shows the other end
- **AND** the notes of the entry, whose order matters, say "1 line moved to line 18" and "Moved from line 16" before and after
  [@test] tests/e2e/commit-files/subject-marks.spec.ts (`the notes of the total moved below the count in its class say its order does not matter once the index answers, and those of an enum entry, whose order matters, stay what they were`)
