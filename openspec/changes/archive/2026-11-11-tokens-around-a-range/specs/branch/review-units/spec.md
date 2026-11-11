## MODIFIED Requirements

### Requirement: Units: The declarations of a file are its spanned declarations

The declarations of a file MUST be those of its summary that are no `Binding`, none the parser may have misread
(`in_error`) and none inside a function or a value (a local): `spanned_declarations`. Their lines and those of the
import statements, which a summary counts as git does, MUST be read as lines of the diffs, which the blocks name: a
lone `\r` ends a line there too.

#### Scenario: A binding, a misread declaration and a local
- **GIVEN** a summary with a `Binding`, a declaration the parser may have misread (`in_error`) and a declaration inside
  a function or a value (a local)
- **WHEN** the declarations of the file are read (`spanned_declarations`)
- **THEN** none of the three is a declaration of the file

#### Scenario: A block below a lone carriage return
- **GIVEN** a file with a lone `\r` above its declarations
- **WHEN** a block changes the code of a declaration below it, and another wraps a call of a declaration
- **THEN** the first goes to its declaration, a change of its behaviour, and the second to the whitespace
  [@test] crates/review-plan/src/units_test.rs (`a_block_below_a_lone_carriage_return_goes_to_the_declaration_of_its_lines`)

### Requirement: Units: A pair is shape, behaviour or whitespace

A pair whose form differs MUST be `shape`, with the parts that changed; one whose form is the same and whose lines
differ by more than whitespace MUST be `behaviour`; one whose lines are equal by tokens too is whitespace, and its
blocks MUST go to the unit of whitespace. The lines of a pair MUST be those of the declaration and of every block of
its unit, from the first to the last on each side: a block may reach past the declaration that holds most of it.

#### Scenario: A change of a form and a change of a body
- **WHEN** the form of a paired declaration differs, and of another only its body changed
- **THEN** the first is `shape`, with the parts that changed
- **AND** the second is `behaviour`
  [@test] crates/review-plan/src/units_test.rs (`each_change_of_a_file_is_one_unit_and_every_block_is_in_one`)

#### Scenario: A pair whose lines are equal by tokens
- **WHEN** the form of a paired declaration is the same and its lines are equal by tokens
- **THEN** it is whitespace, and its blocks go to the unit of whitespace

#### Scenario: A space inside a string of a body
- **WHEN** the body of a paired declaration differs only by the spaces inside a string
- **THEN** it is `behaviour`
  [@test] crates/review-plan/src/units_test.rs (`a_block_of_whitespace_is_told_by_its_tokens`)

#### Scenario: A block that reaches past its declaration
- **WHEN** a block joins the two lines of a declaration and changes the comment after it, and the lines of the declaration alone are equal by tokens
- **THEN** the declaration is `behaviour`: the lines of its block are not
  [@test] crates/review-plan/src/units_test.rs (`a_declaration_is_whitespace_only_with_every_line_of_its_blocks`)
