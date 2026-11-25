## MODIFIED Requirements

### Requirement: The Safe Push of the whole stack in the app: The row of its root hosts it

A stack that forks and lands as its whole stack has no row of its own: the row of its root MUST host it (Rust's
`NextStepView.hosted`): its slot, S, the preview and the first start of its "⋯" are the whole stack's, as the
landing overview and the Plan word its unit; the root's own start stays in its "⋯" as "Safe Push Root Only". Before,
the slot showed the root's own Safe Push, which lands the root alone: the obvious button landed another thing than the
overview said.

#### Scenario: The slot of the root of a small stack
- **GIVEN** auth forks: login and oauth on one side, session on the other, no island
- **WHEN** its rows show
- **THEN** the slot of auth is "Safe Push · 4", as the landing overview says it of the unit, and its node is the node of that unit; while its stack branch is not built it is disabled with "The stack branch conflicts at 0caf9066 in config.js: the stack lands in pieces"
- **AND** a click, or S on the row, opens the popover of the whole stack at the row of auth, whose start lands every branch of it from its stack branch
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should start the whole stack from the slot and S of the root of a small stack that forks, as the landing overview says it, and the root alone from Safe Push Root Only`)
  [@test] crates/repository-model/src/derive_test.rs (`the_root_of_a_stack_that_forks_hosts_its_whole_stack_its_node_the_whole_stacks`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_stack_and_whole_stack_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: The root's own start
- **WHEN** the user opens the "⋯" of the row of auth
- **THEN** it leads with "Safe Push Whole Stack" (S), then "Safe Push Root Only", which lands auth alone ("Lands only auth into master, without the branches stacked on it, which the Safe Push of the whole auth stack lands"), and "Patronus ›" has "Dry Run Root Only"
- **AND** its "Whole Stack ›" has "Dry Run Whole Stack" and the rest, never the start the menu leads with
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should start the whole stack from the slot and S of the root of a small stack that forks, as the landing overview says it, and the root alone from Safe Push Root Only`)
  [@test] src/hooks/useBranchContextActions.test.ts (`leads with Safe Push Whole Stack (S), as its slot, once, and offers the root's own start as Safe Push Root Only and Dry Run Root Only, without a key`)

#### Scenario: The node and the Status cell of the root
- **WHEN** nothing of the root itself decides its node, and no island header shows the whole stack
- **THEN** its node is the node of the whole stack, and its Status cell says the state the whole stack waits in, never a state of its own step
- **AND** a failure, a block, its own robot or a Safe Push that lands it stays its own
  [@test] crates/domain-policy/src/landing_node_test.rs (`the_root_that_hosts_its_whole_stack_keeps_its_own_node_only_while_its_own_facts_decide_it`)
  [@test] crates/domain-policy/src/row_status_test.rs (`the_cell_of_the_root_that_hosts_its_whole_stack_says_the_wait_of_the_whole_stack`)

#### Scenario: The root of an island
- **WHEN** the stack has an island
- **THEN** the header shows the step of the whole stack and the row of the root none, as it leaves Push with Children to the header, and S on the row opens the popover at the header
- **AND** the row of the root keeps its own node and Status cell, which the header shows of the whole stack
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should start the whole stack from the island's ⋯ and with S on its header and on its root's row, the popover at the header`)

#### Scenario: A stack that lands in pieces, a straight chain
- **WHEN** the whole stack holds a parked branch, all of the root is in master, the repository is off Space (no step in the slot), or the stack is a straight chain
- **THEN** the row of the root keeps its own step: a parked branch lands the stack in pieces, a landed root offers no start as every landed branch, and the last branch of a chain shows its Safe Push, which is the whole stack
  [@test] crates/repository-model/src/derive_test.rs (`the_root_of_a_stack_that_forks_hosts_its_whole_stack_its_node_the_whole_stacks`)
  [@test] src/hooks/useBranchContextActions.test.ts (`leads with Safe Push Whole Stack (S), as its slot, once, and offers the root's own start as Safe Push Root Only and Dry Run Root Only, without a key`)

#### Scenario: A need outside the stack
- **WHEN** a branch of the stack needs a branch outside it
- **THEN** the slot of the root's row says it as the overview does ("Needs stills-codec", its fix; "Lands after stills-codec" while that branch is landing, no start), never a ready "Safe Push · 3" its start refuses
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should say on the root's row and in the popover what the whole stack needs outside it before the start, and start it anyway past the refusal`, `should say on the root's row that the whole stack lands after the running Safe Push of what it needs, with no start in its slot`)

### Requirement: The Safe Push of the whole stack in the app: The start checks its gate, pushes the stack branch, starts it

The start (`execute` of the `Land` of the whole stack) MUST check its landing gate first (the needs of its branches
outside it the view found fresh, else the engine runs the gate of the whole stack, `operations::land_session`), push
the stack branch alone
while its remote lacks it (leased, a robot it replaces canceled first), then start the robot from it, or comment on its
pull request.

#### Scenario: Its build needs a branch outside it
- **WHEN** the build of the whole stack needs a branch outside it
- **THEN** its popover says it before the start, Start Anyway its start, as the needs of its branches found it
- **AND** while the needs of new tips are computed, the start is refused by the gate of the whole stack, with the restack and Start Anyway
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should say on the root's row and in the popover what the whole stack needs outside it before the start, and start it anyway past the refusal`, `should refuse the Safe Push of a whole stack in the gate of its start while the needs of new tips are computed, with the restack and Start Anyway`)

#### Scenario: The start pushes the stack branch and starts the robot
- **GIVEN** the remote of the stack branch lacks it
- **WHEN** the user starts the whole stack
- **THEN** the stack branch alone is pushed, leased, a robot it replaces canceled first
- **AND** the robot starts from it, or the start comments on its pull request
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`)
