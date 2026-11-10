# Proposal

## Why

The title bar of the review window offered "Open Pull Request" without saying which one, and showed nothing of the
Safe Push that runs on the branch: the review of `jna-one-copy` read its two commits while its robot of PR #1593 ran
for 1h 36m, and only the main window said so.

## What Changes

- "Open Pull Request #1593": the one item of the branch's pull request (`openPullRequestItemOf`) names its number on
  every surface, the menu of the row, the footer of the preview (which now takes the item) and the title bar of the
  review; the toast of a robot that needs review offers the same item.
- The title bar of the review window shows after the stack bar the robot that lands what it shows while it runs, as
  the Status cell of a row (`RunningRowRobot`): its own, the Safe Push of a branch built on it or of its whole stack
  that lands it ("lands with session"), or the robot of the whole stack; its hover is the popover of its details with
  "Open in Patronus", a click opens it; it takes the room the stack bar leaves, the ring and the line alone in the
  narrowest window. The details of the robot of a whole stack compare its commit with the stack branch.

## Capabilities

### Modified Capabilities

- `branch/branch-review`: the robot in the title bar; "Open Pull Request #42".
- `branch/pull-requests`: "Open Pull Request #7" in the menu, the preview and beside a closed one.
- `branch/branch-row`: O and the menu name "Open Pull Request #7".

## Impact

- `src/components/review/ReviewRobotStatus.tsx`, `src/pages/ReviewPage.tsx`, `src/components/branch/BranchRowStatus.tsx`
  (`RunningRowRobot` exported, `titleBar`), `src/components/branch/PatronusChip.tsx` (`stackRoot`),
  `src/hooks/useRowRobot.ts` (`useRobotChip`).
- `src/hooks/useBranchContextActions.ts`, `src/components/branch/BranchPreviewActions.tsx`.
