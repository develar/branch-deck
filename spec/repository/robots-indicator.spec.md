---
name: Robots Indicator
description: Requirements for the header indicator of the Patronus robots of the repository's branches (running, failed, needs review) and its list of the running robots and those of the last day, and for the Dock of the app, its progress bar of the robot nearest to finishing and its badge for what needs the user.
style: plain-1
targets:
  - ../../src/components/repository/RobotsIndicator.tsx
  - ../../src/components/repository/ConfigurationHeader.tsx
  - ../../src/components/repository/DockStatus.tsx
  - ../../src/pages/IndexPage.tsx
  - ../../src/hooks/useRobotOverview.ts
  - ../../src/hooks/useDockStatus.ts
  - ../../src/lib/robot-overview.ts
  - ../../src/lib/dock.ts
  - ../../src/lib/platform.ts
  - ../../src/lib/outgoing-attention.ts
  - ../../src/hooks/useOutgoingAttention.ts
  - ../../src-tauri/capabilities/default.json
  - ../../tests/e2e/setup/tauri-mocks.ts
---

# Robots Indicator

Status: Implemented
Date: 2026-10-15

## Summary

A Safe Push runs for most of an hour, often several at once, in branches far apart in the table. The header says what
the robots of the repository's branches do: a shield in the rotating ring and "2 running" while robots run, "1 failed"
when a robot failed on the branch as it is. Hovering it lists the running robots with their time and progress, the
failed ones, and the others that finished in the last day; a branch name shows its row. The Dock of the app shows the
progress of the robot nearest to finishing, and a badge only for what needs the user. It all comes from the robots the
app already listed: nothing asks Patronus more ([Patronus](../branch/patronus.spec.md#load-on-patronus)).

## Goals

- See that robots run and when the next one is likely done, without scanning the table, also with the window in the
  background.
- Notice a failed robot once, wherever its branch is in the table.
- Stay quiet while nothing needs the user: no badge for a running robot, no indicator while nothing runs or failed.

## Non-goals

- The robots of other repositories than the selected one: the main window shows one repository.
- A robot of no branch of the table (a Safe Push of another branch from the IDE): only the robots of branches are
  polled, so it would show stale.
- An action of its own: the list has the cancel and the page of a robot; restarting and starting are the row's and the
  branch menu's.

## Requirements

### Indicator

- The header shows the indicator after the outgoing indicator ([Outgoing Commits](outgoing.spec.md)), while a robot of
  a branch runs or a branch's robot failed on the branch as it is (the local tip, or the same commits before a sync
  rebuilt them on a newer baseline; a branch changed since, a fix, needs nothing). Its text has the parts that apply, in
  this order: "2 running", "1 failed" (`destructive`), "1 needs review" (`warning`, failed for missing code-owner
  approvals), apart by " · ". Its accessible name says them: "Patronus robots: 2 running, 1 failed". Hidden otherwise.
  [@test] ../../src/lib/robot-overview.test.ts (`lists the running robots of the branches in the table order, and the branches whose robot failed on them as they are`, `says what runs and what needs the user, and nothing more`)
  [@test] ../../tests/e2e/patronus/robots-indicator.spec.ts (`should show what runs and what failed in the header, and list the robots of the last day`, `should be hidden while nothing runs and nothing failed`)
- A failed robot is told here, not by the attention dot of the outgoing indicator, which keeps the pull requests
  (changes requested, failing checks): once.
  [@test] ../../src/lib/outgoing-attention.test.ts (`names what each pull request waits for, in the table order, a branch once`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show the chips in a column, and leave a robot that failed on the branch tip to the robots indicator`)

### List

- Hovering or clicking the indicator opens the list, "Patronus robots": "Running" (the robots of the branches that run
  or cancel, in the table order), "Needs attention" (the failed ones of the branch as it is, however long ago), then
  "Last 24 hours" (the others that finished in the last day, the newest robot of each branch, newest first, a branch
  archived since too). A branch is listed once.
  [@test] ../../src/lib/robot-overview.test.ts (`lists the robots that finished in the last day, the newest of each branch, also of a branch archived since`)
- A row has the icon of the state (the ring while it runs) and the branch; below it the push mode, then for a running
  robot its time as in the Safe Push slot ("Safe Push · 18m · ~22m left", [Patronus](../branch/patronus.spec.md#timing))
  and a progress bar below, for a finished one the state in its hue and when it finished ("Safe Push · Failed · 50
  minutes ago"); then "Cancel Safe Push" for a running robot and "Open in Patronus", icon buttons named after the robot
  ("Cancel Safe Push of JIRA-456") with the tooltip of the action.
  [@test] ../../tests/e2e/patronus/robots-indicator.spec.ts (`should show what runs and what failed in the header, and list the robots of the last day`, `should cancel a running robot from the list`)
- A click on a branch name closes the list and reveals the row (`requestReveal`), as in the outgoing overview; a branch
  without a row (archived since) is plain text.
  [@test] ../../tests/e2e/patronus/robots-indicator.spec.ts (`should show what runs and what failed in the header, and list the robots of the last day`)
- A cancel is the toast "Safe Push is canceling" of the robot details ([Patronus](../branch/patronus.spec.md#restart-and-cancel)).

### Dock

- The main window alone sets the Dock (the taskbar button on Windows), for the selected repository: the progress bar
  shows the running robot nearest to finishing (the least time left), 0 to 100 of its usual time; indeterminate when no
  running robot has an estimate left (no history, or past its usual time; macOS draws it as an empty bar); none while
  nothing runs. It renews with the relative times, every 15 s.
  [@test] ../../src/lib/robot-overview.test.ts (`shows the running robot nearest to finishing, indeterminate without an estimate, nothing while none runs`)
  [@test] ../../tests/e2e/patronus/robots-indicator.spec.ts (`should show the progress of the robot nearest to finishing, and a badge only for what needs the user`, `should show an indeterminate progress without history, and no badge for a running robot`)
- The badge counts what needs the user: the robots that failed on the branch as it is, "Needs review" too, and the
  branches the last sync left with a conflict. Never a running robot; no badge at 0. Windows has no badge count: it is
  left out there.
  [@test] ../../src/lib/robot-overview.test.ts (`counts the conflicts of the last sync for the badge`)
  [@test] ../../tests/e2e/patronus/robots-indicator.spec.ts (`should show the progress, without a badge on Windows`)
- A call goes out only on a change; a failed one is logged and otherwise ignored. The capability of the main window
  allows `core:window:allow-set-progress-bar` and `core:window:allow-set-badge-count`.

## User Experience

- The indicator is a ghost `sm` button as the outgoing indicator, in the toolbar, which the page zoom leaves alone
  ([Zoom](zoom.spec.md)). While robots run its icon is a `size-2` shield in the `size-4` ring (`RobotRing`,
  `text-primary`); else the shield of the state, `ShieldXIcon` `destructive` or `ShieldAlertIcon` `warning`. The parts
  are in tabular figures, "2 running" in the foreground colour.
- The list is a `Popover` (`w-96`, `p-3`), its sections titled in muted `text-xs`; a row is two lines, as an Activity
  entry: the icon, the branch (`text-sm`, truncated, underlined on hover, with the tooltip "Show JIRA-456"), below it
  the mode and the state or time (muted `text-xs`, tabular), then the `icon-xs` ghost buttons; the progress bar is
  `h-1`, `bg-primary` on `bg-primary/15`, under the time.

## Testing / Local Run

- `pnpm test:unit`: `src/lib/robot-overview.test.ts`, `src/lib/outgoing-attention.test.ts`.
- `pnpm e2e`: `tests/e2e/patronus/robots-indicator.spec.ts`. The Tauri mock records what the page sets in the Dock in
  `window.__e2eDockCalls`. The user agent of the Desktop Chrome device is the one of Windows (no badge); the Dock tests
  take the one of macOS.
- In the app: start a Safe Push, then look at the Dock; a failed robot puts "1" on its icon.

## Open Questions / Risks

- The Dock shows the selected repository only: a robot of another repository open earlier is not in it.
- The last day counts from the time of the page, renewed by the minute: a robot that finished 24 hours ago drops out
  within a minute.

## References

- [Patronus](../branch/patronus.spec.md): the robots, their states, the timing and the load on Patronus
- [Outgoing Commits](outgoing.spec.md): the indicator before this one, its attention dot
- [Activity](activity.spec.md): what the app did, a robot started, restarted or canceled among it
