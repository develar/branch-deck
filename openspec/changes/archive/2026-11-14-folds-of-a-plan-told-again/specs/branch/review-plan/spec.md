## MODIFIED Requirements

### Requirement: In the Review Window: The Mechanical group starts collapsed

The group MUST start collapsed, also when the plan comes after the files or again with other groups, unless it holds
the selected file; so MUST the Wiring of a large step. A file selected only as the first of the tree without the plan,
which may be a generated one, MUST give way to the first file of the tree with the plan.

#### Scenario: The plan comes after the files
- **WHEN** the plan comes after the files
- **THEN** the Mechanical group starts collapsed unless it holds the selected file, and it opens to reveal a file in it
- **AND** a file selected only as the first of the tree without the plan gives way to the first file of the tree with the plan
  [@test] src/lib/file-tree.test.ts (`starts with the Mechanical group collapsed, and opens it to reveal a file in it`)
  [@test] tests/e2e/branch-review/review-plan.spec.ts (`should fold the generated files, each header saying why, a rendering at the end of the concern of its source, and keep the renderer's echo out of the repeated edits`)

#### Scenario: The plan comes again with its modules
- **WHEN** the plan of git showed, the build files of a large step folded, and the plan comes again with the modules, which cut the step into its groups
- **THEN** the Wiring of the step starts collapsed at its place now, and a row folded as a group of the plan of git that is another group now opens
- **AND** a row the reader folded stays folded
  [@test] src/lib/file-tree.test.ts (`folds the Wiring of a plan told again with its modules where it stands now, opens the row it held in the plan of git, and keeps the reader's other folds`)
  [@test] tests/e2e/branch-review/review-groups.spec.ts (`should fold the wiring of a large step the modules cut into its groups after the plan of git, as that plan folded its build files, and keep the step folded by the reader`)
