## MODIFIED Requirements

### Requirement: In the Review Window: The groups of a step are its headings in the tree

A step with groups ([Groups of a Step](#groups-of-a-step)) MUST list them in the tree as the headings of the step, not
as one more level: under its row and its commit rows a row per group at the indent of its files, its icon of what it
holds, its name, its line counts and the files of the chosen owner chip ("3 of 13"), the root of its module in its
hover.

#### Scenario: The rows of the groups
- **WHEN** a step with groups is listed in the tree
- **THEN** a row per group stands at the indent of its files, its name "Contract", the label of its module in the code
  font, its folder, the mechanical group "Wiring" while it holds only the build model, the lists of modules and the
  build files whose change lies in the regions their generator writes, else "Mechanical"
- **AND** its line counts and the files of the chosen owner chip ("3 of 13")
  [@test] src/lib/file-tree.test.ts (`lists a group per section of the step, its files by name with the folder Rust gives them in their module, its tests last, the build files under Wiring`, `words the mechanical group Mechanical when it holds more than build files and lists of modules, and keeps a step without groups a list`)
  [@test] src/presenters/review-steps.test.ts (`names the mechanical group Wiring while it holds the build model alone, a build file generated in its regions too, else Mechanical, its hover saying what such a group holds: moved declarations too`)
  [@test] tests/e2e/branch-review/review-groups.spec.ts (`should read a large step by its groups: their files flat by their directory in the module, their tests last, a new module marked, the wiring folded at the end`)
