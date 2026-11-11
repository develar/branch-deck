## MODIFIED Requirements

### Requirement: Units: A block goes to the first unit it fits

A block MUST go to the repeated edit of its file's group when the block is that edit; else to the imports when its
changed lines are each of an import statement or blank, one at least; else to the whitespace when it changes
whitespace alone, in a file of a grammar its lines equal by tokens; else to the declaration that holds most of its
changed lines; else to the change outside any declaration, one unit per run of such blocks.

#### Scenario: A block that is the repeated edit
- **WHEN** a block is the repeated edit of its file's group
- **THEN** it goes to the repeated edit of that group
  [@test] crates/review-plan/src/units_test.rs (`a_repeated_edit_and_the_whitespace_are_one_unit_each_over_their_files`)

#### Scenario: A block of imports alone
- **WHEN** a block is no repeated edit and its changed lines are each of an import statement or blank, one at least
  (`file_diffs::service::imports_only`, the test the diffs pale a block by)
- **THEN** it goes to the imports
  [@test] crates/review-plan/src/units_test.rs (`each_change_of_a_file_is_one_unit_and_every_block_is_in_one`)

#### Scenario: A block of whitespace alone
- **WHEN** a block is neither of those and changes whitespace alone ([Review Plan](../review-plan/spec.md#requirement-mechanical-changes-whitespace-is-told-by-the-tokens-of-a-grammar)):
  its lines are equal by tokens, as those of a call wrapped inside its brackets
- **THEN** it goes to the whitespace
- **AND** a block that changes the spaces inside a string does not
  [@test] crates/review-plan/src/units_test.rs (`a_repeated_edit_and_the_whitespace_are_one_unit_each_over_their_files`, `a_block_of_whitespace_is_told_by_its_tokens`)

#### Scenario: A block inside declarations
- **WHEN** a block is none of those and declarations hold its changed lines
- **THEN** it goes to the declaration that holds most of its changed lines, the innermost one around each line
- **AND** the lines are read on the new side, on the old side for a block that only removes
  [@test] crates/review-plan/src/units_test.rs (`a_block_belongs_to_the_declaration_of_most_of_its_lines`)

#### Scenario: Blocks outside any declaration
- **WHEN** blocks are none of those and no declaration holds their changed lines
- **THEN** they go to the change outside any declaration, one unit per run of such blocks

### Requirement: Units: A pair is shape, behaviour or whitespace

A pair whose form differs MUST be `shape`, with the parts that changed; one whose form is the same and whose lines
differ by more than whitespace MUST be `behaviour`; one whose lines are equal by tokens too is whitespace, and its
blocks MUST go to the unit of whitespace.

#### Scenario: A change of a form and a change of a body
- **WHEN** the form of a paired declaration differs, and of another only its body changed
- **THEN** the first is `shape`, with the parts that changed
- **AND** the second is `behaviour`
  [@test] crates/review-plan/src/units_test.rs (`each_change_of_a_file_is_one_unit_and_every_block_is_in_one`)

#### Scenario: A pair whose text is the same without whitespace
- **WHEN** the form of a paired declaration is the same and its lines are equal by tokens
- **THEN** it is whitespace, and its blocks go to the unit of whitespace

#### Scenario: A space inside a string of a body
- **WHEN** the body of a paired declaration differs only by the spaces inside a string
- **THEN** it is `behaviour`
  [@test] crates/review-plan/src/units_test.rs (`a_block_of_whitespace_is_told_by_its_tokens`)

### Requirement: Identity: The fingerprint names the change across plans

`fingerprint` MUST name the change across plans: of a declaration a digest of its key, its signature and the content
of its tokens on both sides ([Equal by Tokens](../../commit/equal-by-tokens/spec.md)); of the blocks of a slice
their file and the content of their tokens; of a file taken whole its path and blobs; of a group the set of the
fingerprints of its slices. A side without tokens MUST be read by its text without whitespace.

#### Scenario: The same change on another base, and another change
- **GIVEN** a change of the body of a declaration
- **WHEN** the same change sits on another base, below a line added before the declaration
- **THEN** its unit has the same `fingerprint`
- **AND** a different change of that body has another one
  [@test] crates/review-plan/src/units_test.rs (`the_fingerprint_names_the_change_wherever_it_sits`)

#### Scenario: A slice, a file taken whole and a group
- **WHEN** a unit is of the blocks of a slice, of a file taken whole, or a group
- **THEN** the fingerprint of the blocks of a slice is of their file and the content of their tokens
- **AND** that of a file taken whole of its path and blobs
- **AND** that of a group of the set of the fingerprints of its slices

#### Scenario: The same change with other whitespace
- **WHEN** the same change of a body is written with other whitespace: wrapped, indented again
- **THEN** its unit has the same `fingerprint`
- **AND** so has the same change of a function that is the last of its file: the content is the texts of the tokens and the tree between them
- **AND** a change that differs by the spaces inside a string, or by a line break that ends a statement, has another one
  [@test] crates/review-plan/src/units_test.rs (`the_fingerprint_is_the_same_after_a_change_of_whitespace_alone`)
