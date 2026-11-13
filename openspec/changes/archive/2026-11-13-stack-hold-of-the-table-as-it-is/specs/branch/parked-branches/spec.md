## MODIFIED Requirements

### Requirement: The app: The app's push refuses a parked branch in Rust too

The app's push MUST refuse it in Rust too, as `brd push` does: the run plans the push again over the table of its
session, after the rewrites admitted before it and the sync (`operations::push::execute_push`), and what keeps the
branch out of a push refuses it before the first effect, parked first. A push of a parked branch, of a branch built on
one, or of the stack branch of a stack that holds one pushes nothing, so a push the page did not check (a race, IPC)
never sends it.

#### Scenario: A push of a parked branch or one built on it
- **WHEN** the app pushes a parked branch, or a branch built on one, the page not checking it
- **THEN** nothing is pushed: "Nothing was pushed: login is parked: IJPL-1", code `parked`
  [@test] crates/domain-policy/src/push_test.rs (`a_branch_is_kept_out_parked_then_landed_then_held_unless_replaced_then_not_built`)

#### Scenario: A push of the stack branch of a stack that holds one
- **WHEN** the app pushes the stack branch of a stack that holds a parked branch
- **THEN** nothing is pushed: "… holds parked login; land the stack in pieces"
- **AND** the stack branch is held as the table is now, also while the view of the robots is of the table before the park
  [@test] crates/operations/src/push_test.rs (`a_member_parked_since_the_view_keeps_the_stack_branch_local`)
  [@test] crates/operations/src/pull_request_test.rs (`the_pull_request_of_a_whole_stack_pushes_its_stack_branch_by_the_table_as_it_is`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should leave the whole stack branch out of the header's push once a branch of it is parked, while the view of the robots is of the table before`)
