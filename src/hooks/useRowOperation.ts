import { useStore } from "zustand"
import type { RunningOperation } from "@/lib/bindings"
import type { RowLookup } from "@/presenters/operations"
import { operationOnRow } from "@/presenters/operations"
import { branchSyncStore, getBranchesOfCommits, useBranchSyncField } from "@/stores/branch-sync"
import { indicatorsStore, isClaimed, operationBusyOf } from "@/stores/indicators"

/** The rows of the table of the page, as the operations of the session name them */
const LOOKUP: RowLookup = {
  branchesOf: ids => getBranchesOfCommits(ids),
  pullRequestOf: name => branchSyncStore.getState().openPullRequests[name]?.number,
}

/**
 * The operation of the session that works on the row `name` (`worksOnRow`: a rewrite of the branch or of its commits,
 * the retarget of its pull request), as the view has it (`RepositoryView.operations`), from its admission until it ended:
 * the row is busy meanwhile (no right-click menu, "⋯" disabled, no drag, no push); undefined while none does
 */
export function useRowOperation(name: string): RunningOperation | undefined {
  return useBranchSyncField("operations", operations => operationOnRow(operations, name, LOOKUP))
}

/** `useRowOperation` now, for a menu built on open, a key, a drag */
export function getRowOperation(name: string): RunningOperation | undefined {
  return operationOnRow(branchSyncStore.getState().operations, name, LOOKUP)
}

/** The busy state of the operation that works on the row `name` ("Unstacking tool-events"), null while none does */
export function useRowBusy(name: string): string | null {
  const operation = useRowOperation(name)
  return operation === undefined ? null : operationBusyOf(operation)
}

/**
 * The operation of the row `name` whose start no mounted surface shows (a menu, a key, a toast, a popover the click
 * closed, another window or process): the "⋯" of the row shows its spinner and its busy state, and its Stop Waiting;
 * undefined otherwise
 */
export function useUnclaimedOperation(name: string): RunningOperation | undefined {
  const operation = useBranchSyncField("operations", operations => operationOnRow(operations, name, LOOKUP, true))
  const claimed = useStore(indicatorsStore, () => operation !== undefined && isClaimed(operation))
  return claimed ? undefined : operation
}
