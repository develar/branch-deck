## MODIFIED Requirements

### Requirement: Units: The place a move left is nothing to read where it holds the move alone

The slice of the place that declarations moved whole to other files left MUST be `moved_out` when it holds nothing but
them and what goes with them: its blocks change the old side as removing them would, blank lines aside, and add only
the skeleton of the move (`mod x;` of a file of the change, `pub use x::*;`, an import); a deleted file holds them,
its imports, its package line and its comments alone. Every other slice MUST NOT be `moved_out`: its lines are read
where they stand.

#### Scenario: The places the moves left, each holding them alone
- **GIVEN** `formatTotal`, `count`, `parseTotal` and `parseSum` in `Orders.kt`, and `Legacy.kt` of a comment, its
  package, an import and `legacyTotal`
- **WHEN** the commits move `formatTotal` to `Report.kt`, `parseTotal` and `parseSum`, one block, into the new
  `Parsing.kt`, and `legacyTotal` to `Report.kt`, deleting `Legacy.kt`, each with the same code
- **THEN** the slices of `formatTotal`, of `parseTotal`, whose unit holds the block of both, and of `Legacy.kt` are
  `moved_out`
  [@test] crates/review-plan/src/moved_across_test.rs (`the_place_a_move_left_is_nothing_to_read_where_it_holds_the_move_alone`)
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_moved_from_and_moved_to_of_a_declaration_moved_whole_to_another_file`)

#### Scenario: A type with its impl, and the skeleton of the move
- **WHEN** `struct Shape` and its `impl Shape` move into the new `shape.rs`, and the block of `fn parse` puts
  `mod parsing; pub use parsing::*;` in its place, `parse` in the new `parsing.rs`
- **THEN** the slice of `Shape` and the slice of the module `parsing`, which holds that block, are `moved_out`: the
  methods of a removed type and the lines of an `impl` whose members all go, its header and its closing line, go with
  them
  [@test] crates/review-plan/src/moved_across_test.rs (`a_type_moved_with_its_impl_and_code_replaced_by_its_module_are_the_move_alone`)

#### Scenario: The places a module of tests left
- **WHEN** a test of `mod tests` moves to a new file, a deleted file of `mod tests` and its test moves it into `lib.rs`,
  or the inline `mod tests` becomes `mod tests;`, its tests in `tests.rs`
- **THEN** the slice of the test, the deleted file and the slice of the unit `behaviour` of the module are `moved_out`: a
  module whose members all go, of no code of its own but its header, its closing line and imports, goes with them
  [@test] crates/review-plan/src/moved_across_test.rs (`a_member_of_a_module_moves_between_files_and_its_module_goes_with_it`)

#### Scenario: A call of a macro beside the moved code
- **WHEN** `answer!(Parse);` stands beside `fn parse` in `lib.rs`, and both move into the new `parsing.rs`
- **THEN** the slice of `parse` is `moved_out`, and is read where the call changed on the way: a line of no declaration
  that is an item of its own goes where a file the commits add holds the same code as often
  [@test] crates/review-plan/src/moved_across_test.rs (`a_line_of_no_declaration_moved_into_a_new_file_goes_with_the_move`)

#### Scenario: A `pub use` beside the moved code
- **WHEN** `pub use crate::query::Enclosing;` stands beside `fn parse` in `lib.rs`, and both move into the new
  `parsing.rs`, or the commits remove it and no file holds it again
- **THEN** the slice of `parse` is `moved_out` where `parsing.rs` holds the `pub use`, and is read where it is gone: a
  `pub use` the blocks remove goes with the move only where a file the commits add holds it, as the names a file passes
  on may be API
  [@test] crates/review-plan/src/moved_across_test.rs (`a_pub_use_goes_with_the_move_only_where_a_new_file_holds_it`)

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
