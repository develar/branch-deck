# Proposal

## Why

Issue #237, lane B3: `pnpm perf:table` on the Mac Studio rendered 131 of the 141 rows of `branch_table_large` and
each of its islands twice for a sync of one more commit on one branch; B3 is that row alone. A row reads the ring of
its node off its whole next step (`useRowPart(name, "landing")?.node.ring`, since 918ac6849), the island the hold of
its root (`usePushHold`, since cfdb49097), and the hook answers the whole part, a new object with every patch of it.
After a sync that moved a tip the next step of every branch waits for the landing needs of the new tips, then has
them: two patches of every step the needs decide. `row-renders.spec.ts` synced without Patronus, which computes no
needs.

## What Changes

- `useRowPart(subject, part, select, equal)`: a component that shows one field of a part selects it and wakes only
  when that value changes. A patch replaces the part whole, each of its objects new, so a primitive is selected by
  itself and an object kept while it says the same (`equal`, `sameJson` of src/lib/record.ts; one kept answer per
  subscriber in `useBranchSyncEntry`). Primitives: the ring of the node of a row (`BranchRow`) and of the review window
  (`ReviewRobotStatus`, its stack too), the words of what landed a branch (`useLandedIn`), its earlier robot
  (`useEarlierRobot`), whether its push with children runs. Kept objects: the status of a row, of its node hover and
  of its preview, the hold of a branch (`usePushHold`: the island summary and the actions of a row), the blocker of its
  push (`usePushButton`), the Pick of a push button, the node of a row (`useNodeOf`), the item of Push with Children
  of a root (`ChildrenPushButton`), the fixes of a stack problem (`StackProblemAlert`) and the step a wait status
  says, of a row, of its preview and of an island header (`useBranchStepWait`, `useStackStepWait`).
- `part-field-read` of `pnpm lint:arch`: a field read inline off the answer of `useRowPart(…)` fails, arguments one
  call deep included; a part kept in a name, a call split over lines and deeper arguments pass it, the render counter
  is the guard.
- The render counter counts the next step of a row (`step:<name>`) and the status cell (`status:<name>` moves into
  `RowStatusCell`); `perf:table` adds `oneBranchSyncStatuses`. A test of the renders of a sync whose landing needs every
  step waits for: the steps render, of the rows and their status cells only those of the branch, neither the table nor
  an island; in Chromium and WebKit (`pnpm e2e:webkit`).

Capabilities: `branch/branch-row`.
