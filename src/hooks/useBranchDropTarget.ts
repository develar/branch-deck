import type { CommitDropTarget } from "@/hooks/useCommitDrag"
import { acceptanceOf, getBranchDropFacts, useCommitDropTarget, useDragStores } from "@/hooks/useCommitDrag"
import { parkBranches } from "@/hooks/useParkBranch"
import type { BranchDrop, BranchDropFacts } from "@/lib/branch-drop"
import { parkDrop, showsParkDrop, showsUnparkDrop, unparkDrop } from "@/lib/branch-drop"
import { joinNames } from "@/lib/text"
import { getCommitDrag } from "@/stores/commit-move"

/** Drop target keys that no list has, so a drag from every list may come */
const PARK_TARGET = "\u0000park"
export const UNPARK_TARGET = "\u0000unpark"
export const UNSTACK_TARGET = "\u0000unstack"

export type DecideDrop = (names: readonly string[], facts: BranchDropFacts) => BranchDrop | null

/**
 * A target of a drag of whole branches (`branch-drop.ts`): `decide` says whether it shows (null hides it), takes the
 * branches or refuses them with the reason the drag bar shows; a drop runs `act` on the branches it took. A drag of
 * commit rows passes it by.
 */
export function useBranchDropTarget(targetKey: string, decide: DecideDrop, act: (names: readonly string[]) => void): CommitDropTarget {
  return useCommitDropTarget(
    targetKey,
    drag => {
      const verdict = decide(drag.branches, getBranchDropFacts())
      if (verdict !== null && "names" in verdict) {
        act(verdict.names)
      }
    },
    drag => (drag.branches.length === 0 ? false : acceptanceOf(decide(drag.branches, getBranchDropFacts()))),
  )
}

/** The names the drop on a zone takes ("docs and api"), which its label names; empty while it takes none */
export function useTakenNames(decide: DecideDrop): string {
  return useDragStores(() => {
    const drag = getCommitDrag()
    const verdict = drag === null || drag.branches.length === 0 ? null : decide(drag.branches, getBranchDropFacts())
    return verdict !== null && "names" in verdict ? joinNames(verdict.names) : ""
  })
}

/** Scrolls the row of `name` into view where the drop put it (the Parked card, the table), without expanding it */
export function revealRow(name: string | undefined): void {
  if (name === undefined) {
    return
  }
  // after the sync of the drop rendered its new place
  requestAnimationFrame(() => document.querySelector(`tr[data-branch-name="${CSS.escape(name)}"]`)?.scrollIntoView({ behavior: "smooth", block: "nearest" }))
}

/** Whether the park target takes a drag of `names`, and on what terms (`parkDrop`) */
export function decidePark(names: readonly string[], facts: BranchDropFacts): BranchDrop | null {
  return showsParkDrop(names, facts) ? parkDrop(names, facts) : null
}

/** The park of the dragged branches, without a note: the zone of the drag bar and the whole Parked card */
export function useParkDropTarget(): CommitDropTarget {
  return useBranchDropTarget(PARK_TARGET, decidePark, names => void parkBranches(names, "").then(parked => parked && revealRow(names[0])))
}

export function decideUnpark(names: readonly string[], facts: BranchDropFacts): BranchDrop | null {
  return showsUnparkDrop(names, facts) ? unparkDrop(names, facts) : null
}
