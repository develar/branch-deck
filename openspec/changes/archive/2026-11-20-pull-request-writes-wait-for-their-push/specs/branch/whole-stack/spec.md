## MODIFIED Requirements

### Requirement: The whole stack in the app: Create Pull Request of the Stack… is disabled while it cannot be

"Create Pull Request of the Stack…" MUST be disabled with the reason while the sync didn't build the stack branch
(`WholeStackView.forking.notBuilt`), while the push of its stack branch can't be made (a running Safe Push holds it,
`derive::stack_push_blocker`, by which the engine refuses it too; Request Review… of its pull request waits for it the
same way), and once every branch of the stack landed (`reasons::stack_landed`).

#### Scenario: The stack branch is not built
- **WHEN** the sync didn't build the stack branch (`WholeStackView.forking.notBuilt`)
- **THEN** "Create Pull Request of the Stack…" is disabled with the reason
  [@test] crates/domain-policy/src/actions_test.rs (`the_whole_stack_starts_as_its_step_says_and_lands_in_pieces_while_its_stack_branch_is_not_built`)

#### Scenario: The push of the stack branch is held
- **WHEN** a running Safe Push holds the push of the stack branch the remote lacks
- **THEN** "Create Pull Request of the Stack…" and "Request Review…" of its pull request are disabled with the hold
  [@test] crates/domain-policy/src/actions_test.rs (`a_pull_request_write_waits_for_the_push_it_makes_first_as_the_engine_refuses_it`)

#### Scenario: Every branch of the stack landed
- **WHEN** every branch of the stack landed
- **THEN** "Create Pull Request of the Stack…" is disabled with the reason of `reasons::stack_landed`
  [@test] crates/domain-policy/src/actions_test.rs (`the_whole_stack_starts_as_its_step_says_and_lands_in_pieces_while_its_stack_branch_is_not_built`)
