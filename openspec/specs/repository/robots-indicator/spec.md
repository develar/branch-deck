---
name: Robots in the Dock
description: Requirements for the Dock of the app (the taskbar button on Windows) and the Patronus robots of the repository's branches, its progress bar of the robot nearest to finishing and its badge for what needs the user; the robots in the header are the Landing Overview's.
style: plain-1
targets:
  - src/components/repository/DockStatus.tsx
  - src/pages/IndexPage.tsx
  - src/hooks/useRobotOverview.ts
  - src/hooks/useDockStatus.ts
  - src/lib/dock.ts
  - src/stores/settings/appSettings.ts
  - src/lib/robot-overview.ts
  - crates/domain-policy/src/attention.rs
  - src/lib/platform.ts
  - src-tauri/capabilities/default.json
  - tests/e2e/setup/tauri-mocks.ts
---

# Robots in the Dock

Status: Implemented
Date: 2026-10-18

## Purpose

A Safe Push runs for most of an hour, often several at once. With the window in the background, the Dock of the app
shows the progress of the robot nearest to finishing, and a badge only for what needs the user. It all comes from the
robots the app already listed (`collectRobotOverview`), the same the header shows: the robots that land are a section of
the [Landing Overview](../landing-overview/spec.md) in the order the Dock picks its robot (the nearest to finishing first,
`compareByFinish`), those of the last day its line, a failed one is a move there.

## Goals

- See when the next robot is likely done, also with the window in the background.
- Stay quiet while nothing needs the user: no badge for a running robot.

## Non-goals

- The robots of other repositories than the selected one: the main window shows one repository.
- A robot of no branch of the table (a Safe Push of another branch from the IDE): only the robots of branches are
  polled, so it would show stale. The one exception is a robot the app started on no branch and watches: a Safe Push of
  a reviewed pull request ([Review Requests](../review-requests/spec.md#safe-push-of-a-reviewed-pull-request)).

## Requirements

### Requirement: Dock: The main window alone sets the Dock of the selected repository

The main window alone SHALL set the Dock (the taskbar button on Windows), for the selected repository.

#### Scenario: The main window shows a repository
- **WHEN** the main window shows the selected repository
- **THEN** it alone sets the Dock, for that repository

### Requirement: Dock: The progress bar shows the running robot nearest to finishing

The progress bar MUST show the running robot nearest to finishing (the least time left), 0 to 100 of its usual time;
indeterminate when no running robot has an estimate left (no history, or past its usual time; macOS draws it as an
empty bar); none while nothing runs.

#### Scenario: Several robots run
- **WHEN** several robots run and some have an estimate left
- **THEN** the progress bar shows the one with the least time left, 0 to 100 of its usual time
  [@test] src/lib/robot-overview.test.ts (`shows the running robot nearest to finishing, indeterminate without an estimate, nothing while none runs`)
  [@test] tests/e2e/patronus/dock.spec.ts (`should show the progress of the robot nearest to finishing, and a badge only for what needs the user`)

#### Scenario: No running robot has an estimate left
- **WHEN** no running robot has an estimate left, as it has no history or is past its usual time
- **THEN** the progress bar is indeterminate (macOS draws it as an empty bar)
  [@test] src/lib/robot-overview.test.ts (`shows the running robot nearest to finishing, indeterminate without an estimate, nothing while none runs`)
  [@test] tests/e2e/patronus/dock.spec.ts (`should show an indeterminate progress without history, and no badge for a running robot`)

#### Scenario: Nothing runs
- **WHEN** no robot runs
- **THEN** there is no progress bar
  [@test] src/lib/robot-overview.test.ts (`shows the running robot nearest to finishing, indeterminate without an estimate, nothing while none runs`)

### Requirement: Dock: A failing robot is left out of the progress

A failing robot MUST NOT be waited for: the progress bar MUST leave it out.

#### Scenario: A running robot will fail
- **WHEN** a running robot is failing
- **THEN** the progress bar does not show it
  [@test] tests/e2e/patronus/dock.spec.ts (`should count a running robot that will fail in the badge, and not show its progress`)

### Requirement: Dock: The progress renews with the relative times

The progress MUST renew with the relative times, every 15 s.

#### Scenario: Only the clock moves
- **GIVEN** a running robot with an estimate in the progress bar
- **WHEN** only the clock moves
- **THEN** the progress follows it, every 15 s
  [@test] tests/e2e/patronus/dock.spec.ts (`should show the progress of the robot nearest to finishing, and a badge only for what needs the user`)

### Requirement: Dock: An update or a fetch of the baseline shows its progress instead

While an update or a fetch of the baseline runs in the selected repository, the bar MUST show its progress instead,
set when its whole percent changes; the robots' MUST come back once it ends
([Update Project](../update-project/spec.md)).

#### Scenario: An update runs
- **WHEN** an update or a fetch of the baseline runs in the selected repository
- **THEN** the bar shows its progress, set when its whole percent changes
- **AND** the robots' progress comes back once it ends
  [@test] tests/e2e/update-project/update-project.spec.ts (`should show one progress from the click to the end of its sync, its steps in the tooltip`)

### Requirement: Dock: The badge counts the robots that failed on the branch as it is

The badge MUST count the robots that failed on the branch as it is, "Needs review" too, unless another robot holds or
landed the branch or the baseline has it ([Patronus](../../branch/patronus/spec.md)), as Rust names them in the view
(`attention`, `domain_policy::attention`).

#### Scenario: A robot failed on the branch as it is
- **WHEN** a robot failed, or needs review, on the branch as it is
- **THEN** the badge counts it, as the view names it
  [@test] tests/e2e/patronus/dock.spec.ts (`should show the progress of the robot nearest to finishing, and a badge only for what needs the user`)
  [@test] crates/repository-model/src/derive_test.rs (`the_view_names_the_robots_that_need_the_user_over_the_rows_as_they_are`)

#### Scenario: Another robot holds or landed the branch, or the baseline has it
- **WHEN** a robot failed on a branch that another robot holds or landed, or that the baseline has
- **THEN** the badge does not count it
  [@test] src/lib/robot-overview.test.ts (`asks nothing for a failure the view does not name, nor for one it names of another robot than the row's: it is finished history`)
  [@test] crates/repository-model/src/derive_test.rs (`the_view_names_the_robots_that_need_the_user_over_the_rows_as_they_are`)
  [@test] tests/e2e/patronus/dock.spec.ts (`should not count the failed robot of a branch while the Safe Push of a branch built on it lands it`)

### Requirement: Dock: The badge counts the robot of the whole stack on a built stack branch

The badge MUST count the robot of the whole stack only on a stack branch the sync built
([Whole Stack](../../branch/whole-stack/spec.md#the-whole-stack-in-the-app)), as Rust names it in the view.

#### Scenario: The robot of the whole stack failed
- **WHEN** the robot of the whole stack failed on the stack branch the sync built
- **THEN** the badge counts it
- **AND** once the sync no longer builds that stack branch, the badge does not count it
  [@test] src/lib/robot-overview.test.ts (`lists the robot of the whole stack as the whole stack after the branches, a click showing the root, its failure as the move the view names, any other as finished`)
  [@test] tests/e2e/patronus/dock.spec.ts (`should count the failed robot of the whole stack only while the sync built its stack branch`)

### Requirement: Dock: The badge counts the running robots that will fail

The badge MUST count the running robots that will fail (failing: cancel one, or wait for its failure, which keeps the
count).

#### Scenario: A running robot is failing
- **WHEN** a running robot will fail
- **THEN** the badge counts it, and its failure keeps the count
  [@test] tests/e2e/patronus/dock.spec.ts (`should count a running robot that will fail in the badge, and not show its progress`)

### Requirement: Dock: The badge counts the branches the last sync left with a conflict

The badge MUST count the branches the last sync left with a conflict.

#### Scenario: The last sync left conflicts
- **WHEN** the last sync left branches with a conflict
- **THEN** the badge counts each of them
  [@test] src/lib/robot-overview.test.ts (`counts the conflicts of the last sync for the badge`)

### Requirement: Dock: The badge counts the pull requests that wait for the user's review

The badge MUST count the pull requests that wait for the user's review
([Review Requests](../review-requests/spec.md#badge)).

#### Scenario: Review requests wait
- **WHEN** pull requests wait for the user's review
- **THEN** the badge counts them
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should count the review requests, behind its switch`)

### Requirement: Dock: Two switches of Settings → Notifications turn the counts off

The robots and the conflicts MUST be counted behind "Count failed robots and conflicts" (`attentionBadge`), the review
requests behind "Count review requests" (`reviewBadge`); both switches are in Settings → Notifications, on by default.

#### Scenario: A switch is off
- **WHEN** "Count failed robots and conflicts" or "Count review requests" is off in Settings → Notifications
- **THEN** the badge leaves out what that switch counts
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should count the review requests, behind its switch`)

#### Scenario: Default
- **WHEN** the user never changed them
- **THEN** both switches are on

### Requirement: Dock: The badge shows the sum of its sources

Each source MUST set its count (`setBadgeSource` in `src/lib/dock.ts`, the only `setBadgeCount` caller), and the badge
SHALL show their sum; no badge at 0.

#### Scenario: Two sources set their counts
- **WHEN** the sources set their counts
- **THEN** the badge shows their sum
- **AND** at 0 there is no badge
  [@test] src/lib/dock.test.ts (`shows the sum of its sources, set only on a change, none at 0`)

### Requirement: Dock: The badge never counts a running robot otherwise

The badge MUST NOT count a running robot otherwise.

#### Scenario: A robot runs and is not failing
- **WHEN** a robot runs and is not failing
- **THEN** there is no badge for it
  [@test] tests/e2e/patronus/dock.spec.ts (`should show an indeterminate progress without history, and no badge for a running robot`)

### Requirement: Dock: The count of the robots waits for the view of the robots as they are

The count of the robots MUST wait for the view the session derives of the robots as they are
(`useRepositoryViewCurrent`, the part not `stale` while it is derived): what another robot holds or landed is known
only then, so no badge comes and goes before it.

#### Scenario: The robots change
- **WHEN** the robots change, and the view of them is being derived
- **THEN** the badge waits for the derived view
- **AND** no badge comes and goes before it
  [@test] tests/e2e/patronus/dock.spec.ts (`should not count the failed robot of a branch while the Safe Push of a branch built on it lands it`)

### Requirement: Dock: Windows has no badge count

Windows has no badge count: the badge MUST be left out there, and so MUST the Dock group of Settings → Notifications.

#### Scenario: On Windows
- **WHEN** the app runs on Windows and a robot failed
- **THEN** the taskbar button shows the progress and no badge
  [@test] tests/e2e/patronus/dock.spec.ts (`should show the progress, without a badge on Windows`)

#### Scenario: Settings on Windows
- **WHEN** the user opens Settings → Notifications on Windows
- **THEN** it has no Dock group

### Requirement: Dock: A call goes out only on a change

A call to the Dock MUST go out only on a change; a failed one SHALL be logged and otherwise ignored.

#### Scenario: The same value again
- **WHEN** the progress or the badge would be set to the value it has
- **THEN** no call goes out
  [@test] src/lib/dock.test.ts (`shows the sum of its sources, set only on a change, none at 0`)
  [@test] tests/e2e/patronus/dock.spec.ts (`should show the progress of the robot nearest to finishing, and a badge only for what needs the user`)

#### Scenario: A call fails
- **WHEN** a call to the Dock fails
- **THEN** it is logged and otherwise ignored

### Requirement: Dock: The capability of the main window allows the Dock calls

The capability of the main window MUST allow `core:window:allow-set-progress-bar` and
`core:window:allow-set-badge-count`.

#### Scenario: The main window sets the Dock
- **WHEN** the main window sets the progress bar or the badge count
- **THEN** its capability allows `core:window:allow-set-progress-bar` and `core:window:allow-set-badge-count`

## Testing / Local Run

- `pnpm test:unit`: `src/lib/robot-overview.test.ts`, `src/lib/dock.test.ts`.
- `pnpm e2e`: `tests/e2e/patronus/dock.spec.ts`. The Tauri mock records what the page sets in the Dock in
  `window.__e2eDockCalls`. The user agent of the Desktop Chrome device is the one of Windows (no badge); the Dock tests
  take the one of macOS.
- In the app: start a Safe Push, then look at the Dock; a failed robot puts "1" on its icon.

## Open Questions / Risks

- The Dock shows the selected repository only: a robot of another repository open earlier is not in it.

## References

- [Patronus](../../branch/patronus/spec.md): the robots, their states, the timing and the load on Patronus
- [Landing Overview](../landing-overview/spec.md): the robots in the header, a failed one a move
- [Activity](../activity/spec.md): what the app did, a robot started, restarted or canceled among it
