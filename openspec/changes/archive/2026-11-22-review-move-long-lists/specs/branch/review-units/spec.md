## MODIFIED Requirements

### Requirement: In the Review Window: A declaration moved whole is read at its new place

The heading of a unit `added` with `moved_from` MUST say where it came from ("formatTotal — moved from Orders.kt"), of
a unit `removed` with `moved_to` where it went; a new file MUST name above its lines what moved into it, a deleted file
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
