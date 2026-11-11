## MODIFIED Requirements

### Requirement: Groups of a Step: The contract first, the modules by their uses, the mechanical last

The groups MUST come as `contract`, then a group per module or folder, then `mechanical` last; a module whose
declarations another module of the step newly uses MUST come before that one, the order of their first stops between
the others and inside a cycle.

#### Scenario: The groups of a large step in their order
- **WHEN** a large step is cut into groups
- **THEN** the first is `contract`: the stops of the entries of the brief and of the other files of prose of the step
- **AND** a group per module or folder follows, a module whose declarations another module of the step newly uses
  before that one, the order of their first stops between the others and inside a cycle
- **AND** `mechanical` comes last: the build model, the lists of modules, imports alone, whitespace, moved declarations,
  renames, generated files
  [@test] crates/review-plan/src/groups_test.rs (`a_large_step_reads_as_its_contract_its_modules_in_the_order_of_their_uses_and_its_wiring`, `the_modules_template_reads_as_its_contract_its_modules_and_its_wiring`)
