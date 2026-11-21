## MODIFIED Requirements

### Requirement: `brd check`: A use of a name another branch adds is a missing declaration

With `branchdeck.symbols on`, `missing_declarations` MUST name each use, by a commit of a built branch, of a name
that a commit outside the branch and the branches it is built on adds or gives the form the use needs (an edge by
symbols, a reach that lets the use see it included), an override of it included, with the evidence and the file whose
change the use needs (`change_file`: the declaration's, or of `removes_after` the file whose use the commit removed).
git merges such a branch and its build fails, so a branch that builds otherwise MUST be `missing_declaration` (exit 1).

#### Scenario: A use of a name another branch adds
- **WHEN** a commit of a built branch uses a name that a commit outside it and its ancestors adds
- **THEN** `missing_declarations` names the use with the evidence and the file of the declaration
- **AND** the branch is `missing_declaration`, exit 1
  [@test] crates/bd-rewrite/src/check_test.rs (`a_branch_that_uses_a_name_another_branch_adds_is_a_missing_declaration`)

#### Scenario: An override of another branch's overload
- **WHEN** a commit of a built branch overrides an overload that another branch adds
- **THEN** the override is a missing declaration too
  [@test] crates/bd-rewrite/src/check_test.rs (`an_override_of_another_branchs_overload_is_a_missing_declaration`)

### Requirement: `brd check`: A parent that needs its child is an inverted stack

A branch whose conflict, dangling reference or missing declaration needs a commit of a branch stacked on it, at any
depth, MUST have `inverted_stack {branch, child, needs, fixes}` and keep its `result`; `needs` MUST name each commit of
the parent, the commit of the child it needs and, of a dangling reference or a missing declaration, the file of the
change it needs. The fixes MUST be those a trial passes for both branches: plan parts for `brd group` (`move_to_child`,
`swap`, and of a missing declaration `move_to_parent` where the child's commit changes only that file) or, of a
missing declaration otherwise, the `command` of `brd move-hunk` that moves the change of the file into the commit of
the use (`move_hunk`), tried on a rewritten history; no `swap` for a mutual dangling reference. A fix of a missing
declaration MUST leave the trial none of the missing declarations it was for and none the check didn't have. With no
fix that builds, a `reason` instead.

#### Scenario: A parent that needs its child
- **WHEN** the conflict of a branch needs a commit of a branch stacked on it
- **THEN** it has `inverted_stack` with what it needs and the fixes that a trial build passes for both branches, and
  keeps its `result`
  [@test] crates/bd-rewrite/src/check_test.rs (`a_parent_that_needs_its_child_is_an_inverted_stack`)

#### Scenario: A mutual dangling reference of a stack
- **WHEN** the dangling reference of a stack is mutual
- **THEN** the fixes hold no `swap`
  [@test] crates/bd-rewrite/src/check_test.rs (`a_mutual_dangling_reference_of_a_stack_is_an_inverted_stack_without_a_swap`)

#### Scenario: No fix builds
- **WHEN** no fix builds
- **THEN** `inverted_stack` has a `reason` instead
  [@test] crates/bd-rewrite/src/check_test.rs (`an_inverted_stack_that_no_fix_builds_says_why`)

#### Scenario: A parent that needs the visibility its child gives
- **WHEN** a commit of the parent uses an item that only a commit of its child makes `pub`, beside a change of another
  file
- **THEN** the parent is `missing_declaration` with an `inverted_stack` whose fixes hold `move_hunk`, whose command,
  run as printed, makes every branch build
  [@test] crates/bd-rewrite/src/check_test.rs (`a_parent_that_needs_its_childs_visibility_is_an_inverted_stack_with_a_hunk_to_move`)

#### Scenario: A child's commit of only the needed file
- **WHEN** the child's commit that the parent needs changes only the file of the need
- **THEN** the fixes hold `move_to_parent` and no `move_hunk`
  [@test] crates/bd-rewrite/src/check_test.rs (`a_childs_commit_of_only_the_needed_file_moves_into_the_parent`)
