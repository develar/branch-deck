---
name: Landing Overview
description: Requirements for the header button of the work on its way into master and its overview: the units of the table (a lone branch, a stack) by their landing nodes, the moves that are the user's (their count, the node of the most urgent one, a bar of the way), the next step of each run from the overview at its row, the robots that land as a queue with their time and actions, what waits for others, what is in master until the update, the robots of the last day in one line that opens Activity, and the footer of the update state, the pull requests and Patronus; it fits the window.
style: plain-1
targets:
  - ../../src/components/repository/LandingIndicator.tsx
  - ../../src/components/repository/LandingPopover.tsx
  - ../../src/components/repository/RobotRows.tsx
  - ../../src/components/repository/ConfigurationHeader.tsx
  - ../../src/lib/activity-window.ts
  - ../../src/hooks/useLandingOverview.ts
  - ../../src/hooks/useRobotOverview.ts
  - ../../src/lib/landing-overview.ts
  - ../../src/lib/landing-node.ts
  - ../../src/lib/robot-overview.ts
  - ../../src/lib/patronus-actions.ts
  - ../../src/hooks/usePatronusRobotDetails.ts
  - ../../src/lib/check-counts.ts
  - ../../src/components/repository/UpdateProjectButton.tsx
  - ../../src/components/repository/PullRequestStatusLine.tsx
  - ../../src/components/repository/PatronusStatusLine.tsx
---

# Landing Overview

Status: Implemented
Date: 2026-10-18

## Summary

The work of the branch is done when it is in master ([Landing](../../docs/design/landing.md)). The header says where
it is on that way, in the words of the node of each row ([Branch Row](../branch/branch-row.spec.md#tree)): the node of
the most urgent move that is the user's, how many there are, a bar of the work from landing to waiting to the user's
moves. Hovering it opens the overview, which tells only the present: the user's moves with the next step of each, the
robots that land as a queue (the nearest to finishing first), what waits for others, what is in master until the
update; the robots of the last day are one line, their history is [Activity](activity.spec.md). As IntelliJ's
background tasks show only what runs, and Chrome's downloads the latest with the rest on a page, the overview stays
short and fits the window. A push is not a move: the steps push what they
need ([Next Step](../branch/next-step.spec.md#goals)). It replaces the indicator of the commits to push and the robots
indicator, whose states were the same story told twice.

## Goals

- Answer at a glance whether something is the user's move, and how urgent the most urgent one is.
- Act on a move from wherever the table is scrolled: the step runs at its row, as the slot of the row would run it.
- Count what lands, not branches: a stack lands in one robot, so it is one move.

## Non-goals

- A "Push all": a push stays per branch and per stack ([Outgoing Commits](outgoing.spec.md#push)), and no push is
  offered here.
- Asking Patronus or GitHub for anything of its own: the overview shows the answers the rows have; while it is open it is
  a surface of the readiness of its rows ([Next Step](../branch/next-step.spec.md#the-surfaces)).

## Requirements

### Units

- A unit is what lands in one move: a lone branch, or a stack (its root's row), whose node and next step are those of
  the whole stack, as the header of its island shows them (`getStackLandingNodeNow`, `useStackNextStep`: of its stack
  branch while the stack forks, else of the last branch of its straight chain). The branches of a stack whose own node
  needs a fix are listed below it, each a move of its own. Units come in the order of the table.
  [@test] ../../src/lib/landing-overview.test.ts (`counts the user's moves by unit, a ready stack of 22 branches one move, the most urgent first`, `lists a stack whose branch needs a fix with the moves, the fix counted as a move of its own`)
  [@test] ../../tests/e2e/landing/landing-overview.spec.ts (`should run the next step of a move from the overview at its row: a Safe Push, a pull request`)

### Indicator

- The header shows the button after Update Project, while a unit is on its way to master or in master, or a robot ran
  in the last day; hidden otherwise. Its icon is the node of the most urgent move (`needsFix`, then `yourStep`, then
  `ready`; a hollow neutral node without one), in the ring of a running Safe Push; then the number of the user's moves
  (none at 0); then the bar.
  [@test] ../../tests/e2e/landing/landing-overview.spec.ts (`should count the user's moves, put the most urgent node in the header and group the work by where it is`, `should be hidden while nothing is on the way to master and nothing ran in the last day`)
- The moves are the units whose node is filled, their branches that need a fix, the failed robots of no row (a
  temporary ref, a watched pull request) and the running Safe Pushes whose required check failed. A unit that waits for
  the first answer of Patronus counts no move, so the number does not change when the overview opens and asks; a unit
  whose Safe Push starts lands already, so the number drops at the start, not when Patronus lists the robot.
  [@test] ../../src/lib/landing-overview.test.ts (`counts no move of a unit before the first answer of Patronus, so the number doesn't change on hover`, `counts a failed robot of no row and a running Safe Push that will fail as moves, listed with them; a Dry Run lands nothing`)
  [@test] ../../src/lib/landing-node.test.ts (`is landing once its Safe Push starts, before its robot exists: the start is the move made; a Dry Run lands nothing`)
  [@test] ../../tests/e2e/landing/landing-overview.spec.ts (`should count a running Safe Push whose required check failed as a move, show it failing and cancel it from the toast`)
- The bar has a segment per kind of the units on their way, in the order of the way: landing, waiting, ready, the
  user's step, a fix; its width the commits of its units; light for a hollow node. None for a single kind: the node
  says it.
  [@test] ../../src/lib/landing-overview.test.ts (`has a segment per kind on the way, landing first and the user's moves last, its weight the commits`)
- Its accessible name, the title of the overview: "On the way to master: 3 your move · 1 landing · 3 waiting · 1 in
  master", the parts that apply; "landing" counts the Safe Pushes that run and will not fail.

### Overview

- Hovering (after a short delay) or a click opens the overview; it is loaded while the delay runs. Its sections, each
  hidden while empty, titled with their count:
  - **Your move**: the units with a fix first, then the user's step, then the ready ones; a branch whose robot failed
    on it as it is is the robot's row, with its actions; then the failed robots of no row and the running robots that
    will fail (a required check failed: no use waiting, the user cancels or restarts them), each a move;
  - **Landing** ("Landing · 5", "Landing · 5 · 1 dry run"): the other running robots of the branches, the whole stacks
    and the watched pull requests, as a queue: the Safe Pushes nearest to finishing first, by the time left as the Dock
    picks its robot (`compareByFinish`; one without an estimate last), then the Dry Runs, which land nothing, the same
    way. The order is taken when the overview opens and kept while it is open, so a row never jumps under the pointer;
    a robot started since comes last;
  - **Waiting**: the units that wait for others;
  - **In master**: the units the baseline or a merge has, until the update archives them.
  [@test] ../../tests/e2e/landing/landing-overview.spec.ts (`should count the user's moves, put the most urgent node in the header and group the work by where it is`, `should queue the robots nearest to finishing first, and fit the window`)
  [@test] ../../src/lib/robot-overview.test.ts (`queues the Safe Pushes nearest to finishing first, those without an estimate last, then the Dry Runs`)
  [@test] ../../src/lib/landing-overview.test.ts (`is empty with nothing on the way, nothing in master and no robot of the last day`)
- The robots of the last day are one line below the sections, not a list: "auth, login, oauth and 6 more merged · ui
  failed · 1 canceled · 2 dry runs passed in the last 24 hours" (`formatFinishedSummary`, the parts that apply; the
  merged and the failed named, so a glance answers what landed today), the newest robot of each branch, without those
  of the units in master (the In master row says it). "Show in Activity" closes the overview and opens Activity at its
  Robots, the history of the robots the app told about.
  [@test] ../../src/lib/landing-overview.test.ts (`names what merged and what failed, counts the canceled and the passed Dry Runs, nothing without a robot`, `leaves out of the last day the robots of what is in master: the In master row says it`)
  [@test] ../../tests/e2e/landing/landing-overview.spec.ts (`should count the user's moves, put the most urgent node in the header and group the work by where it is`)
- A row of a unit has its node, its name (a click closes the overview and shows the row; of a stack, the stack
  expanded), its size ("1 commit", "5 branches · 37 commits"), the first line of its node's hover (on hover the hover of
  its node itself, `LandingNodeHover` of [Branch Row](../branch/branch-row.spec.md#tree): the issue keys as links, "Show
  auth" of a parent it names, which closes the overview, the robot whose state it tells), its pull request chip and the
  next step of its row (`StepButton`, the item of `getBranchNextStep`).
  [@test] ../../tests/e2e/landing/landing-overview.spec.ts (`should count the user's moves, put the most urgent node in the header and group the work by where it is`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show a robot that failed on the branch tip once, as the move it is, with the chips of the other rows`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should show the pull request chips of the rows, and a pull request that waits for its author as a fix`)
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should show every branch again for a reveal of a hidden one`, `should show a match deep in a collapsed stack, and expand the stack for a reveal`)
- A click on the step closes the overview, shows the row of its owner (`requestReveal`: a collapsed stack expands, a
  filter that hides it clears) and runs the step there, so its popovers open at the slot of the row: the start of a
  Safe Push (at once when it has nothing to ask), Create Pull Request…, Request Review… (its popover at the slot),
  Retarget, Restart Safe Push.
  [@test] ../../tests/e2e/landing/landing-overview.spec.ts (`should run the next step of a move from the overview at its row: a Safe Push, a pull request`)
- A robot row is two lines, as a background task: the icon of the state (the ring while it runs), the branch, then its
  state in a right-aligned column of tabular figures, for a running robot its time ("18m · ~1h 32m",
  [Patronus](../branch/patronus.spec.md#timing)), for a finished one the state in its hue and when it finished
  ("Failed · 50 minutes ago"), with the mode and the merge profile on hover ("Safe Push · Large Scale Change"); the mode
  is written only for a Dry Run ("Dry Run · 15m · ~20m", a link that opens it in Patronus): a Safe Push is what
  lands. Then the actions of the robot as every surface lists them (`getGatedRobotActions`,
  [Patronus](../branch/patronus.spec.md#restart-and-cancel)), icon buttons named after the robot ("Cancel Safe Push of
  JIRA-456", "Open Safe Push of JIRA-456 in Patronus") with the name of the action as the tooltip, a disabled one
  adding why. The second line of a running robot is its progress bar beside the check it waits for. While the overview
  is open, a running row asks for the details of its robot, as its popover would: the check it waits for and the queued
  checks in a column of their own (kept while they load, so the bar doesn't jump; a long name cut, the whole on
  hover), the checks that failed already below ("Failed: Bazel Community Build Aggregator", "and 2 more" past three,
  four of four named: the backend names one more than three, `list_cut::kept`). A failing robot has a `destructive`
  ring and bar, and "18m · Failing" instead of the time left. A watched robot of no branch is listed by what it tests
  ("#101 Rework the session store"); a branch without a row and a watched robot are plain text.
  [@test] ../../tests/e2e/landing/landing-overview.spec.ts (`should cancel a running robot from the overview`, `should restart a failed robot from the overview, and open the pull request it was started from`, `should count a running Safe Push whose required check failed as a move, show it failing and cancel it from the toast`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should read the estimate of the CI only while the details of a running robot are open`)
  [@test] ../../src/lib/check-counts.test.ts (`names the checks that failed, counts the ones beyond the first three, nothing while none failed`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should watch the robot of a reviewed pull request: listed by what it tests, its merge told`)
- While the overview is open, each of its rows is a surface of the readiness of its branch (`useShowReviewReadiness`),
  so the units of a collapsed stack get their answer too; while some wait, a line under the title says "Asking
  Patronus about 5 branches…".
- Below a rule, the footer: the update state of the baseline with Update ([Update Project](update-project.spec.md)),
  the pull requests checked with Refresh ([Pull Requests](../branch/pull-requests.spec.md)), Patronus checked with
  Refresh ([Patronus](../branch/patronus.spec.md)).
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should show the update state in the footer of the landing overview`)
  [@test] ../../tests/e2e/github-sign-in/github-sign-in.spec.ts (`should sign in from the hint of a repository without a token, and ask GitHub again`)

## User Experience

- The button is `ghost` `sm`, the number in tabular figures, the bar `w-16 h-1.5` of `rounded-sm` segments 2px apart;
  the node is the `LaneNodeDot` of the table, the ring the `RobotRing` of a running robot.
- The overview is a `Popover` aligned to the end of the button, `min-w-96`, as wide as its content up to `max-w-2xl`,
  at most as high as the room below the button (`--available-height`): the title stays at the top, the line of the
  last day and the footer at the bottom, the sections scroll between them (`landing-sections`).
  [@test] ../../tests/e2e/landing/landing-overview.spec.ts (`should queue the robots nearest to finishing first, and fit the window`)
- Its sections are titled in muted `text-xs`. A row is two lines: the name (`text-sm`, underlined on hover, "Show
  auth" as its tooltip) and its size, below them the reason in muted `text-xs`; the step is the ghost `xs` button of
  the slot, its `compactLabel`, at the end of a column as wide as the widest step of the overview: the sections are one
  grid (the rows subgrids of it; a robot row spans it), so the pull request chips and the steps line up across them.
  A branch of a stack that needs a fix is indented one step under its stack.
  [@test] ../../tests/e2e/landing/landing-overview.spec.ts (`should run the next step of a move from the overview at its row: a Safe Push, a pull request`)
- The footer lines wrap instead of truncating.

## Testing / Local Run

- `pnpm test:unit`: `src/lib/landing-overview.test.ts`, `src/lib/landing-node.test.ts`, `src/lib/robot-overview.test.ts`.
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

- [Landing Work into Master](../../docs/design/landing.md): what lands, in how many robots
- [Branch Row](../branch/branch-row.spec.md#tree): the node of a branch, its kinds and their legend
- [Next Step](../branch/next-step.spec.md): the step of each row
- [Robots in the Dock](robots-indicator.spec.md): the Dock and its badge, from the same robots
- [Activity](activity.spec.md): the history of the robots, its window opened by the line of the last day
- [Outgoing Commits](outgoing.spec.md): what a push sends, its colours and its button
