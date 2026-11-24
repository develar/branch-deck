## ADDED Requirements

### Requirement: Rows: A start whose every edge waits lands after it

While every edge of the needs of a start waits (`EdgeState::Waits`) and no lost binding refuses it, its step MUST be
`AfterLanding`, a wait: the Status cell "Lands after acp-load-replay" (before its own failed robot "Restart after
acp-load-replay"), its hover the sentence of each edge, the slot empty, the node `Waiting`; `brd status` says
`after_landing` with `lands_after`.

#### Scenario: The branch it needs is landing
- **GIVEN** `codec` is landing in its Safe Push
- **WHEN** the start of `flow` needs a name `codec` adds
- **THEN** its Status cell says "Lands after codec", quiet, its hover "Uses Codec of codec in
  FlowCodecIntegrationEntryPoint.kt: codec is landing in its Safe Push, which tests it without this start: a start
  beside it fails too.", and its slot shows nothing
  [@test] crates/domain-policy/src/next_step_test.rs (`row_1_prime_waits_while_every_need_is_landing_in_a_running_safe_push`)
  [@test] src/presenters/next-step.test.ts (`row 1′: lands after the running Safe Push of what every need is of, a wait its status says, the start in its popover`)
  [@test] tests/e2e/next-step/lands-after.spec.ts (`should say on the row that a branch lands after the running Safe Push of what it needs, its slot empty and Start Anyway in the popover's ⋯`)
  [@test] crates/bd-remote/src/scenario_test.rs (`the_next_step_row_cases_answer_alike_in_the_app_and_bd`)

#### Scenario: Its own robot failed
- **WHEN** the robot of `flow` failed while `codec` is landing
- **THEN** the cell says "Restart after codec" before the failed robot, and the node waits
  [@test] crates/domain-policy/src/row_status_test.rs (`a_start_that_lands_after_what_it_needs_waits_in_the_cell_and_the_node_before_its_failed_robot`)

### Requirement: Rows: The need of a row is its first edge in one sentence

Another need of row 1′ MUST name its first edge: its hover one sentence of the build ("Uses 5 names of acp-load-replay
in acp_load_replay.rs (Effect, Input +3)" and the fix), a declared order in the gate's words; its click the fix of the
edge: the restack ("Needs stills-codec"), "Restart acp-load-replay" of a failed robot of the branch it needs, "Group
1a2b3c4d into claude-folds", "Safe Push Whole Stack".

#### Scenario: Merged names, a failed robot, an unassigned commit
- **WHEN** fifteen refusals of five names are one edge, the robot of the branch it needs failed, or the commit it
  needs is unassigned
- **THEN** the hover is one sentence without "brd check", the step restarts that robot, or groups the commit
  [@test] src/presenters/next-step.test.ts (`row 1′: says one edge in one sentence, restarts a failed robot of the branch it needs and groups an unassigned commit`)

#### Scenario: A cycle and a lost binding
- **WHEN** the branch it needs is built on what lands, or a lost binding refuses the start
- **THEN** the step is the Safe Push that lands both, or "Needs fix of 6a4e00db", which opens the popover
  [@test] src/presenters/next-step.test.ts (`row 1′: names the branch the build needs and runs the fix, or opens the Safe Push`)

### Requirement: Popover: The refusal is one headline with its fix first

The Safe Push popover of a refused start MUST say the first edge in a headline of one line, then the detail, wrapped
inside the popover (`min-w-0`, `overflow-wrap: anywhere`), its fix focused and "Start Anyway" an `outline` button
below; Enter on the popup MUST NOT start anyway.

#### Scenario: A long path
- **WHEN** the start of `flow` needs `codec`, nothing lands it, and its use is in a long path
- **THEN** the headline is "Uses Codec of codec in FlowCodecIntegrationEntryPoint.kt", the detail names the whole
  path and stays inside the popover, and "Stack flow on codec" has the focus
  [@test] tests/e2e/next-step/lands-after.spec.ts (`should say a need nothing lands in one headline, focus its fix before Start Anyway and wrap a long path inside the popover`)

### Requirement: Popover: A start that waits starts anyway from its ⋯

While every edge of the needs waits, the popover MUST offer no start in its footer: "Start Anyway" MUST be the item
of its "⋯", as a start beside the robot that lands what it needs fails; the headline says "Lands after codec".

#### Scenario: Start Anyway from ⋯
- **WHEN** the user opens the Safe Push of `flow` while `codec` is landing and picks "Start Anyway" of its "⋯"
- **THEN** the start runs with `allowMissing`
  [@test] tests/e2e/next-step/lands-after.spec.ts (`should say on the row that a branch lands after the running Safe Push of what it needs, its slot empty and Start Anyway in the popover's ⋯`)

### Requirement: Merge toast: The update of a merge offers the start that waited for it

The toast of the update that took the place of the toast of a merge MUST offer the start of the subject that waited
for that robot (`AfterLanding`, the one that lands the most), "Start flow": a click of its Safe Push once its step is
decided again (the needs of the tips the update moved); nothing MUST start by itself.

#### Scenario: Update Project of the merge
- **GIVEN** `flow` lands after `codec`, whose robot merges
- **WHEN** the user presses Update Project of the toast of the merge
- **THEN** the same toast says what landed and offers "Start flow", whose press starts it
  [@test] tests/e2e/next-step/lands-after.spec.ts (`should offer the start of the branch that waited in the toast of the merge once its Update Project landed it, and start it on the press`)
