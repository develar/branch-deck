---
name: Landing Plan
description: Requirements for the needs of the starts of the branches as edges of a landing plan (`domain_policy::landing_edges`), merged by the branch they need, lifted to the root of a stack, each with its kind, its state and its actions from one builder; the row that names its first edge in one sentence, or waits while what its start needs is landing ("Lands after acp-load-replay"); the Safe Push popover of a refused start with its short detail and the actions of the edge; the start that waited offered by the toast of the merge; the plan itself (`domain_policy::landing_plan`): its units, its waves and Land Together with its price, of the footprint of each branch in the checks of Patronus.
style: plain-1
targets:
  - crates/domain-policy/src/landing_edges.rs
  - crates/domain-policy/src/landing_plan.rs
  - crates/domain-policy/src/actions.rs
  - crates/domain-policy/src/landing_needs.rs
  - crates/domain-types/src/landing_plan.rs
  - crates/trial-check/src/footprint.rs
  - crates/commit-rules/src/checks.rs
  - crates/domain-types/src/landing.rs
  - crates/domain-policy/src/next_step.rs
  - crates/domain-policy/src/row_status.rs
  - crates/repository-model/src/derive.rs
  - crates/bd-core/src/next_step_summary.rs
  - src/presenters/landing.ts
  - src/presenters/next-step.ts
  - src/hooks/useBranchNextStep.ts
  - src/components/branch/SafePushPopover.tsx
  - src/lib/merge-dependents.ts
  - src/lib/landing-toast.ts
---

# Landing Plan

Status: Implemented
Date: 2026-11-25

## Purpose

Agents cut the work of one local branch into logical virtual branches, and the landing gate refuses a start once per
name it misses, so one need between two pieces reads as many refusals as names times the branches whose start lands
it. The landing plan says each need once, as an edge from the piece that waits to the piece it needs, with what it
waits for: a running Safe Push, a restart, a fix, or the start of what lands both. A row then says one short state,
and a wait is shown as a wait, never as the user's step. The plan the edges make is a fact of the session, the same for
the app and `brd plan`: what lands in one move, what lands now, next and after what, and where two pieces may land in
one robot, with what that costs.

## Goals

- One edge per need of a pair, its names as evidence; one edge of a stack on its root.
- A start whose every need is landing waits quietly ("Lands after acp-load-replay"), and the toast of that merge offers
  the start afterwards.
- A commit of a branch stays in its branch: only an unassigned commit moves.
- Landing two pieces in one robot is an offer with its price, never a silent merge of independent pieces.

## Non-goals

- No start by itself after the merge it waited for; the user presses it.
- No landing together of a declared need: the need is what keeps the two apart.
- The picture of the plan (the Plan view of Landing) comes later; the page shows the actions of an edge where the row
  and the popover say it.
- No Land Together of several edges in one rewrite: a branch has one parent, the next edge is offered once the first
  lands.

## Requirements

### Requirement: Edges: The refusals of one pair are one edge

The refusals of the start of a branch MUST merge into one edge per branch they need and kind (`domain_policy::landing_edges`):
from the root of what lands to the branch of the commit that adds the name or changes the use, or that the user
declared; the names (sorted, each once) and the files of their uses its evidence, with the count of the refusals.

#### Scenario: Five names of one branch
- **WHEN** the start of `air-home-watch` is refused for five names `acp-load-replay` adds, all used in
  `acp_load_replay.rs`
- **THEN** it has one edge `air-home-watch → acp-load-replay` of the build, its evidence the five names and that file,
  five refusals
  [@test] crates/domain-policy/src/landing_edges_test.rs (`fifteen_refusals_of_a_stack_are_one_edge_on_its_root_that_waits_for_the_running_safe_push`)

#### Scenario: A dangling reference
- **WHEN** a dangling reference of the start is changed by a commit of the branch the missing names are of
- **THEN** it joins the same edge of the build, its name and its file added to the evidence
  [@test] crates/domain-policy/src/landing_edges_test.rs (`a_dangling_reference_is_an_edge_of_the_build_that_waits_for_the_branch_changing_the_use`)

### Requirement: Edges: The edges of a stack are lifted to its root

Of every branch, the edges MUST be lifted to the root of its stack and merged with those of the other branches of it
that need the same branch, the branches each refuses kept; a need of a branch of the same stack MUST be dropped, as
the whole stack lands both.

#### Scenario: Fifteen refusals of idea-1
- **GIVEN** `hosted-exit` and `claude-folds` are built on `air-home-watch`, and the start of each is refused for the
  same five names of `acp-load-replay`
- **WHEN** the edges of every branch are lifted
- **THEN** there is one edge, `air-home-watch → acp-load-replay`, of fifteen refusals, refusing the three
  [@test] crates/domain-policy/src/landing_edges_test.rs (`fifteen_refusals_of_a_stack_are_one_edge_on_its_root_that_waits_for_the_running_safe_push`)

#### Scenario: A declared need a branch inherits
- **WHEN** `ui`, built on `auth`, inherits the need `auth` declared on `docs`
- **THEN** the stack has one declared edge `auth → docs` with the fix of the root (none), while the edge of the start of
  `ui` keeps its unstack
  [@test] crates/domain-policy/src/landing_edges_test.rs (`an_inherited_declared_need_lifts_to_the_root_with_the_fix_of_the_root`)

### Requirement: Edges: A declared need is never merged and a lost binding is no edge

A need the user declared (`brd need`) MUST be an edge of its own kind, `declared` (`companion` of a branch of a
companion), never merged with a need of the build of the same pair; a lost binding MUST be no edge, as no branch helps.

#### Scenario: A need of the build and a declared one of the same pair
- **WHEN** the start of `air-home-watch` uses a name of `acp-load-replay` and the user declared that
  `acp-load-replay` lands first, and needs `docs:guide` of a companion
- **THEN** it has a build edge and a declared edge to `acp-load-replay` and a companion edge to `docs:guide`, and its
  lost binding is none
  [@test] crates/domain-policy/src/landing_edges_test.rs (`a_declared_need_is_never_merged_with_a_need_of_the_build_and_a_lost_binding_is_no_edge`)

### Requirement: States: An edge says where the branch it needs stands

An edge MUST wait (`Waits`) while a running Safe Push lands the branch it needs, a refusal of it naming that robot
(`landing_in`, of a dangling reference too); else it MUST be `NeededFailed` while the robot of that branch failed,
else `Open`; the robots of a companion are its own.

#### Scenario: Landing, failed, open
- **WHEN** `acp-load-replay` is landing in its Safe Push, or its robot failed, or neither
- **THEN** the edge is `Waits` with that robot, `NeededFailed` with the failed one, `Open`
  [@test] crates/domain-policy/src/landing_edges_test.rs (`an_edge_waits_while_what_it_needs_is_landing_needs_a_restart_of_a_failed_robot_else_is_open`)
  [@test] crates/domain-policy/src/landing_needs_test.rs (`a_dangling_reference_names_the_safe_push_that_lands_the_branch_changing_the_use`)

### Requirement: States: A cycle starts what lands both and only an unassigned commit moves

An edge to a branch built on what lands MUST be a `Cycle` of the branch itself, its fix the start of that branch
when it is built on the branch, else of the whole stack (`LandingFix::Start`), never a move; an edge to an unassigned
commit MUST be `Unassigned`, its fix the group of the commit into the branch that needs it, the only move.

#### Scenario: A sibling and an unassigned commit
- **WHEN** `claude-folds` uses a name its sibling `hosted-exit` adds, and one an unassigned commit adds
- **THEN** its edge to `hosted-exit` is a cycle whose fix is the Safe Push of the whole `air-home-watch` stack, and its
  edge to the commit is unassigned, its fix "Group 1a2b3c4d into claude-folds"
- **AND** the stack has no edge to `hosted-exit`
  [@test] crates/domain-policy/src/landing_edges_test.rs (`a_cycle_starts_what_lands_both_and_an_unassigned_commit_is_grouped_into_the_branch_that_needs_it`, `the_fix_of_a_need_restacks_starts_what_lands_both_or_groups_an_unassigned_commit`)

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

The Safe Push popover of a refused start MUST say the first edge in a headline of one line, then a short detail
wrapped inside it: commits by their short hashes, the file by its name and line (each whole in its hover), never a
sentence a button repeats; the actions of the edge as buttons, the first focused, "Start Anyway" an `outline` button
below; Enter MUST NOT start anyway.

#### Scenario: A long path
- **WHEN** the start of `flow` needs `codec`, nothing lands it, and its use is in a long path
- **THEN** the headline is "Uses Codec of codec in FlowCodecIntegrationEntryPoint.kt", the detail "1a2b3c4d of flow
  uses Codec (FlowCodecIntegrationEntryPoint.kt:3), which 5e6f7a8b of codec adds.", its whole path in the hover of
  the file, and "Land Together" has the focus, "Land codec First" beside it
  [@test] tests/e2e/next-step/lands-after.spec.ts (`should say a need nothing lands in one headline and a short detail, focus its fix before Start Anyway and keep the popover narrow`)
  [@test] src/presenters/landing.test.ts (`says the first refusal short: the commits by their hashes, the file by its name, each whole in its hover, the fix only without its button`)

### Requirement: Popover: A start that waits starts anyway from its ⋯

While every edge of the needs waits, the popover MUST offer no start in its footer: "Start Anyway" MUST be the item
of its "⋯", as a start beside the robot that lands what it needs fails; the headline says "Lands after codec".

#### Scenario: Start Anyway from ⋯
- **WHEN** the user opens the Safe Push of `flow` while `codec` is landing and picks "Start Anyway" of its "⋯"
- **THEN** the start runs with `allowMissing`
  [@test] tests/e2e/next-step/lands-after.spec.ts (`should say on the row that a branch lands after the running Safe Push of what it needs, its slot empty and Start Anyway in the popover's ⋯`)

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
  [@test] src/presenters/landing.test.ts (`says a refusal in a short line: the commits by their short hash and the use by its file, their whole words on hover`)

### Requirement: Merge toast: The update of a merge offers the start that waited for it

The toast of the update that took the place of the toast of a merge MUST offer the start of the subject that waited
for that robot (`AfterLanding`, the one that lands the most), "Start flow": a click of its Safe Push once its step is
decided again (the needs of the tips the update moved); nothing MUST start by itself.

#### Scenario: Update Project of the merge
- **GIVEN** `flow` lands after `codec`, whose robot merges
- **WHEN** the user presses Update Project of the toast of the merge
- **THEN** the same toast says what landed and offers "Start flow", whose press starts it
  [@test] tests/e2e/next-step/lands-after.spec.ts (`should offer the start of the branch that waited in the toast of the merge once its Update Project landed it, and start it on the press`)

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
  [@test] src/presenters/landing.test.ts (`labels every action of an edge as every surface shows it`)
  [@test] src/presenters/next-step.test.ts (`row 1′: names the branch the build needs and runs the fix, or opens the Safe Push`)
  [@test] src/presenters/next-step.test.ts (`row 1′: says one edge in one sentence, restarts a failed robot of the branch it needs and groups an unassigned commit`)

## User Experience

- Status cell: "Lands after acp-load-replay", "Restart after acp-load-replay" (quiet).
- The step of row 1′: "Needs stills-codec", "Restart acp-load-replay", "Group 1a2b3c4d into claude-folds", "Safe Push
  Whole Stack", "Safe Push ui"; its hover "Uses 5 names of acp-load-replay in acp_load_replay.rs (Effect, Input +3).
  Stack air-home-watch on acp-load-replay to land it too."
- The actions of an edge: "Land Together" ("Land Together · Safe Push Whole Stack" of a stack that forks), "Land
  agent-id First", "Restart acp-load-replay", "Group 1a2b3c4d into claude-folds", "Unstack ui from auth", "Safe Push
  meter".
- The popover: the headline "Lands after codec" (info) or "Uses Codec of codec in Flow.kt" (warning), then the detail
  "1a2b3c4d uses Codec at Flow.kt:3, which 5e6f7a8b of codec adds: the build of flow fails." (subjects and the path on
  hover); the actions of the edge as `outline` buttons; "Start Anyway" in "⋯" of a wait, an `outline` button otherwise.
- The waves: "Landing now", "Next", "After acp-load-replay"; the price of Land Together: "1 robot instead of 2", "same
  7 checks", "+ AI Assistant Tests +11", "checks unknown", "a failure holds both", "approvals count in #12", the code
  owners it adds, "#12 and #13 become one GitHub stack".
- The toast of the update after a merge: "Start flow".

## References

- [Landing](../../../../docs/design/landing.md#landing-plan)
- [Next Step](../next-step/spec.md#what-the-safe-push-needs)
