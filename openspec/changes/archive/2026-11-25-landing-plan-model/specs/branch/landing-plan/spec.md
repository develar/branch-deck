## ADDED Requirements

### Requirement: Units: What lands in one move is decided in Rust

The units of the plan MUST be decided in Rust (`domain_policy::landing_plan::landing_units`), in the order of the
table: every root by declared parent that is not parked, with the branches stacked on it that are not parked; its node
the node of its whole stack (its stack branch while it forks, else the last branch of its chain), the root's own while
the whole stack holds a parked branch; the branches of a stack whose own node needs a fix listed with it.

#### Scenario: The units of idea-1
- **GIVEN** the table of idea-1: the stack `air-home-watch` (`hosted-exit`, `claude-folds`), the chains of
  `genui-host-crate` and `air-tools-rust`, lone branches, three parked
- **WHEN** the units are decided
- **THEN** each stack is one unit with the node of its whole stack, each lone branch one, a parked branch none, and
  `hosted-exit`, whose node needs a fix, is listed with its stack
  [@test] crates/domain-policy/src/landing_plan_test.rs (`the_plan_of_idea_1_lands_two_robots_now_three_branches_next_and_two_units_after_them`)
  [@test] crates/domain-policy/src/landing_plan_test.rs (`a_stack_that_holds_a_parked_branch_takes_the_node_of_its_root`)
  [@test] crates/repository-model/src/derive_test.rs (`the_view_carries_the_units_of_the_landing_plan_by_name_and_a_patch_none_that_stayed`)

### Requirement: Waves: The units land in the order of their edges

The plan MUST order the units on their way in waves by the edges between them: "Landing now" (their robots run), "Next"
(nothing they need is on the way), then "After …" one wave after the deepest unit each needs; the units in master and
the parked branches in none, the parked apart; while the needs are computed the plan MUST be stale.

#### Scenario: Two robots, three branches next, two units after them
- **WHEN** `genui-host-crate` and `acp-load-replay` land in their robots, `air-tools-rust` needs the first and
  `air-home-watch` the second, `agent-id`, `IJAI-7037` and `ci-pack-cache` nothing
- **THEN** the waves are Landing now: both robots; Next: the three; After `acp-load-replay`, `genui-host-crate`: the
  two; the parked apart
  [@test] crates/domain-policy/src/landing_plan_test.rs (`the_plan_of_idea_1_lands_two_robots_now_three_branches_next_and_two_units_after_them`)

#### Scenario: A unit after a unit after a robot
- **WHEN** `ui` needs `flow`, which needs `codec`, landing
- **THEN** `flow` is one wave after `codec`, `ui` one after `flow`
  [@test] crates/domain-policy/src/landing_plan_test.rs (`a_unit_lands_one_wave_after_the_deepest_unit_it_needs`)

### Requirement: Land Together: Only a need of the build that nothing lands is offered

Land Together MUST be offered only for an edge of the build in state `Open` between two units that neither land, need
a fix nor are in master, never of a declared need, never while the plan is stale, never between units that need nothing
of each other; its action MUST be the restack of the root of the unit that waits onto the branch it needs, with the
refusal it fixes (`RewriteIntent::Restack`), one undo, never Stack Together.

#### Scenario: ci-pack-cache needs agent-id
- **WHEN** `ci-pack-cache` uses a name of `agent-id`, whose robot neither lands nor failed
- **THEN** Land Together restacks `ci-pack-cache` on `agent-id`, its command `brd restack ci-pack-cache agent-id`
  [@test] crates/domain-policy/src/landing_plan_test.rs (`land_together_restacks_onto_the_branch_a_need_of_the_build_nothing_lands_with_its_price`)

#### Scenario: A failed unit and a declared need
- **WHEN** the unit a need is of failed, or the need is declared
- **THEN** nothing is offered: the edge restarts that robot, or lands the branch first
  [@test] crates/domain-policy/src/landing_plan_test.rs (`land_together_is_never_offered_with_a_landing_or_failed_unit_nor_for_a_declared_need`)

#### Scenario: Onto a branch with a child
- **WHEN** the branch it needs has a branch stacked on it
- **THEN** the stack forks: "Land Together · Safe Push Whole Stack", the approvals in the pull request of that stack
  [@test] crates/domain-policy/src/landing_plan_test.rs (`land_together_onto_a_branch_with_a_child_forks_and_the_approvals_count_in_the_stack_pull_request`)

### Requirement: Land Together: The price is said as its risks

Land Together MUST carry its price as risks, each a field the page words: the robots ("1 robot instead of 2"), the
checks of Patronus (the same, those only one of them starts, or unknown with the reason), that a failure holds both, the
pull request whose approvals count, the code owners of the unit it needs that the other lacks, and off Patronus the
open pull requests that become one stack of GitHub.

#### Scenario: The checks of two units
- **WHEN** each starts the same checks, one starts others, or the checks are unknown
- **THEN** the price says the same count, the added names, or unknown with the reason
  [@test] crates/domain-policy/src/landing_plan_test.rs (`the_checks_of_two_units_are_the_same_the_added_or_unknown`)
  [@test] crates/domain-policy/src/landing_plan_test.rs (`land_together_restacks_onto_the_branch_a_need_of_the_build_nothing_lands_with_its_price`)

### Requirement: Land Together: The checks of a branch are those its paths start

The footprint of each branch MUST be the required checks of `.patronus/config.yaml` of the baseline its paths start,
matched as Patronus matches them, and the code owners of its paths, found with the landing needs in their session from
one `cat-file --batch`, remembered so that a sync that moves neither the baseline nor the paths starts no process; a
config that can't be read MUST leave the checks unknown with the reason.

#### Scenario: The checks of the Air plugin
- **WHEN** a branch changes the Air plugin and its readme, another the build, a third only a readme
- **THEN** each has the required checks its paths start and their owners; without the config the checks are unknown
  [@test] crates/trial-check/src/footprint_test.rs (`each_branch_starts_the_required_checks_of_its_paths_and_has_their_owners`)
  [@test] crates/trial-check/src/footprint_test.rs (`without_the_config_the_checks_are_unknown`)
  [@test] crates/commit-rules/src/checks_test.rs (`a_change_starts_the_checks_whose_last_matching_rule_is_not_negated`)

#### Scenario: No process of a sync that moves nothing
- **WHEN** the landing needs are computed again on the same baseline and paths
- **THEN** the footprint starts no git process
  [@test] crates/trial-check/src/process_budget_test.rs (`the_landing_needs_of_every_branch`)

### Requirement: Actions: One builder gives the actions of an edge to every surface

The actions of an edge MUST come from one builder (`domain_policy::actions::edge_actions`), the primary first, for the
slot, "⋯" and the Safe Push popover: none while it waits; "Restart acp-load-replay" of the failed robot of the branch it
needs; "Group 1a2b3c4d into claude-folds"; the Safe Push of a cycle; Land Together and "Land agent-id First" of a need
nothing lands; an unstack and the landing first of a declared need.

#### Scenario: The actions of each state
- **WHEN** an edge waits, its branch's robot failed, it needs an unassigned commit, closes a cycle, or nothing lands it
- **THEN** it has no action, the restart, the group, the start, or Land Together with Land … First
  [@test] crates/domain-policy/src/landing_plan_test.rs (`land_together_restacks_onto_the_branch_a_need_of_the_build_nothing_lands_with_its_price`)
  [@test] src/presenters/next-step.test.ts (`row 1′: names the branch the build needs and runs the fix, or opens the Safe Push`)
  [@test] src/presenters/next-step.test.ts (`row 1′: says one edge in one sentence, restarts a failed robot of the branch it needs and groups an unassigned commit`)

### Requirement: Popover: Land Together lands both from the popover and one undo puts them back

The Safe Push popover of a start refused by a need nothing lands MUST offer the actions of its edge as buttons, Land
Together focused; its press MUST run on fresh landing needs, make the two one unit of the Landing overview, and one undo
MUST put them back.

#### Scenario: Land Together and Undo
- **WHEN** the user presses Land Together in the popover of `lane-run`, which needs `stills-codec`
- **THEN** `lane-run` is stacked on `stills-codec`, the overview lists one unit of three branches, and Undo puts the
  two units and the need back
  [@test] tests/e2e/next-step/land-together.spec.ts (`should land a branch with the branch it needs in one robot from the Safe Push popover, one unit in the Landing overview, and undo it`)

### Requirement: Popover: The popover groups an unassigned commit and starts what lands a cycle

The popover of a start that needs an unassigned commit MUST offer its group as a button, and of a cycle the Safe Push
that lands both.

#### Scenario: Group and Start
- **WHEN** `lane-encode` needs a commit of no branch, or `gauge` needs `meter`, built on it
- **THEN** "Group 1a2b3c4d into lane-encode" groups it, "Safe Push meter" opens the start of `meter`
  [@test] tests/e2e/next-step/land-together.spec.ts (`should group the unassigned commit a branch needs from the Safe Push popover`)
  [@test] tests/e2e/next-step/land-together.spec.ts (`should start the Safe Push of the branch built on both from the Safe Push popover of a cycle`)

### Requirement: Popover: The detail of a refusal is short

The detail under the headline MUST be one or two short lines: the commits by short hash and the use by file name and
line, their subject and whole path on hover, and no sentence that says what a button of the popover does; `brd` MUST keep
the whole sentence of the gate.

#### Scenario: The detail of lane-run
- **WHEN** the popover of `ui-perf-nightly` says that `lane-run` uses Stills of `stills-codec`
- **THEN** the detail is "<hash> uses Stills at BuildLog.kt:3, which <hash> of stills-codec adds: the build of
  ui-perf-nightly fails.", the path and the subjects on hover
  [@test] tests/e2e/next-step/landing-needs.spec.ts (`should say in the Safe Push popover why the build fails before the start, and land the branch together with it`)

## MODIFIED Requirements

### Requirement: Rows: The need of a row is its first edge in one sentence

Another need of row 1′ MUST name its first edge: its hover one sentence of the build ("Uses 5 names of acp-load-replay
in acp_load_replay.rs (Effect, Input +3)" and the fix), a declared order in the gate's words; its click the primary
action of the edge: Land Together ("Needs stills-codec"), "Restart acp-load-replay" of a failed robot of the branch it
needs, "Group 1a2b3c4d into claude-folds", "Safe Push Whole Stack"; "⋯" every action of it.

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
inside the popover (`min-w-0`, `overflow-wrap: anywhere`), the actions of the edge as buttons, the first focused, and
"Start Anyway" an `outline` button below; Enter on the popup MUST NOT start anyway.

#### Scenario: A long path
- **WHEN** the start of `flow` needs `codec`, nothing lands it, and its use is in a long path
- **THEN** the headline is "Uses Codec of codec in FlowCodecIntegrationEntryPoint.kt", the detail names the file, its
  whole path on hover, and stays inside the popover, and "Land Together" has the focus, "Land codec First" beside it
  [@test] tests/e2e/next-step/lands-after.spec.ts (`should say a need nothing lands in one headline, focus its fix before Start Anyway and wrap a long path inside the popover`)
