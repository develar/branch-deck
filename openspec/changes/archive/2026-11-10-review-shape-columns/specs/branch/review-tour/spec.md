## MODIFIED Requirements

### Requirement: The Map: A row names its module, its state, its share and its counts

A row of the shape MUST show "New module" or "Deleted module" when the change adds or deletes its module, its label in
the code font, a bar of its share of the changed lines against the largest row, its lines and files. The rows MUST
share their columns: the bars of all rows start at one edge, the lines and the files of all rows stand in a column
each, whatever the widths of their counts.

#### Scenario: A new module and a changed one
- **WHEN** the shape lists a module the change adds and one it changes
- **THEN** the new one shows "New module", a deleted one "Deleted module"
- **AND** each shows its label in the code font, a bar of its share of the changed lines against the largest row, its
  lines and files
  [@test] src/presenters/review-map.test.ts (`names the modules of a change in Rust's order: a new one with what it uses and where it plugs in, a changed one by its folders and what it adds beside its code`)

#### Scenario: Counts of different widths
- **WHEN** the rows of the shape have counts of different widths
- **THEN** their bars start at one edge, their lines end at one edge and their files start at one edge
  [@test] tests/e2e/branch-review/review-groups.spec.ts (`should stand the bars, the lines and the files of the rows of the shape in columns, whatever the widths of their counts`)

### Requirement: The Map: Under a row, what a new module uses, the folders of a changed one

Under a row, muted, the map MUST show of a new module what it newly uses and the plugins that hold it as content, of a
changed one its folders in words, the most files first, and what it adds beside its code.

#### Scenario: The line under a new module
- **WHEN** the row is of a module the change adds
- **THEN** the line under it names what it newly uses and the plugins that hold it as content ("uses view.gui and shared/code · content of com.intellij.air")
  [@test] src/presenters/review-map.test.ts (`names the modules of a change in Rust's order: a new one with what it uses and where it plugs in, a changed one by its folders and what it adds beside its code`)

#### Scenario: The line under a changed module
- **WHEN** the row is of a module the change changes
- **THEN** the line under it names its folders in words, the most files first, from five on the first three and the
  count of the rest ("in ui/embeds, ui/markdown, ui/util and references"), no count of files: the row has it
- **AND** what it adds beside its code ("+ depends on intellij.platform.ide.util.io", "Opens wider: isNightlyBuild")
  [@test] src/presenters/review-map.test.ts (`names the modules of a change in Rust's order: a new one with what it uses and where it plugs in, a changed one by its folders and what it adds beside its code`)
