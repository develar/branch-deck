## MODIFIED Requirements

### Requirement: `brd safe-push`: The refusal of a missing declaration names the fix

The message MUST name the use, the commit that adds the name with its branch, and the fix: `brd restack <root> <branch>`,
which lands that branch too; where that branch is built on what lands, the start that lands both (`brd safe-push
<branch>`, `--whole-stack`), never a move; the move of an unassigned commit;
"(N more: brd check)" for the others. `--dry-run` MUST list it in `refusals`, and `--allow-missing` MUST start anyway.

#### Scenario: The message, the dry run and the allowance
- **WHEN** a missing declaration refuses the start
- **THEN** the message names the use, the commit that adds the name with its branch, and `brd restack <root> <branch>`,
  the start that lands both when that branch is built on what lands, or the move of the commit when it is unassigned,
  with "(N more: brd check)" for the others
- **AND** `--dry-run` lists it in `refusals`, and `--allow-missing` starts anyway
  [@test] crates/bd-remote/src/safe_push_test.rs (`a_name_another_branch_adds_refuses_the_safe_push_unless_allowed`, `a_name_a_sibling_adds_refuses_the_branch_alone_but_not_the_whole_stack`)

### Requirement: `brd status`: Each branch on Space shows its `landing_needs`

On Space (the push URL of `origin`), each branch MUST show `landing_needs`: what its Safe Push needs of a commit
outside what it lands, the refusals `brd safe-push` would give its start
(`trial_check::landing_gate::landing_needs`, one trial check of every branch, less what merged Safe Pushes landed), so
an agent sees them without `brd check`.

#### Scenario: A need of a branch outside it
- **GIVEN** Space
- **WHEN** a branch uses a commit of a branch outside what it lands
- **THEN** its `landing_needs` has per need the `code` (`missing_declaration`, `dangling_reference`), the
  `identifier`, `at` (`file:line` of the use), the commit it `needs` and its `needs_branch`
- **AND** the `fix` (`brd restack <root> <branch>`, which the user approves first; `brd safe-push <branch>`, with
  `--whole-stack`, where `needs_branch` is built on what lands; `move <commit> into <branch>` of an unassigned commit)
  and `landing_in`, the robot of the running Safe Push that lands `needs_branch` (of a dangling reference too)
  [@test] crates/bd-remote/src/landed_test.rs (`status_names_what_the_safe_push_of_a_branch_needs_of_a_branch_outside_it`)
  [@test] crates/trial-check/src/landing_gate_test.rs (`the_needs_of_a_branch_are_the_refusals_of_its_start`)
