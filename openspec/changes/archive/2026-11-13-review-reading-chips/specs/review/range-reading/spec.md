## MODIFIED Requirements

### Requirement: Window: The header says what the pull request leaves to read

The header of the review window of a pull request, and of a layer of a stack, MUST say what it leaves to read once its
fetch answered and the repository reads review plans, read by `get_range_readings`; its labels MUST stand as chips,
and the author's word the diff proves wrong is said once, as in a row of Your Turn: the label's own chip in the
warning tone, or a compact chip of the title after them, its hover why.

#### Scenario: Formatting alone
- **WHEN** a pull request changes the spaces between tokens alone, labelled refactoring
- **THEN** its header says "Nothing to read · formatting", the label shows, and no chip flags it
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should say a pull request of formatting alone leaves nothing to read, its label of a refactoring then holding`)

#### Scenario: A refactoring that changes a body
- **WHEN** a pull request labelled refactoring changes the body of a function
- **THEN** its header says "1 change to read in 1 file", the label chip "refactoring" in the warning tone with its hover, no other chip, and the header of its changes keeps the reading
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should flag a pull request labelled a refactoring whose diff changes the body of a function`)
  [@test] src/presenters/range-reading.test.ts (`words the author's word the diff proves wrong as the author's`)

## ADDED Requirements

### Requirement: Verdict: A narrow list and the sum of a stack

A narrow list (the layers of a stack in the column of its window) MUST say a verdict compactly, "Nothing to read", "4
to read", "Tests only · 1 to read", the whole verdict in its hover. The sum of a stack MUST count each thing of its own
unit: "1 of 7 with nothing to read · 14 changes to read", "Nothing to read" when no layer has anything.

#### Scenario: The compact words and the sum
- **WHEN** the verdicts of a list and the sums of stacks are worded
- **THEN** they read as above, "2 being read" while layers are read
  [@test] src/presenters/range-reading.test.ts (`says a verdict compactly for a narrow list`, `sums the layers of a stack, each count of its own unit`)
  [@test] crates/domain-policy/src/range_reading_test.rs (`a_stack_counts_its_layers_and_opens_on_the_first_one_to_read_once_those_below_are_known`)
