## ADDED Requirements

### Requirement: Moves: The whitespace at the ends of a line that changed otherwise than its move is a range

Under the policy that counts every character, a line of a move whose indent is another text and did not move by the
shift, or whose whitespace after its text is another, MUST be an edited line of the move with that whitespace as a
range, a blank line too: a view that shows the old lines of a move once hides no change. Where the indentation has
no meaning it MUST count in no quarter and end no move. Under a policy that leaves that whitespace out it MUST be no
change of a move.

#### Scenario: Under each policy
- **WHEN** a function moves and two of its five lines are indented by two more columns than the others, a line of a raw string by eight more, a line lost or got its indent, got tabs for its spaces, or the whitespace after its text was added, lost or changed
- **THEN** under the policy that counts every character it is one move, each such line an edited line whose range is its indent or the whitespace that ends it, beside the ranges of its text
- **AND** two such lines of five, and such a line at an end of the move, are lines of the move
- **AND** under `Trim`, `Ignore` and `IgnoreBlankLines` the move has no such range, where the indentation has a meaning too
  [@test] crates/text-diff/src/moves_test.rs (`the_whitespace_at_the_ends_of_a_line_that_changed_otherwise_than_its_move_is_a_range`)
