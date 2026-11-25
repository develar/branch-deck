## ADDED Requirements

### Requirement: The Safe Push of the whole stack in the app: The row of its root hosts it

A stack that forks and lands as its whole stack (`domain_policy::landing_plan::lands_as_whole_stack`: the rule of the
unit of the landing plan, so not while it holds a parked branch) has no row of its own: the row of its root MUST host
it, as Rust decides it (`NextStepView.hostsWholeStack`). Its slot shows the step of the whole stack ("Safe Push · 4",
disabled with "The stack branch conflicts at 0caf9066 in config.js: the stack lands in pieces" while its stack branch
is not built), in the words and with the node and the state of its unit in the landing overview and the Plan; S on the
row, the preview of the root and the first start of its "⋯" ("Safe Push Whole Stack", S) start the whole stack, its
popover at the root's row. Its node is the node of the whole stack unless something of the root itself decides it
(parked, all of it in master, a failure, no commits, a block, its own robot running or failed, a Safe Push that lands
it: `landing_node::root_own_node`), and its Status cell says the state the whole stack waits in
(`row_status::hosting_row_status`). The root's own start, which lands the root alone, MUST stay in its "⋯": "Safe Push
Root Only" after the start of the whole stack, without a key, "Dry Run Root Only" in "Patronus ›"; its "Whole Stack ›"
holds the rest of the actions of the whole stack, the start the menu leads with not again. In an island the header shows
the step of the whole stack right above the root's row: the row MUST leave its slot to the header, as it leaves Push
with Children there, and S on it opens the popover at the header. A straight chain keeps its rows as they are: the Safe
Push of its last branch is its whole stack, in the slot of that branch's row. `brd status` says it of the branch
(`next_step.hosts_whole_stack`).

Why: the landing overview and the Plan say "Safe Push · 4" of the unit, which lands into master in one robot; the slot
of the root showed the root's own "Safe Push", which lands the root alone, so the obvious button landed another thing
than the overview said. The whole stack has no row; the root's row is the one the stack hangs from, as the island
header is where there is one.

#### Scenario: The slot of the root of a small stack
- **GIVEN** auth forks: login and oauth on one side, session on the other, no island
- **WHEN** its rows show
- **THEN** the slot of auth is "Safe Push · 4", as the landing overview says it of the unit, and its node is the node of that unit
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
- **WHEN** nothing of the root itself decides its node
- **THEN** its node is the node of the whole stack, and its Status cell says the state the whole stack waits in, never a state of its own step
- **AND** a failure, a block, its own robot or a Safe Push that lands it stays its own
  [@test] crates/domain-policy/src/landing_node_test.rs (`the_root_that_hosts_its_whole_stack_keeps_its_own_node_only_while_its_own_facts_decide_it`)
  [@test] crates/domain-policy/src/row_status_test.rs (`the_cell_of_the_root_that_hosts_its_whole_stack_says_the_wait_of_the_whole_stack`)

#### Scenario: The root of an island
- **WHEN** the stack has an island
- **THEN** the header shows the step of the whole stack and the row of the root none, and S on the row opens the popover at the header
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should start the whole stack from the island's ⋯ and with S on its header and on its root's row, the popover at the header`)

#### Scenario: A stack that lands in pieces, a straight chain
- **WHEN** the whole stack holds a parked branch, or the stack is a straight chain
- **THEN** the row of the root keeps its own step: a parked branch lands the stack in pieces, and the last branch of a chain shows its Safe Push, which is the whole stack
  [@test] crates/repository-model/src/derive_test.rs (`the_root_of_a_stack_that_forks_hosts_its_whole_stack_its_node_the_whole_stacks`)
  [@test] src/hooks/useBranchContextActions.test.ts (`leads with Safe Push Whole Stack (S), as its slot, once, and offers the root's own start as Safe Push Root Only and Dry Run Root Only, without a key`)

## MODIFIED Requirements

### Requirement: The Safe Push of the whole stack in the app: It is asked from the header, the root's menu and popovers

It MUST be asked from the header of an island (its slot, "Safe Push Whole Stack" and "Dry Run Whole Stack" of its "⋯",
S on the focused header and on the row of its root), the popover at the header's slot
([Stack Island](../stack-island/spec.md#header)); without an island from the slot of its root's row, S on it and the
root's menu, which leads with "Safe Push Whole Stack" ([The row of its root hosts it](#requirement-the-safe-push-of-the-whole-stack-in-the-app-the-row-of-its-root-hosts-it));
from "Whole Stack ›" of the menu of every other branch of the stack, and from the Safe Push popover of the root and of
any branch of a stack that forks ("Land the whole stack: Safe Push Whole Stack",
[Patronus](../patronus/spec.md#starting-a-safe-push-or-a-dry-run)).

#### Scenario: The header of an island
- **WHEN** the user chooses the slot of the header, "Safe Push Whole Stack" or "Dry Run Whole Stack" of its "⋯", or presses S on the focused header or on the row of its root
- **THEN** the popover opens at the header's slot
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`, `should start the whole stack from the island's ⋯ and with S on its header and on its root's row, the popover at the header`, `should check the whole stack in a Dry Run from the island's ⋯, its popover at the header`)

#### Scenario: Whole Stack › of the root's menu of a small stack
- **WHEN** the user chooses "Safe Push Whole Stack", which the root's menu of a small stack leads with, or "Dry Run Whole Stack" of its "Whole Stack ›"
- **THEN** the popover opens at the root's row (`getWholeStackStartItems`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a small stack with its side branch from the root's menu, its popover at the root`)

#### Scenario: The Safe Push popover of a branch of the stack
- **WHEN** the Safe Push popover of the root or of any branch of a stack that forks is open
- **THEN** it offers "Land the whole stack: Safe Push Whole Stack", which opens the popover of the whole stack at the header of the root's island, else at the root's row
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should offer the whole stack in the Safe Push popover of its root and of its branches, which opens its own popover`)

### Requirement: The Safe Push of the whole stack in the app: Its steps show where its popover opens

Its steps MUST show where its popover opens, as the start of a branch shows them in its slot: "Checking…" while the
facts load, the spinner and "45%" of its push, "Starting…", in the button of the header's slot, else in the slot of the
root's row of a small stack, which hosts the whole stack (`HostedStackSlot`); the push button of the root's row stays the
root's own meanwhile (`useStackBranchPushAlone`).

#### Scenario: The steps in the header's slot
- **WHEN** the start of the whole stack of an island runs
- **THEN** the button of the header's slot shows "Checking…" while the facts load, the spinner and "45%" of its push, then "Starting…"
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`)

#### Scenario: The steps in the root's slot of a small stack
- **WHEN** the start of the whole stack of a small stack runs
- **THEN** the root's slot, which shows the step of the whole stack, shows its steps (`HostedStackSlot`), then that step again
- **AND** the push button of the root's row stays the root's own
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should land a small stack with its side branch from the root's menu, its popover at the root`)
