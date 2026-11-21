## ADDED Requirements

### Requirement: Units: The place a move left is nothing to read where it holds the move alone

The slice of the place that declarations moved whole to other files left MUST be `moved_out` when it holds nothing but
them: the blocks of a unit `removed` change the old side as removing those declarations would, blank lines aside; a
deleted file holds them, its imports, its package line and its comments alone. The other places show what it removes.
Every other slice MUST NOT be `moved_out`: its lines are read where they stand.

#### Scenario: The places the moves left, each holding them alone
- **GIVEN** `formatTotal`, `count`, `parseTotal` and `parseSum` in `Orders.kt`, and `Legacy.kt` of a comment, its
  package, an import and `legacyTotal`
- **WHEN** the commits move `formatTotal` to `Report.kt`, `parseTotal` and `parseSum`, one block, into the new
  `Parsing.kt`, and `legacyTotal` to `Report.kt`, deleting `Legacy.kt`, each with the same code
- **THEN** the slices of `formatTotal`, of `parseTotal`, whose unit holds the block of both, and of `Legacy.kt` are
  `moved_out`
  [@test] crates/review-plan/src/moved_across_test.rs (`the_place_a_move_left_is_nothing_to_read_where_it_holds_the_move_alone`)
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_moved_from_and_moved_to_of_a_declaration_moved_whole_to_another_file`)

#### Scenario: A place that holds more than the move
- **WHEN** a block removes a constant that moved nowhere beside a function that moved, a deleted file holds a function
  that moved nowhere beside one that moved, or a block adds `legacyTotal` in the place of two functions that moved
- **THEN** no slice is `moved_out`, and the moves say where they went and came from all the same
  [@test] crates/review-plan/src/moved_across_test.rs (`the_place_a_move_left_is_nothing_to_read_where_it_holds_the_move_alone`, `a_declaration_moved_whole_to_another_file_says_where_it_went_and_came_from`)

#### Scenario: A function between two others without a blank line
- **WHEN** a function moves from between two others it touches, its closing brace the line before the next, or with the
  annotation the next one has too
- **THEN** its slice is `moved_out`, however the comparison takes the lines it shares with its neighbours: the old side
  without its blocks reads as the old side without the function
  [@test] crates/review-plan/src/moved_across_test.rs (`a_function_moved_from_between_two_others_without_a_blank_line_is_the_move_alone`)

### Requirement: In the Review Window: A declaration moved whole is read at its new place

The heading of a unit `added` with `moved_from` MUST say where it came from ("formatTotal — moved from Orders.kt"), of
a unit `removed` with `moved_to` where it went, the file by its name among the files of the review; a new file MUST
name above its lines what moved into it, a deleted file that is not the move alone where its declarations went. Its
lines are read where it stands now: nothing of them is folded or mechanical, and what is left to read counts as the
diffs count it.

#### Scenario: The headings of the moves of a branch
- **GIVEN** the review of the branch `moves` of the `review_moves` template, with the plan
- **WHEN** it shows its diffs
- **THEN** the heading of `formatTotal` in `Report.kt` says "formatTotal — moved from Orders.kt", and its lines show
- **AND** the new `Parsing.kt` says "parseTotal — moved from Orders.kt · parseSum — moved from Orders.kt", and
  `legacyTotal` in `Orders.kt` "legacyTotal — moved from Legacy.kt"
  [@test] tests/e2e/branch-review/review-moves.spec.ts (`should read a declaration moved to another file at its new place, its old place one row that opens it, in another step too, and name what moved into a new file and out of a deleted one`)
  [@test] src/presenters/review-units.test.ts (`words a declaration moved whole between two files where it stands now and where it left, as Rust decided`, `names in the row of the place two declarations left both and where each went, and a file taken whole by the moves into it, out of it, or its row`)

#### Scenario: A place that holds more than the move
- **WHEN** the block of the place a declaration left holds more than the move, or a deleted file more than its moves
- **THEN** its lines show under the heading "formatTotal — moved to Report.kt", "legacyTotal — moved to Orders.kt"
  [@test] src/presenters/review-units.test.ts (`makes the place a declaration left one row that opens where it went, where Rust says it holds the move alone, and keeps its lines under the heading where it holds more`, `names in the row of the place two declarations left both and where each went, and a file taken whole by the moves into it, out of it, or its row`)

#### Scenario: One place of a move in the range
- **WHEN** the review shows a range that holds one place of a move alone: a commit shown alone that copied the function,
  or Since Your Review of the commit that dropped its old copy
- **THEN** the plan of that range says no move: the function is new or removed there, and read
  [@test] crates/review-plan/src/moved_across_test.rs (`a_range_that_holds_one_place_of_a_move_says_nothing`)

### Requirement: In the Review Window: The words of a move are said once

The note of the marks of a move at the start of the block of a heading of that move MUST name the line of its other end
alone ("legacyTotal — moved from Legacy.kt · line 4"), its button and its name to a screen reader in full; the card of
a name of a unit with `moved_from` or `moved_to` MUST say "Moved from Orders.kt" or "Moved to Report.kt", and a step
that does it to a unit another step shows "Moves formatTotal, shown in step 2".

#### Scenario: The note of the move in the row of its heading
- **GIVEN** the review of the branch `moves`
- **WHEN** it shows `legacyTotal` in `Orders.kt`, whose block starts with the note of its move from `Legacy.kt`
- **THEN** the row says "legacyTotal — moved from Legacy.kt · line 4", the line a button to the other end
- **AND** a note of a move in a row of its own, or of another file, says its words as before ("Moved from Orders.kt:7")
  [@test] tests/e2e/branch-review/review-moves.spec.ts (`should read a declaration moved to another file at its new place, its old place one row that opens it, in another step too, and name what moved into a new file and out of a deleted one`)
  [@test] src/presenters/diff-marks.test.ts (`says of a move the heading of its row names already the line of the other end alone, and names it in full to a screen reader`)

#### Scenario: The card of a name and the facts of a step
- **WHEN** a mention of the story names a unit with `moved_from` or `moved_to`, and a step does it to a unit the stop of
  another step shows
- **THEN** the card says "Moved from Orders.kt" or "Moved to Report.kt", never "New" or "Removed", and the facts of the
  step "Moves format, shown in step 2"
  [@test] src/presenters/review-story.test.ts (`links a code name to its place, worded by its unit, its line and its step, and marks the section it names with the line that names it`, `words the overview: the headline, What Changes one header per file, the steps with the lead of each, their facts and a step no test proves`)

## MODIFIED Requirements

### Requirement: Units: A declaration moved whole to another file says where

A unit `removed` MUST say the file, the line and the unit of the place it went to (`moved_to`), and a unit `added`
those of the place it came from (`moved_from`), where exactly one declaration of one side alone before the commits and
one after them, in two files, have the same code, the same content of their tokens; the other place is a unit `added`
or `removed`, or an outermost declaration of a new or a deleted file, whose unit says nothing. Every other unit MUST
have neither.

#### Scenario: A function moved to a file that stays, into a new file and out of a deleted file
- **GIVEN** `formatTotal` and `parseTotal` in `Orders.kt`, and `legacyTotal` alone in `Legacy.kt`
- **WHEN** the commits move `formatTotal` to `Report.kt`, `parseTotal` into the new `Parsing.kt` and `legacyTotal`
  into `Orders.kt`, deleting `Legacy.kt`, each with the same code
- **THEN** the removed `formatTotal` says `moved_to` `Report.kt`, the line of its name there and the added unit there,
  the added one `moved_from` `Orders.kt`, the line of its name before the commits and the removed unit there
- **AND** the removed `parseTotal` says `moved_to` `Parsing.kt` with its unit `newFile`, and the added `legacyTotal`
  `moved_from` `Legacy.kt` with its unit `deletedFile`
- **AND** the units `newFile` of `Parsing.kt` and `deletedFile` of `Legacy.kt` say neither, and the units are those
  without a move
  [@test] crates/review-plan/src/moved_across_test.rs (`a_declaration_moved_whole_to_another_file_says_where_it_went_and_came_from`)
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_moved_from_and_moved_to_of_a_declaration_moved_whole_to_another_file`)

#### Scenario: A function moved into a class of another file, indented again
- **WHEN** a top-level function moves into a class of another file that was there, indented again
- **THEN** the removed one says `moved_to` and the added one `moved_from`: the content of their tokens is the same
  [@test] crates/review-plan/src/moved_across_test.rs (`a_function_moved_into_a_class_of_another_file_indented_again_is_a_move`)

#### Scenario: What is no move
- **WHEN** a function moves to another file with a changed line, one is added to two files and removed from one, one
  is removed from two files and added to one, or one moves to another class of its own file
- **THEN** no unit says `moved_from` or `moved_to`: a word on two files must not guess, and the diff of one file shows
  a move inside it
  [@test] crates/review-plan/src/moved_across_test.rs (`a_move_with_a_changed_line_or_of_two_places_on_a_side_says_nothing`)

#### Scenario: The places a move pairs
- **WHEN** the two places of one code are a unit `removed` and a unit `added` of two files that stay, a unit `removed`
  and an outermost declaration of a new file, or an outermost declaration of a deleted file and a unit `added`
- **THEN** the unit of the declaration says the other place, and a file taken whole (`newFile`, `deletedFile`, in
  `deletedFiles` too) says nothing
- **AND** a new and a deleted file alone, a member of a new or a removed type (part of its unit), a declaration of a
  file a kind takes whole (`mechanical`, `other`) and a pair of one declaration (`shape`, `behaviour`, `moved`) are no
  place of a move

### Requirement: In `brd`: `brd review-plan` prints the units

`brd review-plan` MUST print `units` (`"schema": 40`): per unit its `id`, `kind`, `name`, `key`, `signature`, `api`,
`test`, `line`, its `slices`, its `fingerprint` and, of a body changed in a rename the index has not proven, `renamed`,
of a declaration renamed so, `renamed_from`, of a declaration moved whole to another file, `moved_from` or `moved_to`;
of a slice that holds nothing but declarations moved whole to other files, `moved_out`.

#### Scenario: The units of a range
- **WHEN** `brd review-plan` runs on a range
- **THEN** it prints `units` (`"schema": 40`), per unit its `id`, `kind`, `name`, `key`, `signature`, `api`, `test`,
  `line`, its `slices` and its `fingerprint`
- **AND** `kind` is `shape` with `changes`, `behaviour`, `added`, `removed`, `newFile`, `deletedFile`, `deletedFiles`
  with `directory`, `mechanical` with `mechanical`, `repeatedEdit` with `group`, or `other` with `reason`
- **AND** each of its `slices` has `path`, `old_path`, `whole`, the `blocks` by `index` with their `old` and `new` lines
  from 1, both included, `null` for a side with none, and a `fingerprint`
- **AND** a `behaviour` unit whose body changed in renamed names alone that the index has not proven has `renamed`, its
  `names` (`old`, `new`) and its `wait` (`checking`, `notProven`); no other unit has it
- **AND** a declaration renamed where the index has not proven it is one unit with `renamed_from`, its `old` name and its
  `wait`
- **AND** an `added` declaration moved whole from another file has `moved_from`, a `removed` one moved whole to another
  file `moved_to`, each the `path` of the other place, the `line` of its name there, from 1, and the `unit` that shows
  it; no other unit has them
- **AND** a slice that holds nothing but declarations moved whole to other files has `moved_out: true`; no other slice
  has it
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`, `prints_the_renamed_of_a_body_changed_in_a_rename_alone_and_of_the_declaration`, `prints_the_moved_from_and_moved_to_of_a_declaration_moved_whole_to_another_file`)

### Requirement: In the Review Window: A block another section shows is one row

A block that another section shows MUST be one row in the section of its file, "Shown in Imports only": a group by its
name, on each of its files, never the count of its heading; no change for N and P. A block of a slice that holds the
move alone (`moved_out`) MUST be such a row that says what moved and where ("Moved formatTotal to Report.kt"), another
step by its number; in a fold the words of the row.

#### Scenario: A block of the imports in its file's section
- **WHEN** the section of a file holds a block that the group of the imports shows
- **THEN** the block is one row there, "Shown in Imports only", without the count of its heading "Imports only (1)"
- **AND** N and P pass it, no change of that section
  [@test] src/lib/diff/rows.test.ts (`makes the rows of a section of a review: a heading before each change it marks, one row for a block another section shows`)
  [@test] src/presenters/review-units.test.ts (`names a block a group shows by the name of the group on each of its files, never by the count of its heading`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a block of the imports alone in the group of the imports, in its file's section one row that opens it, and pass it with N`)

#### Scenario: A block of the place a declaration left
- **WHEN** the section of `Orders.kt` holds the block `parseTotal` and `parseSum` left for the new `Parsing.kt`, the one
  `formatTotal` left for `Report.kt` in another step, and the section of the deleted `Legacy.kt` the block `legacyTotal`
  left for `Orders.kt`
- **THEN** each is one row, "Moved parseTotal and parseSum to Parsing.kt", "Moved formatTotal to Report.kt in step 1",
  "Moved legacyTotal to Orders.kt", none of their lines shown
- **AND** N and P pass it, no change of that section; a fold that hides it says its words
  [@test] tests/e2e/branch-review/review-moves.spec.ts (`should read a declaration moved to another file at its new place, its old place one row that opens it, in another step too, and name what moved into a new file and out of a deleted one`)
  [@test] src/presenters/review-units.test.ts (`makes the place a declaration left one row that opens where it went, where Rust says it holds the move alone, and keeps its lines under the heading where it holds more`, `names the step of where a declaration went in the row of the place it left, when it is another, and in a fold the row itself`)

### Requirement: In the Review Window: A click on the row opens the other section

A click on the row of a block another section shows, or on its place in a fold, MUST open that section, expanded, as
the section at the top: the tree and the footer follow it. The row of the place a declaration left opens the section of
where it went, of another step and another file too.

#### Scenario: The row of the imports and the row of a repeated edit
- **WHEN** the user clicks the row of a block another section shows
- **THEN** that section opens, expanded
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should show a block of the imports alone in the group of the imports, in its file's section one row that opens it, and pass it with N`, `should show a repeated edit once under its group, the other files of the edit collapsed, and a file's own change in its section with a row to the edit`)

#### Scenario: The row of the place a declaration left
- **WHEN** the user clicks the row "Moved parseTotal and parseSum to Parsing.kt", "Moved formatTotal to Report.kt in
  step 1" or "Moved legacyTotal to Orders.kt"
- **THEN** the section of `Parsing.kt`, of `Report.kt` in step 1, of `Orders.kt` in step 2 opens, the tree and the
  footer following it
  [@test] tests/e2e/branch-review/review-moves.spec.ts (`should read a declaration moved to another file at its new place, its old place one row that opens it, in another step too, and name what moved into a new file and out of a deleted one`)

#### Scenario: The place of a block in a fold
- **WHEN** the user clicks the place of a block in a fold ("1 change in step 1")
- **THEN** that section opens, expanded, and the footer names it
  [@test] tests/e2e/branch-review/review-parts.spec.ts (`should show a file in parts as a card each, its part and its own lines, the footer the part at the top as J walks them, its row the first part, and a fold the changes other parts show, which it opens`)
