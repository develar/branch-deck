import { useMemo } from "react"
import { getNodeKindNow, getStackNodeBranch, getStackNodeKindNow, useNodeStores } from "@/hooks/useLandingNode"
import type { LandingNodeKind } from "@/lib/bindings"
import type { StackState } from "@/lib/stack-state"
import { firstBranchOfEachState, summarizeStackState } from "@/lib/stack-state"
import { branchSyncStore, structureIn } from "@/stores/branch-sync"

export interface StackStates {
  readonly state: StackState
  /** The first branch in each state: where a click on its chip goes */
  readonly first: Partial<Record<LandingNodeKind, string>>
}

/**
 * What the branches of the stack on `root` hold by the kinds of the nodes of their rows, as the view decides them and
 * the rows show them (`getNodeKindNow`, `summarizeStackState`): with the root and a fix of the whole stack for the
 * island header (`withRoot`), the branches stacked on it alone for the chip of a collapsed stack, whose root row shows
 * its own node; null while `enabled` is false. Re-renders only when a count or a first branch changes.
 */
export function useStackStates(root: string, { withRoot, enabled = true }: { withRoot: boolean; enabled?: boolean }): StackStates | null {
  // a string, so the stores notify only when the states change
  const key = useNodeStores(() => {
    if (!enabled) {
      return ""
    }
    const descendants = structureIn(branchSyncStore.getState(), root).descendants
    const names = withRoot ? [root, ...descendants] : descendants
    // the node of the stack branch while the stack forks; of a straight chain it is its last row's, counted already
    const stackNode = withRoot && getStackNodeBranch(root) === null ? getStackNodeKindNow(root) : null
    return JSON.stringify({
      state: summarizeStackState(names, getNodeKindNow, stackNode),
      first: firstBranchOfEachState(names, getNodeKindNow, stackNode, root),
    } satisfies StackStates)
  })
  return useMemo(() => (key === "" ? null : (JSON.parse(key) as StackStates)), [key])
}
