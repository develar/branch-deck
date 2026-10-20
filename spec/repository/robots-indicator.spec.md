---
name: Robots in the Dock
description: Requirements for the Dock of the app (the taskbar button on Windows) and the Patronus robots of the repository's branches, its progress bar of the robot nearest to finishing and its badge for what needs the user; the robots in the header are the Landing Overview's.
style: plain-1
targets:
  - ../../src/components/repository/DockStatus.tsx
  - ../../src/pages/IndexPage.tsx
  - ../../src/hooks/useRobotOverview.ts
  - ../../src/hooks/useDockStatus.ts
  - ../../src/lib/dock.ts
  - ../../src/stores/settings/appSettings.ts
  - ../../src/lib/robot-overview.ts
  - ../../src/lib/platform.ts
  - ../../src-tauri/capabilities/default.json
  - ../../tests/e2e/setup/tauri-mocks.ts
---

# Robots in the Dock

Status: Implemented
Date: 2026-10-18

## Summary

A Safe Push runs for most of an hour, often several at once. With the window in the background, the Dock of the app
shows the progress of the robot nearest to finishing, and a badge only for what needs the user. It all comes from the
robots the app already listed (`collectRobotOverview`), the same the header shows: the robots that land are a section of
the [Landing Overview](landing-overview.spec.md) in the order the Dock picks its robot (the nearest to finishing first,
`compareByFinish`), those of the last day its line, a failed one is a move there.

## Goals

- See when the next robot is likely done, also with the window in the background.
- Stay quiet while nothing needs the user: no badge for a running robot.

## Non-goals

- The robots of other repositories than the selected one: the main window shows one repository.
- A robot of no branch of the table (a Safe Push of another branch from the IDE): only the robots of branches are
  polled, so it would show stale. The one exception is a robot the app started on no branch and watches: a Safe Push of
  a reviewed pull request ([Review Requests](review-requests.spec.md#safe-push-of-a-reviewed-pull-request)).

## Requirements

### Dock

- The main window alone sets the Dock (the taskbar button on Windows), for the selected repository: the progress bar
  shows the running robot nearest to finishing (the least time left), 0 to 100 of its usual time; indeterminate when no
  running robot has an estimate left (no history, or past its usual time; macOS draws it as an empty bar); none while
  nothing runs. A failing robot is not waited for: it is left out. It renews with the relative times, every 15 s.
  While an update or a fetch of the baseline runs in the selected repository, the bar shows its progress instead, set
  when its whole percent changes; the robots' comes back once it ends ([Update Project](update-project.spec.md)).
  [@test] ../../src/lib/robot-overview.test.ts (`shows the running robot nearest to finishing, indeterminate without an estimate, nothing while none runs`)
  [@test] ../../tests/e2e/patronus/dock.spec.ts (`should show the progress of the robot nearest to finishing, and a badge only for what needs the user`, `should show an indeterminate progress without history, and no badge for a running robot`)
  [@test] ../../tests/e2e/update-project/update-project.spec.ts (`should show one progress from the click to the end of its sync, its steps in the tooltip`)
- The badge counts what needs the user: the robots that failed on the branch as it is, "Needs review" too, unless
  another robot holds or landed the branch or the baseline has it ([Patronus](../branch/patronus.spec.md)), the robot
  of the whole stack only on a stack branch the sync built ([Whole Stack](../branch/whole-stack.spec.md#the-whole-stack-in-the-app)), the running
  robots that will fail (failing: cancel one, or wait for its failure, which keeps the count), and the
  branches the last sync left with a conflict ("Count failed robots and conflicts", `attentionBadge`), plus the pull
  requests that wait for the user's review ("Count review requests", `reviewBadge`,
  [Review Requests](review-requests.spec.md#badge)); both switches are in Settings → Notifications, on by default. Each source sets its
  count (`setBadgeSource` in `src/lib/dock.ts`, the only `setBadgeCount` caller), the badge shows their sum. Never a
  running robot otherwise; no badge at 0. Windows has no badge count: it is left out there, and so is the Dock group of Settings → Notifications.
  [@test] ../../src/lib/robot-overview.test.ts (`counts the conflicts of the last sync for the badge`)
  [@test] ../../src/lib/robot-overview.test.ts (`lists the robot of the whole stack as the whole stack after the branches, on its stack branch as it is, a click showing the root, its failure on one not built as finished`, `asks nothing for the failure of a branch another robot holds or landed, or the baseline has: it is finished history`)
  [@test] ../../src/lib/dock.test.ts (`shows the sum of its sources, set only on a change, none at 0`)
  [@test] ../../tests/e2e/review-requests/review-requests.spec.ts (`should count the review requests, behind its switch`)
  [@test] ../../tests/e2e/patronus/dock.spec.ts (`should show the progress, without a badge on Windows`)
  [@test] ../../tests/e2e/patronus/dock.spec.ts (`should count a running robot that will fail in the badge, and not show its progress`)
  [@test] ../../tests/e2e/patronus/dock.spec.ts (`should count the failed robot of the whole stack only while the sync built its stack branch`, `should not count the failed robot of a branch while the Safe Push of a branch built on it lands it`)
- A call goes out only on a change; a failed one is logged and otherwise ignored. The capability of the main window
  allows `core:window:allow-set-progress-bar` and `core:window:allow-set-badge-count`.

## Testing / Local Run

- `pnpm test:unit`: `src/lib/robot-overview.test.ts`, `src/lib/dock.test.ts`.
- `pnpm e2e`: `tests/e2e/patronus/dock.spec.ts`. The Tauri mock records what the page sets in the Dock in
  `window.__e2eDockCalls`. The user agent of the Desktop Chrome device is the one of Windows (no badge); the Dock tests
  take the one of macOS.
- In the app: start a Safe Push, then look at the Dock; a failed robot puts "1" on its icon.

## Open Questions / Risks

- The Dock shows the selected repository only: a robot of another repository open earlier is not in it.

## References

- [Patronus](../branch/patronus.spec.md): the robots, their states, the timing and the load on Patronus
- [Landing Overview](landing-overview.spec.md): the robots in the header, a failed one a move
- [Activity](activity.spec.md): what the app did, a robot started, restarted or canceled among it
