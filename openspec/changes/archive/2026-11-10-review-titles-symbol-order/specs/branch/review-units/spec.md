## MODIFIED Requirements

### Requirement: Order: The units follow the order of the files of the plan

The units MUST come in the order of the files of the plan (`order`: a declaration before its uses, a test right after
its subject), each unit at the first block it holds, a unit without a block right after the unit of the block that
changed its lines, the units without a block of one block in the order of their lines; a repeated edit at the place
of its first file.

#### Scenario: A declaration before its use
- **GIVEN** a file that declares a function and a file that uses it, both changed
- **WHEN** the units are put in order
- **THEN** the unit of the declaration comes before the unit of its use, each at the first block it holds
  [@test] crates/review-plan/src/units_test.rs (`the_units_follow_the_order_of_the_files_and_the_pair_of_a_file_changed_twice`)

#### Scenario: A unit without a block
- **WHEN** a unit has no block
- **THEN** it comes right after the unit of the block that changed its lines

#### Scenario: The units without a block of one block
- **WHEN** one block changes the lines of several declarations that hold none of it
- **THEN** they follow the unit of the block in the order the source declares them, not by their names, so the
  heading of the block names them so
  [@test] crates/review-plan/src/units_test.rs (`the_declarations_of_one_block_follow_in_the_order_of_their_lines`)

#### Scenario: A repeated edit
- **WHEN** a repeated edit spans several files
- **THEN** it comes at the place of its first file
