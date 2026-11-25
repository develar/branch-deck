## ADDED Requirements

### Requirement: The whole stack: The row of its root shows its step

The step of a whole stack that lands as one unit MUST show in the row of its root, which hosts it
(`NextStepView.hostsWholeStack`, decided with the steps by `repository_model::derive::next_steps` on
`domain_policy::landing_plan::lands_as_whole_stack`; [Whole Stack](../whole-stack/spec.md#requirement-the-safe-push-of-the-whole-stack-in-the-app-the-row-of-its-root-hosts-it)):
its slot, S, the preview and the first start of its "⋯" take the step of `stack:<root>`, the node of the row is the node
of the whole stack unless something of the root itself decides it (`landing_node::root_own_node`), and its Status cell
says the state the step of the whole stack waits in (`row_status::hosting_row_status`). The step of the root itself stays
what the table decides for it: its "⋯" offers it as "Safe Push Root Only", and `brd status` gives it as the root's
`next_step`, with `hosts_whole_stack`, the step of the slot in `stacks`. A straight chain has no such row: the step of
its whole stack is the step of its last branch, in that branch's row.

#### Scenario: The steps of a forking stack
- **WHEN** the steps of a stack that forks are derived
- **THEN** the root's view says that its row hosts the whole stack, its node the node of `stack:<root>` while nothing of its own decides it, and no other branch's view says so
- **AND** `brd status` says `hosts_whole_stack` of the root, as the app does
  [@test] crates/repository-model/src/derive_test.rs (`the_root_of_a_stack_that_forks_hosts_its_whole_stack_its_node_the_whole_stacks`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_stack_and_whole_stack_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: A whole stack that lands in pieces
- **WHEN** the whole stack holds a parked branch
- **THEN** no row hosts it: the row of the root shows its own step
  [@test] crates/repository-model/src/derive_test.rs (`the_root_of_a_stack_that_forks_hosts_its_whole_stack_its_node_the_whole_stacks`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_landing_cases_answer_alike_in_the_app_and_bd`)
