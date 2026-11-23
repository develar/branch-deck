## MODIFIED Requirements

### Requirement: `brd check`: A use of a name another branch adds is a missing declaration

With `branchdeck.symbols on`, `missing_declarations` MUST name each use, by a commit of a built branch, of a name
that a commit outside the branch and the branches it is built on adds or gives the form or reach the use needs, an
override of it included, with the evidence and `change_file`, the file of the change it needs. git merges such a
branch and its build fails, so a branch that builds otherwise MUST be `missing_declaration` (exit 1).

#### Scenario: A use of a name another branch adds
- **WHEN** a commit of a built branch uses a name that a commit outside it and its ancestors adds
- **THEN** `missing_declarations` names the use with the evidence and the file of the declaration as `change_file`
  (of `removes_after` the file whose use the commit removed)
- **AND** the branch is `missing_declaration`, exit 1
  [@test] crates/bd-rewrite/src/check_test.rs (`a_branch_that_uses_a_name_another_branch_adds_is_a_missing_declaration`)

#### Scenario: An override of another branch's overload
- **WHEN** a commit of a built branch overrides an overload that another branch adds
- **THEN** the override is a missing declaration too
  [@test] crates/bd-rewrite/src/check_test.rs (`an_override_of_another_branchs_overload_is_a_missing_declaration`)

#### Scenario: A Rust method call that only another branch's arity takes
- **WHEN** a commit of a built branch calls a method on a value with the count of arguments that only the form a commit
  of another branch gives the method takes
- **THEN** `missing_declarations` names the call with the evidence `method_arity`
- **AND** the branch is `missing_declaration`, exit 1
  [@test] crates/bd-rewrite/src/check_test.rs (`a_rust_method_call_that_only_another_branchs_arity_takes_is_a_missing_declaration`)
