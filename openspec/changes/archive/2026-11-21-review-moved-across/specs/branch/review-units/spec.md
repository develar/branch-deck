## ADDED Requirements

### Requirement: Units: A declaration moved whole to another file says where

A unit `removed` MUST say the file and the line it went to (`moved_to`), and a unit `added` those it came from
(`moved_from`), where exactly one declaration of one side alone before the commits and one after them, in two files,
have the same code, the same content of their tokens; the other place is a unit `added` or `removed`, or an outermost
declaration of a new or a deleted file, whose unit says nothing. Every other unit MUST have neither.

#### Scenario: A function moved to a file that stays, into a new file and out of a deleted file
- **GIVEN** `formatTotal` and `parseTotal` in `Orders.kt`, and `legacyTotal` alone in `Legacy.kt`
- **WHEN** the commits move `formatTotal` to `Report.kt`, `parseTotal` into the new `Parsing.kt` and `legacyTotal`
  into `Orders.kt`, deleting `Legacy.kt`, each with the same code
- **THEN** the removed `formatTotal` says `moved_to` `Report.kt` and the line of its name there, the added one
  `moved_from` `Orders.kt` and the line of its name before the commits
- **AND** the removed `parseTotal` says `moved_to` `Parsing.kt`, and the added `legacyTotal` `moved_from` `Legacy.kt`
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

## MODIFIED Requirements

### Requirement: In `brd`: `brd review-plan` prints the units

`brd review-plan` MUST print `units` (`"schema": 39`): per unit its `id`, `kind`, `name`, `key`, `signature`, `api`,
`test`, `line`, its `slices`, its `fingerprint` and, of a body changed in a rename the index has not proven, `renamed`,
of a declaration renamed so, `renamed_from`, of a declaration moved whole to another file, `moved_from` or `moved_to`.

#### Scenario: The units of a range
- **WHEN** `brd review-plan` runs on a range
- **THEN** it prints `units` (`"schema": 39`), per unit its `id`, `kind`, `name`, `key`, `signature`, `api`, `test`,
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
  file `moved_to`, each the `path` of the other place and the `line` of its name there, from 1; no other unit has them
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`, `prints_the_renamed_of_a_body_changed_in_a_rename_alone_and_of_the_declaration`, `prints_the_moved_from_and_moved_to_of_a_declaration_moved_whole_to_another_file`)
