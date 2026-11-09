---
name: Landing Overview
description: Requirements for the header button of the work on its way into master and its overview: the units of the table (a lone branch, a stack) by their landing nodes, the moves that are the user's (their count, the node of the most urgent one, a bar of the way), the next step of each run from the overview at its row, the robots that land as a queue with their time and actions, what waits for others, what is in master until the update, the robots of the last day in one line that opens Activity, and the footer of the update state, the pull requests and Patronus; it fits the window.
style: plain-1
targets:
  - src/components/repository/LandingIndicator.tsx
  - src/components/repository/LandingPopover.tsx
  - src/components/repository/RobotRows.tsx
  - src/components/repository/ConfigurationHeader.tsx
  - src/lib/activity-window.ts
  - src/hooks/useLandingOverview.ts
  - src/hooks/useLandingNode.ts
  - crates/domain-policy/src/landing_node.rs
  - crates/domain-policy/src/next_step.rs
  - crates/domain-policy/src/attention.rs
  - src/hooks/useRobotOverview.ts
  - src/lib/landing-overview.ts
  - src/presenters/landing-node.ts
  - src/lib/robot-overview.ts
  - src/lib/robot-run.ts
  - src/presenters/actions.ts
  - src/hooks/useRobotDetails.ts
  - src/lib/check-counts.ts
  - src/components/repository/UpdateProjectButton.tsx
  - src/components/repository/PullRequestStatusLine.tsx
  - src/components/repository/PatronusStatusLine.tsx
---

# Landing Overview

Status: Implemented
Date: 2026-10-18

## Purpose

The work of the branch is done when it is in master ([Landing](../../../../docs/design/landing.md)). The header says where
it is on that way, in the words of the node of each row ([Branch Row](../../branch/branch-row/spec.md#tree)): the node of
the most urgent move that is the user's, how many there are, a bar of the work from landing to waiting to the user's
moves. Hovering it opens the overview, which tells only the present: the user's moves with the next step of each, the
robots that land as a queue (the nearest to finishing first), what waits for others, what is in master until the
update; the robots of the last day are one line, their history is [Activity](../activity/spec.md). As IntelliJ's
background tasks show only what runs, and Chrome's downloads the latest with the rest on a page, the overview stays
short and fits the window. A push is not a move: the steps push what they
need ([Next Step](../../branch/next-step/spec.md#goals)). It replaces the indicator of the commits to push and the robots
indicator, whose states were the same story told twice.

## Goals

- Answer at a glance whether something is the user's move, and how urgent the most urgent one is.
- Act on a move from wherever the table is scrolled: the step runs at its row, as the slot of the row would run it.
- Count what lands, not branches: a stack lands in one robot, so it is one move.

## Non-goals

- A "Push all": a push stays per branch and per stack ([Outgoing Commits](../outgoing/spec.md#push)), and no push is
  offered here.
- Asking Patronus or GitHub for anything of its own: the overview shows the answers the rows have; while it is open it is
  a surface of the readiness of its rows ([Next Step](../../branch/next-step/spec.md#the-surfaces)).

## Requirements

### Requirement: Units: A unit is a lone branch or a stack, with the node of the whole stack

A unit is what lands in one move: a lone branch, or a stack (its root's row). The node and the next step of a stack
MUST be those of the whole stack, as the header of its island shows them (`getStackLandingNodeNow`,
`useStackNextStep`: of its stack branch while the stack forks, `nextSteps` of the view under `stack:<root>`, else of
the last branch of its straight chain).

#### Scenario: A stack is one unit, its root's row with the step of the whole stack
- **GIVEN** a stack of several branches and a lone branch
- **WHEN** the overview lists the user's moves
- **THEN** the stack is one unit, its root's row ("4 branches · 4 commits"), and no row of its other branches
- **AND** a ready stack of 22 branches counts one move
- **AND** the step of its row is the step of the whole stack
  [@test] src/lib/landing-overview.test.ts (`counts the user's moves by unit, a ready stack of 22 branches one move, the most urgent first`)
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should run the next step of a move from the overview at its row: a Safe Push, a pull request`)

#### Scenario: A stack that forks takes the step of its stack branch
- **GIVEN** a stack that forks
- **WHEN** the view carries the next steps
- **THEN** the unit takes the step of its stack branch, `nextSteps` of the view under `stack:<root>`
  [@test] crates/repository-model/src/derive_test.rs (`the_view_carries_the_next_step_and_the_node_of_each_subject_and_a_patch_only_those_that_changed`)

#### Scenario: A straight chain takes the step of its last branch
- **GIVEN** a stack that is a straight chain
- **WHEN** the overview shows its unit
- **THEN** the node and the step are those of the last branch of the chain

### Requirement: Units: Rust decides every node and step, the overview counts their kinds

Rust MUST decide every node and step (`domain_policy::landing_node`, `domain_policy::next_step`). The overview MUST
count their kinds as the rows show them (`getNodeKindNow`) and SHALL word only the nodes it lists.

#### Scenario: The view carries the node and the step of each subject
- **WHEN** the session derives its view
- **THEN** the view carries the next step and the node of each subject, as Rust decided them
- **AND** the overview counts their kinds as the rows show them
  [@test] crates/repository-model/src/derive_test.rs (`the_view_carries_the_next_step_and_the_node_of_each_subject_and_a_patch_only_those_that_changed`)

### Requirement: Units: A branch of a stack that needs a fix is a move of its own

The branches of a stack whose own node needs a fix MUST be listed below it, each a move of its own.

#### Scenario: A stack with a branch that needs a fix
- **GIVEN** a waiting stack whose branch needs a fix
- **WHEN** the overview collects the moves
- **THEN** the stack is listed with the moves, the branch below it
- **AND** the fix counts as a move of its own
  [@test] src/lib/landing-overview.test.ts (`lists a stack whose branch needs a fix with the moves, the fix counted as a move of its own`)

### Requirement: Units: Units come in the order of the table

The units MUST come in the order of the table.

#### Scenario: The order of the units
- **WHEN** the overview lists the units
- **THEN** they come in the order of the table

### Requirement: Units: A parked branch is in no unit

A parked branch, and one parked with its parent, MUST be in no unit ([Parked Branches](../../branch/parked-branches/spec.md)).

#### Scenario: A parked branch
- **GIVEN** a parked branch, or one parked with its parent
- **WHEN** the overview collects the units
- **THEN** that branch is in no unit

### Requirement: Units: A whole stack that holds a parked branch takes the node of its root

A whole stack that holds a parked branch lands in pieces, so its unit MUST take the node of its root.

#### Scenario: A stack that holds a parked branch
- **GIVEN** a whole stack that holds a parked branch
- **WHEN** the overview shows its unit
- **THEN** the unit takes the node of its root

### Requirement: Units: A merge of the pull request on GitHub is the user's move

Where the repository lands by a merge of the pull request on GitHub, Merge #7 and Auto-Merge #7 MUST be the user's move
(the node ready), and a unit whose auto-merge is enabled MUST be landing ([Landing Routes](../../branch/landing-routes/spec.md#merge)).

#### Scenario: Merge and Auto-Merge are offered
- **GIVEN** a repository that lands by a merge of the pull request on GitHub
- **WHEN** Merge #7 or Auto-Merge #7 is the step of a unit
- **THEN** it is the user's move, the node ready

#### Scenario: An auto-merge is enabled
- **GIVEN** a repository that lands by a merge of the pull request on GitHub
- **WHEN** the auto-merge of a unit is enabled
- **THEN** its node is landing, no move of the user

### Requirement: Indicator: The button shows while work is on its way or a robot ran

The header MUST show the button after Update Project while a unit is on its way to master or in master, or a robot
ran in the last day, and MUST hide it otherwise.

#### Scenario: Work on its way to master
- **GIVEN** units on their way to master and one in master
- **WHEN** the header shows
- **THEN** the button is there after Update Project
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should count the user's moves, put the most urgent node in the header and group the work by where it is`)

#### Scenario: A pushed branch still waits for its pull request
- **GIVEN** a lone branch whose step is its push, "On the way to master: 1 move is yours"
- **WHEN** the user pushes it and it waits for its pull request
- **THEN** the button stays, "On the way to master: 1 waiting"
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should be hidden while nothing is on the way to master and nothing ran in the last day`)

#### Scenario: Nothing on the way, nothing in master, no robot of the last day
- **WHEN** nothing is on the way to master, nothing is in master and no robot ran in the last day
- **THEN** the overview is empty ("Nothing on the way to master") and the button is hidden
  [@test] src/lib/landing-overview.test.ts (`is empty with nothing on the way, nothing in master and no robot of the last day`)

### Requirement: Indicator: Its icon is the node of the most urgent move

Its icon MUST be the node of the most urgent move (`needsFix`, then `yourStep`, then `ready`; a hollow neutral node
without one), in the ring of a running Safe Push.

#### Scenario: A fix among the moves while a Safe Push runs
- **GIVEN** a branch that needs a fix, the user's steps and a running Safe Push
- **WHEN** the header shows the button
- **THEN** its icon is the node `needsFix`, in the ring of the running Safe Push
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should count the user's moves, put the most urgent node in the header and group the work by where it is`)

#### Scenario: No move of the user
- **WHEN** no unit is the user's move
- **THEN** the icon is a hollow neutral node

### Requirement: Indicator: The number of the user's moves keeps its place

After the icon the button MUST show the number of the user's moves (none at 0, its place kept: one digit wide), then
the bar.

#### Scenario: The user has moves
- **WHEN** 5 units are the user's moves
- **THEN** the button shows "5" after its icon, then the bar
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should count the user's moves, put the most urgent node in the header and group the work by where it is`)

#### Scenario: No move
- **WHEN** the user has no move
- **THEN** no number is shown and its place is kept, one digit wide

### Requirement: Indicator: The button is as wide whatever the answers say

The button MUST be as wide whatever the answers say, so an answer of Patronus or GitHub after the sync moves no
control of the header (at the minimum width a count that came moved Update Project and the start of the row by 12 px).

#### Scenario: Patronus answers after the sync
- **GIVEN** the window at its minimum width and the answer of Patronus held after the sync
- **WHEN** Patronus answers and a move is counted
- **THEN** the count comes in the place kept for it and no control of the header moves
  [@test] tests/e2e/repository/header.spec.ts (`should keep its controls in place when Patronus answers after the sync`)

### Requirement: Indicator: The button holds its place while the rows are to come

While the rows of a repository synced before are still to come, the button MUST hold its place, disabled
(`landing-indicator-loading`).

#### Scenario: A repository synced before opens
- **GIVEN** a repository synced before
- **WHEN** its rows are still to come
- **THEN** the button holds its place, disabled (`landing-indicator-loading`)

### Requirement: Indicator: A Push to master that can run is the user's move

In a repository that lands by a push into the target the node of a unit whose Push to master can run MUST be ready, so
the header counts it as the user's move as it counts a ready Safe Push ([Landing Routes](../../branch/landing-routes/spec.md#push-to-master)).

#### Scenario: Push to master can run
- **GIVEN** a repository that lands by a push into the target
- **WHEN** the Push to master of a unit can run
- **THEN** its node is ready and the header counts it as the user's move
  [@test] crates/domain-policy/src/landing_node_test.rs (`a_push_into_the_target_is_ready_and_a_refusal_of_its_gate_waits`)
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should push the copy into master from the slot of the row, the popover asking what CI did not pass, and update after it`)

### Requirement: Indicator: The moves are the filled units, their fixes and the robots to act on

The moves MUST be the units whose node is filled, their branches that need a fix, the failed robots of no row (a
temporary ref, a watched pull request) and the running Safe Pushes whose required check failed.

#### Scenario: A failed robot of no row and a running Safe Push that will fail
- **GIVEN** a failed robot of no row, a running Safe Push whose required check failed and a Dry Run
- **WHEN** the overview counts the moves
- **THEN** the failed robot and the failing Safe Push are moves; the Dry Run lands nothing
  [@test] src/lib/landing-overview.test.ts (`counts a failed robot of no row and a running Safe Push that will fail as moves, listed with them; a Dry Run lands nothing`)
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should count a running Safe Push whose required check failed as a move, show it failing and cancel it from the toast`)

### Requirement: Indicator: A unit that waits for the first answer of Patronus counts no move

A unit that waits for the first answer of Patronus MUST count no move, so the number does not change when the overview
opens and asks.

#### Scenario: Before the first answer of Patronus
- **GIVEN** a unit whose node waits for the first answer of Patronus
- **WHEN** the moves are counted, and the overview opens and asks
- **THEN** the unit counts no move and the number does not change
  [@test] src/lib/landing-overview.test.ts (`counts no move of a unit before the first answer of Patronus, so the number doesn't change on hover`)
  [@test] crates/domain-policy/src/landing_node_test.rs (`waits_for_the_first_answers_as_the_step_does_and_says_a_read_that_failed`)

### Requirement: Indicator: A unit whose move runs counts no move

A unit whose move runs (a start or restart of its Safe Push, the creation of its pull request, a review request) MUST
count no move either, so the number drops at the click, not when Patronus or GitHub answer.

#### Scenario: The user starts a move
- **WHEN** the user starts the move of a unit: a start or restart of its Safe Push, the creation of its pull request, a review request
- **THEN** from its start on the unit counts no move, Landing for a robot that comes, else Waiting
- **AND** the number drops at the click, not when Patronus or GitHub answer
  [@test] src/presenters/landing-node.test.ts (`is the state after a running move of the row from its start on: Landing for a robot that comes, else Waiting`, `counts the kind a row shows without its words: the move that runs and the push from its click laid over the node`)

### Requirement: Indicator: The bar has a segment per kind on the way

The bar MUST have a segment per kind of the units on their way, in the order of the way: landing, waiting, ready, the
user's step, a fix; its width the commits of its units; light for a hollow node.

#### Scenario: Units of several kinds
- **GIVEN** units landing, waiting, ready, with a fix and in master
- **WHEN** the bar is drawn
- **THEN** it has a segment per kind on the way, landing first and the user's moves last, the width of each the commits of its units
  [@test] src/lib/landing-overview.test.ts (`has a segment per kind on the way, landing first and the user's moves last, its weight the commits`)

#### Scenario: A hollow node
- **WHEN** the units of a segment have a hollow node
- **THEN** the segment is light

### Requirement: Indicator: The bar is there either way

A single kind MUST fill the bar, and with nothing on the way the bar MUST be empty: it is there either way, so a kind
that comes moves nothing.

#### Scenario: A single kind
- **WHEN** the units on the way are all of one kind
- **THEN** its segment fills the bar

#### Scenario: Nothing on the way
- **WHEN** nothing is on the way
- **THEN** the bar is there, empty, and a kind that comes moves nothing

### Requirement: Indicator: Its accessible name is the title of the overview

The accessible name of the button MUST be the title of the overview, the parts that apply: "On the way to master: 3
moves are yours · 1 landing · 3 waiting · 1 in master"; "landing" MUST count the Safe Pushes that run and will not fail.

#### Scenario: The parts that apply
- **GIVEN** moves of the user, units waiting and one in master
- **WHEN** the title is worded
- **THEN** it names the parts that apply: "On the way to master: 4 moves are yours · 1 waiting · 1 in master"
  [@test] src/lib/landing-overview.test.ts (`counts the user's moves by unit, a ready stack of 22 branches one move, the most urgent first`)

### Requirement: Indicator: The button shows the number alone

The button MUST show the number alone: a word beside it would say "move" with another number than the Your Move chip
of the table, which counts the branches.

#### Scenario: The label of the button
- **WHEN** the button shows the user's moves
- **THEN** it shows the number without a word beside it

### Requirement: Indicator: A line under the title says what a move counts

While a stack is among the moves, the line under the title MUST say what a move counts: "A stack counts as one move:
it lands in one robot. A branch of it that needs a fix counts on its own." (`formatMoveUnit`).

#### Scenario: A stack among the moves
- **WHEN** a stack is among the moves
- **THEN** the line under the title says "A stack counts as one move: it lands in one robot. A branch of it that needs a fix counts on its own."
  [@test] src/lib/landing-overview.test.ts (`counts the user's moves by unit, a ready stack of 22 branches one move, the most urgent first`)

### Requirement: Overview: Hovering or a click opens the overview

Hovering the button (after a short delay) or a click MUST open the overview; it SHALL be loaded while the delay runs.

#### Scenario: The pointer rests on the button
- **WHEN** the pointer hovers the button for a short delay
- **THEN** the overview opens, loaded while the delay ran
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should count the user's moves, put the most urgent node in the header and group the work by where it is`)

#### Scenario: A click on the button
- **WHEN** the user clicks the button
- **THEN** the overview opens

### Requirement: Overview: Its sections are hidden while empty and titled with their count

The sections of the overview (Your move, Landing, Waiting, In master) MUST each be hidden while empty and titled with
their count.

#### Scenario: A section is titled with its count
- **GIVEN** two running Safe Pushes and a running Dry Run
- **WHEN** the overview opens
- **THEN** their section is titled with its count, "Landing · 2 · 1 dry run"
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should queue the robots nearest to finishing first, and fit the window`)

#### Scenario: An empty section is hidden
- **GIVEN** no robot that lands, or no move of the user
- **WHEN** the overview opens
- **THEN** the section of Landing, or of Your move, is hidden
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should count a running Safe Push whose required check failed as a move, show it failing and cancel it from the toast`, `should wait for the Safe Push of the root instead of the failure of the whole straight stack, once it runs`)

### Requirement: Overview: Your move lists the fixes, then the user's step, then the ready units

**Your move** MUST list the units with a fix first, then the user's step, then the ready ones.

#### Scenario: Units of every kind of move
- **GIVEN** units that need a fix, that are the user's step and that are ready
- **WHEN** the overview lists Your move
- **THEN** the units with a fix come first, then the user's step, then the ready ones
  [@test] src/lib/landing-overview.test.ts (`counts the user's moves by unit, a ready stack of 22 branches one move, the most urgent first`)
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should count the user's moves, put the most urgent node in the header and group the work by where it is`)

### Requirement: Overview: A branch whose robot needs the user is the robot's row

In **Your move**, a branch whose robot needs the user (it failed on the branch as it is and nothing else holds it, the
view's `attention`, [Patronus](../../branch/patronus/spec.md#landing-overview)) MUST be the robot's row, with its
actions.

#### Scenario: The view names the robots that need the user
- **GIVEN** robots that failed on their branch as it is, one on a commit the branch no longer is at, one of a parked branch, one held by the Safe Push of another branch
- **WHEN** the session derives the view
- **THEN** its `attention` names only the robots that failed on the branch as it is and nothing else holds
  [@test] crates/repository-model/src/derive_test.rs (`the_view_names_the_robots_that_need_the_user_over_the_rows_as_they_are`)

#### Scenario: The failed robot is the row of its branch
- **GIVEN** a branch whose robot needs the user, as the view names it
- **WHEN** the overview lists Your move
- **THEN** the branch is the robot's row, with its actions, and no second row of the branch
  [@test] src/lib/robot-overview.test.ts (`lists the running robots of the branches in the table order, and the branches whose robot needs the user as the view names them`)
  [@test] tests/e2e/patronus/patronus.spec.ts (`should show a robot that failed on the branch tip once, as the move it is, with the chips of the other rows`)

### Requirement: Overview: The failed robots of no row and the robots that will fail are moves

After the units, **Your move** MUST list the failed robots of no row and the running robots that will fail (a required
check failed: no use waiting, the user cancels or restarts them), each a move.

#### Scenario: A running Safe Push whose required check failed
- **GIVEN** a running Safe Push whose required check failed
- **WHEN** the overview opens
- **THEN** its robot is in Your move, not in Landing, a move the user cancels or restarts
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should count a running Safe Push whose required check failed as a move, show it failing and cancel it from the toast`)
  [@test] src/lib/landing-overview.test.ts (`counts a failed robot of no row and a running Safe Push that will fail as moves, listed with them; a Dry Run lands nothing`)

### Requirement: Overview: Landing queues the running robots nearest to finishing first

**Landing** ("Landing · 5", "Landing · 5 · 1 dry run") MUST list the other running robots of the branches, the whole
stacks and the watched pull requests, as a queue: the Safe Pushes nearest to finishing first, by the time left as the
Dock picks its robot (`compareByFinish`; one without an estimate last), then the Dry Runs, which land nothing, the same
way.

#### Scenario: Safe Pushes and a Dry Run run
- **GIVEN** two running Safe Pushes, one with 10 minutes left and one with 22, and a running Dry Run
- **WHEN** the overview opens
- **THEN** Landing is titled "Landing · 2 · 1 dry run"
- **AND** the Safe Push nearest to finishing comes first, then the other, then the Dry Run
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should queue the robots nearest to finishing first, and fit the window`)

#### Scenario: A Safe Push without an estimate
- **GIVEN** running Safe Pushes, one without an estimate, and Dry Runs
- **WHEN** the robots are queued
- **THEN** those without an estimate come after the Safe Pushes that have one, then the Dry Runs, ordered the same way
  [@test] src/lib/robot-overview.test.ts (`queues the Safe Pushes nearest to finishing first, those without an estimate last, then the Dry Runs`)

### Requirement: Overview: The order of Landing is kept while the overview is open

The order of **Landing** MUST be taken when the overview opens and kept while it is open, so a row never jumps under
the pointer; a robot started since MUST come last.

#### Scenario: The time left changes while the overview is open
- **GIVEN** the overview is open
- **WHEN** the time left of the robots changes
- **THEN** the rows keep their order

#### Scenario: A robot starts while the overview is open
- **GIVEN** the overview is open
- **WHEN** a robot starts
- **THEN** it comes last

### Requirement: Overview: Waiting lists the units that wait for others

**Waiting** MUST list the units that wait for others.

#### Scenario: A unit waits for others
- **WHEN** a unit waits for others
- **THEN** it is listed in Waiting
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should wait for the Safe Push of the root instead of the failure of the whole straight stack, once it runs`)

### Requirement: Overview: A stack waits for the Safe Push of its branch instead of a restart

A stack whose last robot failed while a Safe Push of a branch of it lands a part of it MUST wait for that robot ("A
Safe Push of auth is running: wait until it merges"), with no restart: two robots would land the same commits.

#### Scenario: The root of a failed stack starts its Safe Push
- **GIVEN** the robot of the whole straight stack failed on it as it is, its restart the step
- **WHEN** a Safe Push of its root runs, landing a part of the stack
- **THEN** the stack is in Waiting, "A Safe Push of IJPL-257337 is running: wait until it merges", not in Your move
- **AND** its step is no restart
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should wait for the Safe Push of the root instead of the failure of the whole straight stack, once it runs`)

### Requirement: Overview: In master lists the units until the update archives them

**In master** MUST list the units the baseline or a merge has, until the update archives them.

#### Scenario: A branch merged by Patronus
- **GIVEN** a branch merged by Patronus, the update not run
- **WHEN** the overview opens
- **THEN** the branch is in In master
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should count the user's moves, put the most urgent node in the header and group the work by where it is`)

### Requirement: Overview: The robots of the last day are one line

The robots of the last day MUST be one line below the sections, not a list: "auth, login, oauth and 6 more merged · ui
failed · 1 canceled · 2 dry runs passed in the last 24 hours" (`formatFinishedSummary`, the parts that apply; the
merged and the failed named, so a glance answers what landed today), the newest robot of each branch.

#### Scenario: Robots finished in the last day
- **GIVEN** robots that merged, failed, were canceled and Dry Runs that passed in the last day
- **WHEN** the line is worded
- **THEN** it names what merged and what failed, and counts the canceled and the passed Dry Runs, the parts that apply
  [@test] src/lib/landing-overview.test.ts (`names what merged and what failed, counts the canceled and the passed Dry Runs, nothing without a robot`)
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should count the user's moves, put the most urgent node in the header and group the work by where it is`)

#### Scenario: No robot of the last day
- **WHEN** no robot finished in the last day
- **THEN** the line names no robot
  [@test] src/lib/landing-overview.test.ts (`names what merged and what failed, counts the canceled and the passed Dry Runs, nothing without a robot`)

### Requirement: Overview: The line of the last day leaves out the units in master

The line of the last day MUST leave out the robots of the units in master (the In master row says it).

#### Scenario: A branch in master merged in the last day
- **GIVEN** a unit in master whose robot merged in the last day
- **WHEN** the line of the last day is collected
- **THEN** that robot is not in it
  [@test] src/lib/landing-overview.test.ts (`leaves out of the last day the robots of what is in master: the In master row says it`)

### Requirement: Overview: Show in Activity opens Activity at its Robots

"Show in Activity" MUST close the overview and open Activity at its Robots (its side panel, or its window where the
panel has no room), the history of the robots the app told about.

#### Scenario: Show in Activity
- **WHEN** the user clicks "Show in Activity" on the line of the last day
- **THEN** the overview closes and Activity opens at its Robots, its side panel where it has room
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should count the user's moves, put the most urgent node in the header and group the work by where it is`)

#### Scenario: The panel has no room
- **WHEN** the user clicks "Show in Activity" and the side panel has no room
- **THEN** the window of Activity opens at its Robots

### Requirement: Overview: Show Statistics opens the Landing view

"Show Statistics", on the line of the last day also without a robot of the last day, MUST close the overview and show
the Landing view: how the Safe Pushes went ([Main Views](../main-views/spec.md)).

#### Scenario: Show Statistics
- **WHEN** the user clicks "Show Statistics", with or without a robot of the last day
- **THEN** the overview closes and the Landing view shows
  [@test] tests/e2e/landing/statistics.spec.ts (`should open from the native View menu and Show Statistics, and give the branches back as they were`)

### Requirement: Overview: A row of a unit shows its node, name, size, reason, chip and step

A row of a unit MUST have its node, its name, its size ("1 commit", "5 branches · 37 commits"), the first line of its
node's hover, its pull request chip and the next step of its row (`StepButton`, the item of `presentNextStep`).

#### Scenario: The row of a unit in master
- **GIVEN** a branch merged by Patronus
- **WHEN** the overview lists it in In master
- **THEN** its row has its node, its name and, as its reason, the first line of the hover of its node, "Merged by Patronus"
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should count the user's moves, put the most urgent node in the header and group the work by where it is`)

#### Scenario: The pull request chips of the rows
- **GIVEN** units with a pull request and without one
- **WHEN** the overview lists them
- **THEN** a row has the pull request chip of its row, and a row without a robot has no Patronus chip
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should show the pull request chips of the rows, and a pull request that waits for its author as a fix`)
  [@test] tests/e2e/patronus/patronus.spec.ts (`should show a robot that failed on the branch tip once, as the move it is, with the chips of the other rows`)

### Requirement: Overview: The hover of a row's reason is the hover of its node

On hover the reason of a row MUST show the hover of its node itself, `LandingNodeHover` of
[Branch Row](../../branch/branch-row/spec.md#tree): the issue keys as links, "Show auth" of a parent it names, which
closes the overview, the robot whose state it tells.

#### Scenario: Hovering the reason
- **WHEN** the user hovers the reason of a row
- **THEN** the hover of its node shows, the state in full
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should count the user's moves, put the most urgent node in the header and group the work by where it is`)

#### Scenario: Show the parent from the hover
- **GIVEN** the hover of a node names a parent
- **WHEN** the user clicks "Show auth" in it
- **THEN** the overview closes

### Requirement: Overview: A click on the name of a unit shows its row

A click on the name of a unit, or on the branch of a robot row that has a row, MUST close the overview and show that
row; the name of a stack's unit shows it expanded.

#### Scenario: The branch of a robot row
- **GIVEN** a running Safe Push of JIRA-456, listed in Landing by its robot row
- **WHEN** the user clicks JIRA-456 in that row
- **THEN** the overview closes and the row of JIRA-456 shows, open
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should count the user's moves, put the most urgent node in the header and group the work by where it is`)

#### Scenario: A row the filter hides
- **GIVEN** a filter that hides the row of a unit
- **WHEN** the user clicks the name of the unit in the overview
- **THEN** the row shows
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should show every branch again for a reveal of a hidden one`)

#### Scenario: A collapsed stack
- **GIVEN** a collapsed stack
- **WHEN** the user clicks the name of its unit, its root's
- **THEN** the row of the root shows, the stack expanded
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should show a match deep in a collapsed stack, and expand the stack for a reveal`)

### Requirement: Overview: A click on the step runs it at the row of its owner

A click on the step MUST close the overview, show the row of its owner (`requestReveal`: a collapsed stack expands, a
filter that hides it clears) and run the step there, so its popovers open at the slot of the row: the start of a Safe
Push (at once when it has nothing to ask), Create Pull Request…, Request Review… (its popover at the slot), Retarget,
Restart Safe Push.

#### Scenario: A Safe Push with nothing to ask
- **GIVEN** a ready branch whose Safe Push has nothing to ask
- **WHEN** the user clicks its step in the overview
- **THEN** the overview closes, the row shows and the Safe Push starts at once
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should run the next step of a move from the overview at its row: a Safe Push, a pull request`)

#### Scenario: A step with a popover
- **GIVEN** a stack whose step is Create Pull Request…
- **WHEN** the user clicks its step in the overview
- **THEN** its popover opens at the slot of the row
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should run the next step of a move from the overview at its row: a Safe Push, a pull request`)

#### Scenario: The other steps
- **WHEN** the user clicks Request Review…, Retarget or Restart Safe Push in the overview
- **THEN** the step runs at the row of its owner, Request Review… with its popover at the slot

### Requirement: Overview: A robot row is two lines with its state in a column

A robot row MUST be two lines, as a background task: the icon of the state (the ring while it runs), the branch, then
its state in a right-aligned column of tabular figures: for a running robot its time ("18m · ~1h 32m",
[Patronus](../../branch/patronus/spec.md#timing)), for a finished one the state in its hue and when it finished
("Failed · 50 minutes ago").

#### Scenario: A running robot and a failed one
- **GIVEN** a running Safe Push and a failed one
- **WHEN** the overview lists their rows
- **THEN** the running one says its time ("18m · ~22m") and the failed one its state and when it finished ("Failed · 50 minutes ago")
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should count the user's moves, put the most urgent node in the header and group the work by where it is`)

### Requirement: Overview: The mode of a robot is written only for a Dry Run

The mode and the merge profile of a robot MUST show on hover ("Safe Push · Large Scale Change"); the mode MUST be
written only for a Dry Run ("Dry Run · 15m · ~20m", a link that opens it in Patronus): a Safe Push is what lands.

#### Scenario: A Dry Run
- **WHEN** a Dry Run runs
- **THEN** its state starts with its mode, "Dry Run · 35m", a link that opens it in Patronus
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should queue the robots nearest to finishing first, and fit the window`)

#### Scenario: A Safe Push
- **WHEN** the row is of a Safe Push
- **THEN** its mode is not written
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should restart a failed robot from the overview, and open the pull request it was started from`)

#### Scenario: Hovering the state
- **WHEN** the user hovers the state of a robot
- **THEN** the mode and the merge profile show ("Safe Push · Large Scale Change")

### Requirement: Overview: The actions of a robot are icon buttons named after it

A robot row MUST then have the actions of the robot as every surface lists them (`getRobotItems`, as Rust offers them,
[Patronus](../../branch/patronus/spec.md#restart-and-cancel)): icon buttons named after the robot ("Cancel Safe Push of
JIRA-456", "Open Safe Push of JIRA-456 in Patronus") with the name of the action as the tooltip, a disabled one adding
why.

#### Scenario: Cancel a running robot
- **WHEN** the user clicks "Cancel Safe Push of JIRA-456" in the overview
- **THEN** the cancel reaches Patronus and the row says "Canceling"
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should cancel a running robot from the overview`)

#### Scenario: The actions of a failed robot
- **GIVEN** a failed robot started from a pull request
- **WHEN** the overview lists its row
- **THEN** its actions are those of every surface, in their order and under their names
- **AND** they open the pull request and the robot in Patronus, and restart it
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should restart a failed robot from the overview, and open the pull request it was started from`)

#### Scenario: A disabled action
- **WHEN** an action of the robot is disabled
- **THEN** its tooltip adds why

### Requirement: Overview: The second line of a running robot is its progress

The second line of a running robot MUST be its progress bar beside the check it waits for.

#### Scenario: A running robot
- **WHEN** a robot runs
- **THEN** its second line is its progress bar beside the check it waits for
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should count the user's moves, put the most urgent node in the header and group the work by where it is`)

### Requirement: Overview: A running row asks for the details of its robot while open

While the overview is open, a running row MUST ask for the details of its robot, as its popover would: the check it
waits for and the queued checks in a column of their own (kept while they load, so the bar doesn't jump; a long name
cut, the whole on hover).

#### Scenario: The details while the overview is open
- **WHEN** the overview is open with a running robot
- **THEN** its row asks for the details of the robot, and the check it waits for and the queued checks show in a column of their own
  [@test] tests/e2e/patronus/patronus.spec.ts (`should read the estimate of the CI only while the details of a running robot are open`)

#### Scenario: The details load again
- **WHEN** the details of the robot load again
- **THEN** the checks shown are kept while they load, so the bar doesn't jump

#### Scenario: A long name of a check
- **WHEN** the name of a check is long
- **THEN** it is cut, the whole on hover

### Requirement: Overview: The checks that failed already are listed below

The checks that failed already MUST be listed below ("Failed: Bazel Community Build Aggregator", "and 2 more" past
three, four of four named: the backend names one more than three, `list_cut::kept`).

#### Scenario: A check failed
- **WHEN** a required check of a running robot failed
- **THEN** its row says "Failed: Bazel Community Build Aggregator"
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should count a running Safe Push whose required check failed as a move, show it failing and cancel it from the toast`)
  [@test] src/lib/check-counts.test.ts (`names the checks that failed, counts the ones beyond the first three, nothing while none failed`)

#### Scenario: More than three checks failed
- **WHEN** more checks failed than three
- **THEN** the first three are named and the others counted ("and 2 more"), and four of four are named
  [@test] src/lib/check-counts.test.ts (`names the checks that failed, counts the ones beyond the first three, nothing while none failed`)

### Requirement: Overview: A failing robot has a destructive ring and bar

A failing robot MUST have a `destructive` ring and bar, and "18m · Failing" instead of the time left.

#### Scenario: A running Safe Push whose required check failed
- **WHEN** a required check of a running Safe Push failed
- **THEN** its ring and bar are `destructive` and its state says "18m · Failing" instead of the time left
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should count a running Safe Push whose required check failed as a move, show it failing and cancel it from the toast`)

### Requirement: Overview: A watched robot of no branch is listed by what it tests

A watched robot of no branch MUST be listed by what it tests ("#101 Rework the session store"); a branch without a row
and a watched robot MUST be plain text.

#### Scenario: The robot of a reviewed pull request
- **GIVEN** a watched robot of a reviewed pull request, of no branch
- **WHEN** the overview lists it
- **THEN** it is listed by what it tests, as plain text
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should watch the robot of a reviewed pull request: listed by what it tests, its merge told`)

#### Scenario: A branch without a row
- **WHEN** a robot is of a branch without a row
- **THEN** its branch is plain text

### Requirement: Overview: Each row is a surface of the readiness of its branch

While the overview is open, each of its rows MUST be a surface of the readiness of its branch
(`useShowReviewReadiness`), so the units of a collapsed stack get their answer too.

#### Scenario: A collapsed stack
- **GIVEN** a collapsed stack
- **WHEN** the overview opens
- **THEN** its units get their answer of readiness too

### Requirement: Overview: A line under the title says which branches wait for an answer

While some rows wait for an answer of Patronus or GitHub
([Facts without an answer](../../branch/next-step/spec.md#facts-without-an-answer)), a line under the title MUST say
"Checking 5 branches: Patronus and GitHub answer…".

#### Scenario: Branches wait for an answer
- **WHEN** 5 branches of the overview wait for an answer of Patronus or GitHub
- **THEN** the line under the title says "Checking 5 branches: Patronus and GitHub answer…"

### Requirement: Overview: The footer tells the update, the pull requests and Patronus

Below a rule, the footer MUST show the update state of the baseline with Update
([Update Project](../update-project/spec.md)), the pull requests checked with Refresh
([Pull Requests](../../branch/pull-requests/spec.md)), Patronus checked with Refresh
([Patronus](../../branch/patronus/spec.md)).

#### Scenario: The update state
- **WHEN** the overview opens
- **THEN** its footer shows the update state of the baseline with Update
  [@test] tests/e2e/update-project/update-project.spec.ts (`should show the update state in the footer of the landing overview`)

#### Scenario: The pull requests
- **WHEN** the overview opens
- **THEN** its footer says how the pull requests were checked, with its action
  [@test] tests/e2e/github-sign-in/github-sign-in.spec.ts (`should sign in from the hint of a repository without a token, and ask GitHub again`)

#### Scenario: Refresh of the pull requests
- **WHEN** the user clicks Refresh of the pull requests in the footer
- **THEN** GitHub is asked for the pull requests again
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should refresh from the overview`)

#### Scenario: Patronus
- **WHEN** the user clicks Refresh of Patronus in the footer
- **THEN** Patronus is asked again
  [@test] tests/e2e/patronus/patronus.spec.ts (`should list the robots again from the overview`)
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should wait for the Safe Push of the root instead of the failure of the whole straight stack, once it runs`)

## User Experience

- The button is `ghost` `sm`, the number in tabular figures, the bar `w-16 h-1.5` of `rounded-sm` segments 2px apart;
  the node is the `LaneNodeDot` of the table, the ring the `RobotRing` of a running robot.
- The overview is a `Popover` aligned to the end of the button, `min-w-96`, as wide as its content up to `max-w-2xl`,
  at most as high as the room below the button (`--available-height`): the title stays at the top, the line of the
  last day and the footer at the bottom, the sections scroll between them (`landing-sections`).
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should queue the robots nearest to finishing first, and fit the window`)
- Its sections are titled in muted `text-xs`. A row is two lines: the name (`text-sm`, underlined on hover, "Show
  auth" as its tooltip) and its size, below them the reason in muted `text-xs`; the step is the ghost `xs` button of
  the slot, its `compactLabel`, at the end of a column as wide as the widest step of the overview: the sections are one
  grid (the rows subgrids of it; a robot row spans it), so the pull request chips and the steps line up across them.
  A branch of a stack that needs a fix is indented one step under its stack.
  [@test] tests/e2e/landing/landing-overview.spec.ts (`should run the next step of a move from the overview at its row: a Safe Push, a pull request`)
- The footer lines wrap instead of truncating.

## Testing / Local Run

- `pnpm test:unit`: `src/lib/landing-overview.test.ts`, `src/presenters/landing-node.test.ts`, `src/lib/robot-overview.test.ts`.
- `cargo nextest run -p domain-policy -p repository-model`: the nodes and the steps of the units
  (`landing_node_test.rs`, `next_step_test.rs`, `derive_test.rs`).
- `pnpm e2e`: `tests/e2e/landing/landing-overview.spec.ts`; the footer in `tests/e2e/update-project`,
  `tests/e2e/github-sign-in` and `tests/e2e/pull-requests`. Test ids: `landing-indicator` (`data-urgent`),
  `landing-moves`, `landing-bar`, `landing-popover`, its scrolling `landing-sections`, the sections
  `landing-your-move`, `landing-running`, `landing-waiting`, `landing-in-master`, the line of the last day
  `landing-finished`, the rows `landing-row` (`data-branch-name`, `data-kind`) with `landing-row-reason`, the robot
  rows `robots-row` with `robots-row-state`, `robots-row-checks`, `robots-row-failed` and the Dry Run's
  `robots-row-mode`.

## Open Questions / Risks

- A unit's node is as fresh as the readiness of its branch: a branch no surface showed yet waits for its first answer
  until the overview opens.
- The overview of a big repository can be long: a stack is one row, but every lone branch is one; it scrolls within
  the window, the moves first.
- The line of the last day counts the robots of Patronus; Activity has those the app told about, so a robot that merged
  while the app was closed is in the line, not in Activity.

## References

- [Landing Work into Master](../../../../docs/design/landing.md): what lands, in how many robots
- [Branch Row](../../branch/branch-row/spec.md#tree): the node of a branch, its kinds and their legend
- [Next Step](../../branch/next-step/spec.md): the step of each row
- [Robots in the Dock](../robots-indicator/spec.md): the Dock and its badge, from the same robots
- [Activity](../activity/spec.md): the history of the robots, its window opened by the line of the last day
- [Outgoing Commits](../outgoing/spec.md): what a push sends, its colours and its button
