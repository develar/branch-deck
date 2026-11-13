## MODIFIED Requirements

### Requirement: Units: A pair is shape, behaviour, moved or whitespace

A pair whose form differs MUST be `shape`, with the parts that changed. One of the same form that moved (every line
of it deleted in blocks of old lines alone and added in blocks of new lines alone, the same tokens) MUST go to the
unit `moved` where the index says its order is free and MUST be `behaviour` elsewhere, never whitespace. Else it is whitespace when its lines are equal by tokens, else `behaviour`. The lines of
a pair are those of the declaration and of every block of its unit.

#### Scenario: A change of a form and a change of a body
- **WHEN** the form of a paired declaration differs, and of another only its body changed
- **THEN** the first is `shape`, with the parts that changed
- **AND** the second is `behaviour`
  [@test] crates/review-plan/src/units_test.rs (`each_change_of_a_file_is_one_unit_and_every_block_is_in_one`)

#### Scenario: A pair whose lines are equal by tokens
- **WHEN** the form of a paired declaration is the same and its lines are equal by tokens
- **THEN** it is whitespace, and its blocks go to the unit of whitespace

#### Scenario: A declaration at another place of its file
- **WHEN** a function stands at the end of its file now, between two other functions, or there with a call wrapped on the way, its code the same, and the index says its order is free
- **THEN** its two blocks go to the unit `moved`, whatever functions stand around it at the two places
- **AND** it is `behaviour` where the index says its order matters, and without an index
- **AND** moved with a changed line it is `behaviour`, and so is a function whose statement stands at another place inside it, whatever the index says
- **AND** a blank line added where the function stays is whitespace: no line of it moved
  [@test] crates/review-plan/src/units_test.rs (`a_declaration_at_another_place_with_the_same_code_is_moved_where_the_index_says_its_order_is_free`)

#### Scenario: A space inside a string of a body
- **WHEN** the body of a paired declaration differs only by the spaces inside a string
- **THEN** it is `behaviour`
  [@test] crates/review-plan/src/units_test.rs (`a_block_of_whitespace_is_told_by_its_tokens`)

#### Scenario: A block that reaches past its declaration
- **WHEN** a block joins the two lines of a declaration and changes the comment after it, and the lines of the declaration alone are equal by tokens
- **THEN** the declaration is `behaviour`: the lines of its block are not
  [@test] crates/review-plan/src/units_test.rs (`a_declaration_is_whitespace_only_with_every_line_of_its_blocks`)

### Requirement: Units: A pair that moved is folded only where its place has no meaning

The place of a pair that moved has no meaning where the index says so (`accio moves`, `reordered`, `order: "free"`)
and no other code moved with it: a declaration of free order MUST hold the name of the pair on both sides (its own,
or one around it that moved whole), and every line of code its blocks add MUST be a line of such a declaration. Every
other pair that moved MUST be `behaviour`, also before the index answered and where none answers: a fold is never
wrong.

#### Scenario: The declarations whose place has no meaning
- **WHEN** a function, a type or a type alias of Kotlin, Java, C# or Rust moves across declarations, or a hoisted function of TypeScript (a `function` declaration, a method, a signature) or a type alias of it, and the accio of this machine is asked
- **THEN** it goes to the unit `moved`: a function of Kotlin, Java or C# across a property or a field and a function, a function of Rust across a type, a constant and a function, a hoisted function of TypeScript across a constant, a class of Kotlin moved whole
- **AND** a value or a constant (its initializer runs in its turn), an enum entry, a macro, a module, a function a `const` of TypeScript holds, a class or an enum of TypeScript, and every declaration of Starlark are `behaviour`
  [@test] crates/review-plan/src/units_test.rs (`a_moved_declaration_is_folded_only_where_the_index_says_its_place_has_no_meaning`)

#### Scenario: A method that moved with its decorator
- **WHEN** a method of a class of TypeScript or JavaScript moves with its decorator, on a line of its own or on the line of the method, or with a decorator of one of its parameters: the route of a controller with a parameter, above the routes with a fixed path
- **THEN** it is `behaviour`, a unit that is read: its decorator runs in another turn, and the routes are matched in the order of the methods
- **AND** a method without a decorator that moves across decorated ones goes to the unit `moved`: their order among themselves stays
- **AND** a method named by an expression (`[key()]() {}`), which runs in its turn as the class is made, is no declaration of the file: its lines are a change of the class, read
  [@test] crates/review-plan/src/units_test.rs (`a_moved_declaration_is_folded_only_where_the_index_says_its_place_has_no_meaning`)

#### Scenario: What a declaration crossed
- **WHEN** a function moves across code that is no declaration beside it (an initializer block, the end of an `impl` and the start of the next), a function of Rust across a macro, an overload of TypeScript across an overload of its name, or a function moves with the property beside it in one block
- **THEN** it is `behaviour`, a unit that is read
- **AND** so is a function the index says is free that moved with a property beside it in its blocks, which the index did not say is free
  [@test] crates/review-plan/src/units_test.rs (`a_moved_declaration_is_folded_only_where_the_index_says_its_place_has_no_meaning`)
  [@test] crates/review-plan/src/units_test.rs (`a_declaration_at_another_place_with_the_same_code_is_moved_where_the_index_says_its_order_is_free`)

#### Scenario: The plan of git alone, then the plan the index answered
- **WHEN** a plan of a function that moved is read with an index that says its order is free, and of a change where nothing moved
- **THEN** the plan of git alone reads it (`behaviour`) and the plan the index answered folds it, its story told again with the fold; the index is asked of the moves once, beside its other queries, in the query the diffs of the same subject ask (`FileDiffs::moves`, kept by the two trees)
- **AND** a change where nothing moved asks no moves
  [@test] crates/review-plan/src/stages_test.rs (`a_moved_declaration_is_read_in_the_plan_of_git_and_folded_once_the_index_says_its_order_is_free`)
