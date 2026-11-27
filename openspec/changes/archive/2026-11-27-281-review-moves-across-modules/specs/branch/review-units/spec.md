## MODIFIED Requirements

### Requirement: Units: Declarations are paired by key, form, then order

A declaration MUST be paired across the two sides by its key and, among overloads of one key, by an equal form, then
in order. A module MUST pair only with a module: a module and a declaration of another kind of one key are two
declarations (Rust keeps `fn files` and `mod files` in two namespaces), in the units and in the API.

#### Scenario: Two overloads of one key
- **GIVEN** two overloads of one name, of which only one has a changed body
- **WHEN** the declarations are paired
- **THEN** each overload is paired with the one of an equal form
- **AND** only the one whose body changed is a unit, named by its signature
  [@test] crates/review-plan/src/units_test.rs (`an_overload_is_a_unit_of_its_own`)

#### Scenario: A function and a module of its name
- **GIVEN** `pub fn files` in `lib.rs`
- **WHEN** the commits move it into the new `files.rs` and declare `mod files;` in `lib.rs`
- **THEN** the function is a unit `removed` and the module a unit `added`, both of the key `.files`
- **AND** the API lists the function removed from `lib.rs` and added to `files.rs`, never a change of its form
  [@test] crates/review-plan/src/units_test.rs (`a_function_and_a_module_of_its_name_are_two_declarations`)
  [@test] crates/review-plan/src/api_test.rs (`a_function_replaced_by_a_module_of_its_name_is_removed`)

### Requirement: Units: A declaration moved whole to another file says where

A unit `removed` MUST say the file, the line and the unit of the place it went to (`moved_to`), and a unit `added`
those of the place it came from (`moved_from`), where one declaration of one side alone before the commits and one
after them, in two files, have the same code, the same content of their tokens, and a member of an `impl` of Rust the
same code of the header of its `impl`; the other place is a unit `added` or `removed`, or a declaration of a new or a
deleted file that stands in modules alone (each container the file declares a module), whose unit says nothing. Of a
code of several places on a side, each two of one member (what it is a member of but the modules, and its name) that
no other place of the code on its side is of MUST pair. The places left without a pair MUST pair by their code but
the modifiers of their reach and their member, one on each side, and both MUST say `visibility`. Every other unit MUST
have neither. A move between files is not the mechanical kind `moved`, a reorder inside one file.

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

#### Scenario: A member of an impl moves with its header
- **GIVEN** `fn area` of `impl Area for Shape` in `lib.rs`
- **WHEN** the commits move it into `impl Shape` of `other.rs`, or into `impl Area for Shape` there
- **THEN** the first is no move, the second says `moved_to` and `moved_from`
  [@test] crates/review-plan/src/moved_across_test.rs (`a_member_of_an_impl_moves_with_the_header_of_its_impl`)

#### Scenario: A member of a module
- **WHEN** a test of `mod tests` of `lib.rs` moves into `mod tests` of the new `odd.rs`, moves back into `lib.rs` out of
  the deleted `odd.rs`, or the inline `mod tests` becomes `mod tests;` with its tests in the new `tests.rs`, one indent
  less
- **THEN** each test says where it went or came from
  [@test] crates/review-plan/src/moved_across_test.rs (`a_member_of_a_module_moves_between_files_and_its_module_goes_with_it`)

#### Scenario: Two places of one code
- **WHEN** `fn total` of `impl Answer for A` and of `impl Answer for B`, one code, move to `a.rs` and `b.rs`
- **THEN** each says the file of its own `impl`
  [@test] crates/review-plan/src/moved_across_test.rs (`two_places_of_one_code_pair_by_what_they_are_members_of`)

#### Scenario: A move whose reach changed
- **WHEN** one commit makes `fn helper` and `struct Reread` with its field `pub(crate)`, and the next moves them to
  `other.rs`
- **THEN** in the range of both each says `moved_to` and `moved_from` with `visibility`, in the range of the second
  alone without it
- **AND** moved with a changed line beside the reach it is no move
  [@test] crates/review-plan/src/moved_across_test.rs (`a_move_whose_reach_changed_says_so`)
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_visibility_of_a_move_whose_reach_changed`)

#### Scenario: What is no move
- **WHEN** a function moves to another file with a changed line, one is added to two files and removed from one, one
  is removed from two files and added to one, or one moves to another class of its own file
- **THEN** no unit says `moved_from` or `moved_to`: a word on two files must not guess, and the diff of one file shows
  a move inside it
  [@test] crates/review-plan/src/moved_across_test.rs (`a_move_with_a_changed_line_or_of_two_places_on_a_side_says_nothing`)

#### Scenario: The places a move pairs
- **WHEN** the two places of one code are a unit `removed` and a unit `added` of two files that stay, a unit `removed`
  and a declaration of a new file that stands in modules alone (a test of its `mod tests` too), or such a declaration
  of a deleted file and a unit `added`
- **THEN** the unit of the declaration says the other place, and a file taken whole (`newFile`, `deletedFile`, in
  `deletedFiles` too) says nothing
- **AND** a new and a deleted file alone, a member of a type (part of its unit), a declaration of a file a kind takes
  whole (`mechanical`, `other`) and a pair of one declaration (`shape`, `behaviour`, `moved`) are no place of a move

### Requirement: Units: The place a move left is nothing to read where it holds the move alone

The slice of the place that declarations moved whole to other files left MUST be `moved_out` when it holds nothing but
them: the blocks of a unit change the old side as removing those declarations would, blank lines aside, with the
members of a removed type, the modules and the `impl` blocks of Rust they emptied (each declaration in one gone, no
code of its own but its header and its closing line), the imports and the names passed on (`pub use`) beside them, and
the lines of no declaration that are items of their own (a call of a macro) where a file the commits add holds the same
code as often; and they add nothing but the skeleton of the move: a module `mod x;` whose file is a file of the change,
an import, a name passed on (`pub use x::*;`), a blank line. A deleted file holds them, its imports, its package line
and its comments alone, a module of no code of its own by its members. The other places show what it removes. Every
other slice MUST NOT be `moved_out`: its lines are read where they stand.

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
- **THEN** the slice of `Shape` and the slice of the module `parsing`, which holds that block, are `moved_out`
  [@test] crates/review-plan/src/moved_across_test.rs (`a_type_moved_with_its_impl_and_code_replaced_by_its_module_are_the_move_alone`)

#### Scenario: The places a module of tests left
- **WHEN** a test of `mod tests` moves to a new file, a deleted file of `mod tests` and its test moves it into `lib.rs`,
  or the inline `mod tests` becomes `mod tests;`, its tests in `tests.rs`
- **THEN** the slice of the test, the deleted file and the slice of the unit `behaviour` of the module are `moved_out`
  [@test] crates/review-plan/src/moved_across_test.rs (`a_member_of_a_module_moves_between_files_and_its_module_goes_with_it`)

#### Scenario: A call of a macro beside the moved code
- **WHEN** `answer!(Parse);` stands beside `fn parse` in `lib.rs`, and both move into the new `parsing.rs`
- **THEN** the slice of `parse` is `moved_out`, and is read where the call changed on the way
  [@test] crates/review-plan/src/moved_across_test.rs (`a_line_of_no_declaration_moved_into_a_new_file_goes_with_the_move`)

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

### Requirement: In `brd`: `brd review-plan` prints the units

`brd review-plan` MUST print `units` (`"schema": 44`): per unit its `id`, `kind`, `name`, `key`, `signature`, `api`,
`test`, `line`, its `slices`, its `fingerprint` and, of a body changed in a rename the index has not proven, `renamed`,
of a declaration renamed so, `renamed_from`, of a declaration moved whole to another file, `moved_from` or `moved_to`,
with `visibility` where its reach changed with it; of a slice that holds nothing but declarations moved whole to other
files, `moved_out`.

#### Scenario: The units of a range
- **WHEN** `brd review-plan` runs on a range
- **THEN** it prints `units` (`"schema": 44`), per unit its `id`, `kind`, `name`, `key`, `signature`, `api`, `test`,
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
- **AND** a place of a move whose code is the same but for the modifiers of its reach has `visibility: true`; a place
  of the same code has no `visibility`
- **AND** a slice that holds nothing but declarations moved whole to other files has `moved_out: true`; no other slice
  has it
  [@test] crates/bd-rewrite/src/review_plan_test.rs (`prints_the_api_and_the_files_of_a_range`, `prints_the_renamed_of_a_body_changed_in_a_rename_alone_and_of_the_declaration`, `prints_the_moved_from_and_moved_to_of_a_declaration_moved_whole_to_another_file`, `prints_the_visibility_of_a_move_whose_reach_changed`)

### Requirement: In the Review Window: A declaration moved whole is read at its new place

The heading of a unit `added` with `moved_from` MUST say where it came from ("formatTotal — moved from Orders.kt"), of
a unit `removed` with `moved_to` where it went, and either ", visibility changed" where the place has `visibility`; a new file MUST name above its lines what moved into it, a deleted file
that is not the move alone where its declarations went, each other file once, a long list cut as every list is. Its
lines are read where it stands now, nothing of them folded, and what is left to read counts as the diffs count it.

#### Scenario: The headings of the moves of a branch
- **GIVEN** the review of the branch `moves` of the `review_moves` template, with the plan
- **WHEN** it shows its diffs
- **THEN** the heading of `formatTotal` in `Report.kt` says "formatTotal — moved from Orders.kt", and its lines show
- **AND** the new `Parsing.kt` says "parseTotal and parseSum — moved from Orders.kt", and `legacyTotal` in `Orders.kt`
  "legacyTotal — moved from Legacy.kt"
  [@test] tests/e2e/branch-review/review-moves.spec.ts (`should read a declaration moved to another file at its new place, its old place one row that opens it, in another step too, and name what moved into a new file and out of a deleted one`)
  [@test] src/presenters/review-units.test.ts (`words a declaration moved whole between two files where it stands now and where it left, as Rust decided`, `names in the row of the place two declarations left both and where each went, and a file taken whole by the moves into it, out of it, or its row`)

#### Scenario: A move whose reach changed
- **WHEN** a function made `pub(crate)` by one commit moves to `scope.rs` in the next, and the review shows both
- **THEN** its heading in `scope.rs` says "helper — moved from lib.rs, visibility changed", the place it left "helper —
  moved to scope.rs, visibility changed", and the card of its name "Moved from lib.rs, visibility changed"
- **AND** a place without `visibility`, or with `false`, says neither
  [@test] src/presenters/review-units.test.ts (`words a declaration moved whole between two files where it stands now and where it left, as Rust decided`)
  [@test] src/presenters/review-story.test.ts (`links a code name to its place, worded by its unit, its line and its step, and marks the section it names with the line that names it`)

#### Scenario: A place that holds more than the move
- **WHEN** the block of the place a declaration left holds more than the move, or a deleted file more than its moves
- **THEN** its lines show under the heading "formatTotal — moved to Report.kt", "legacyTotal — moved to Orders.kt"
  [@test] src/presenters/review-units.test.ts (`makes the place a declaration left one row that opens where it went, where Rust says it holds the move alone, and keeps its lines under the heading where it holds more`, `names in the row of the place two declarations left both and where each went, and a file taken whole by the moves into it, out of it, or its row`)

#### Scenario: A long list of moves
- **WHEN** ten functions move in one block of `Orders.kt` into the new `Parsing.kt`
- **THEN** the heading of `Parsing.kt` says "parseA, parseB, parseC and 7 more — moved from Orders.kt", each name in the
  code font, the seven in the hover of the count and heard by a screen reader after it
- **AND** the row of the place they left says "Moved parseA, parseB, parseC and 7 more to Parsing.kt", every name in its
  hover and its name to a screen reader, in a fold that hides it too; four are named whole, with no hover
  [@test] src/presenters/review-units.test.ts (`cuts a long list of the declarations that moved as every list is cut, in the heading of a new file and in the row of the place they left, the whole list in their hover`)
  [@test] src/lib/diff/rows.test.ts (`names in a fold of a section the blocks another section shows that it hides, and counts only its unchanged lines`)

#### Scenario: One place of a move in the range
- **WHEN** the review shows a range that holds one place of a move alone: a commit shown alone that copied the function,
  or Since Your Review of the commit that dropped its old copy
- **THEN** the plan of that range says no move: the function is new or removed there, and read
  [@test] crates/review-plan/src/moved_across_test.rs (`a_range_that_holds_one_place_of_a_move_says_nothing`)
