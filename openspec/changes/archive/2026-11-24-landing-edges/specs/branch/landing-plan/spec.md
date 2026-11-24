## ADDED Requirements

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
