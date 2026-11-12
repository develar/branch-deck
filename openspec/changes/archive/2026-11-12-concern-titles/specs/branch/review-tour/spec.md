## MODIFIED Requirements

### Requirement: In the Review Window: A task of one commit is named by the label it tells

A step of the concerns MUST be named by the label of the first section of the body of its commit it tells (`title`,
"The splash"; a verification and a Look closely aside), else by its concern, what it does to its name as a sentence
or its name alone as code, wherever the step is named, and keeps its main rows ([Review Concerns](../review-concerns/spec.md#in-the-review-window)).

#### Scenario: A task of a labelled section
- **WHEN** a step of the concerns tells a labelled section of the body of its commit
- **THEN** it is named by the label of the first one it tells ("The splash")
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should title each task of one commit by the label of the section of its body it tells`)

#### Scenario: A task of no label
- **WHEN** a step of the concerns tells no labelled section
- **THEN** it is titled by its concern wherever the step is named (the tree, the contents, its opening, the bar of the
  step, the heading of a card of a story mark): what the concern does to its name, a sentence set as text ("Add
  probe", [Review Concerns](../review-concerns/spec.md#requirement-in-the-review-window-the-tree-lists-the-concerns-at-its-top)),
  else its name alone, an identifier set in the code font
- **AND** it keeps its main rows
  [@test] src/presenters/review-steps.test.ts (`names a step of a concern by the label of the section of the body it tells, else by its concern as code, keeps its main rows, and tells no step of a story Rust tells in none`)
