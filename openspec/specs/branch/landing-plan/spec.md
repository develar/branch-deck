---
name: Landing Plan
description: Requirements for the needs of the starts of the branches as edges of a landing plan (`domain_policy::landing_edges`), merged by the branch they need, lifted to the root of a stack, each with its kind and its state; the row that names its first edge in one sentence, or waits while what its start needs is landing ("Lands after acp-load-replay"); the Safe Push popover of a refused start; the start that waited offered by the toast of the merge.
style: plain-1
targets:
  - crates/domain-policy/src/landing_edges.rs
  - crates/domain-policy/src/landing_needs.rs
  - crates/domain-types/src/landing.rs
  - crates/domain-policy/src/next_step.rs
  - crates/domain-policy/src/row_status.rs
  - crates/repository-model/src/derive.rs
  - crates/bd-core/src/next_step_summary.rs
  - src/presenters/landing.ts
  - src/presenters/next-step.ts
  - src/components/branch/SafePushPopover.tsx
  - src/lib/merge-dependents.ts
  - src/lib/landing-toast.ts
---

# Landing Plan

Status: Implemented
Date: 2026-11-24

## Purpose

Agents cut the work of one local branch into logical virtual branches, and the landing gate refuses a start once per
name it misses, so one need between two pieces reads as many refusals as names times the branches whose start lands
it. The landing plan says each need once, as an edge from the piece that waits to the piece it needs, with what it
waits for: a running Safe Push, a restart, a fix, or the start of what lands both. A row then says one short state,
and a wait is shown as a wait, never as the user's step.

## Goals

- One edge per need of a pair, its names as evidence; one edge of a stack on its root.
- A start whose every need is landing waits quietly ("Lands after acp-load-replay"), and the toast of that merge offers
  the start afterwards.
- A commit of a branch stays in its branch: only an unassigned commit moves.

## Non-goals

- No start by itself after the merge it waited for; the user presses it.
- No landing together of a declared need: the need is what keeps the two apart.
- The waves of the plan, the offers to land together with their cost, `brd plan` and the picture come later.

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

## User Experience

- Status cell: "Lands after acp-load-replay", "Restart after acp-load-replay" (quiet).
- The step of row 1′: "Needs stills-codec", "Restart acp-load-replay", "Group 1a2b3c4d into claude-folds", "Safe Push
  Whole Stack", "Safe Push ui"; its hover "Uses 5 names of acp-load-replay in acp_load_replay.rs (Effect, Input +3).
  Stack air-home-watch on acp-load-replay to land it too."
- The popover: the headline "Lands after codec" (info) or "Uses Codec of codec in Flow.kt" (warning), then the detail;
  "Start Anyway" in "⋯" of a wait, an `outline` button otherwise.
- The toast of the update after a merge: "Start flow".

## References

- [Landing](../../../../docs/design/landing.md#landing-plan)
- [Next Step](../next-step/spec.md#what-the-safe-push-needs)
