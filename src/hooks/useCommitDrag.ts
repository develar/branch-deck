import { useEffect, useRef, useState } from "react"
import type { DragEvent } from "react"
import { useStore } from "zustand"
import { toast } from "@/components/ui/toast"
import type { BranchDrop, BranchDropFacts } from "@/lib/branch-drop"
import { getRowPart } from "@/model/store"
import { actionBlocker } from "@/presenters/actions"
import { branchSyncStore, getBranch, getBranchCommits, getParkedState, getSessionOperations, getStackDescendants, isStacksEnabled, resolveCommits } from "@/stores/branch-sync"
import type { CommitDrag } from "@/stores/commit-move"
import { clearHoverRefusal, commitMoveStore, endCommitDrag, isDroppableOn, setDragOrigin, setHoverRefusal, useDroppableDrag } from "@/stores/commit-move"
import { indicatorsStore } from "@/stores/indicators"
import { patronusStore } from "@/stores/patronus"

/** `dataTransfer` type of a commit drag; the commits themselves are in the commit move store */
export const COMMIT_DRAG_TYPE = "application/x-branch-deck-commits"

/**
 * Starts a native drag of `count` commits with a badge as the drag image: "3 commits", or `label` ("docs · 5 commits",
 * "2 branches · 5 commits"). The drag ends with the `dragend` of the row it started on, also when a sync remounted
 * the row meanwhile, which React no longer hears.
 */
export function beginCommitDrag(event: DragEvent, count: number, label?: string): void {
  event.dataTransfer.effectAllowed = "move"
  event.dataTransfer.setData(COMMIT_DRAG_TYPE, String(count))
  const badge = document.createElement("div")
  badge.textContent = label ?? (count === 1 ? "1 commit" : `${count} commits`)
  // in view at the pointer for the snapshot the browser takes (WebKit takes an element off screen blank), then removed
  badge.className = "pointer-events-none fixed z-100 rounded-lg bg-primary px-2 py-1 text-xs font-medium text-primary-foreground"
  badge.style.left = `${event.clientX + 12}px`
  badge.style.top = `${event.clientY + 8}px`
  document.body.append(badge)
  event.dataTransfer.setDragImage(badge, -12, -8)
  // WebKit may hold the frames of the page while the drag goes on: the timer removes it then
  const remove = () => badge.remove()
  requestAnimationFrame(remove)
  setTimeout(remove, 100)
  event.currentTarget.addEventListener("dragend", () => endCommitDrag(), { once: true })
  setDragOrigin({ x: event.clientX, y: event.clientY })
}

export interface CommitDropTarget {
  /** Commits from another list are being dragged, and this target takes them */
  canDrop: boolean
  /** They are over this target, also when it refuses them */
  isOver: boolean
  /** Why this target refuses the drag that it shows for ("Cannot park auth: …"); null otherwise */
  refusal: string | null
  /** Spread on the target element; empty while nothing droppable is dragged */
  dropProps: {
    onDragEnter?: (event: DragEvent) => void
    onDragOver?: (event: DragEvent) => void
    onDragLeave?: (event: DragEvent) => void
    onDrop?: (event: DragEvent) => void
  }
}

/**
 * What a target says of a drag (`useCommitDropTarget`): `true` takes it, `false` ignores it (the target may hide), a
 * string refuses it with the reason, which the drag bar shows
 */
export type DropAcceptance = boolean | string

/** What a drop decides by (`branch-drop.ts`), as the stores have it now; read inside `useDragStores` or a handler */
export function getBranchDropFacts(): BranchDropFacts {
  return {
    branch: getBranch,
    parked: getParkedState,
    blocker: (name, id) => actionBlocker(getRowPart(name, "actions"), id, { subject: name, statusText: getBranch(name)?.statusText ?? "", operations: getSessionOperations() }),
    descendants: getStackDescendants,
    stacksEnabled: isStacksEnabled(),
  }
}

/** The answer of a target as `branch-drop.ts` gives it: null ignores the drag */
export function acceptanceOf(verdict: BranchDrop | null): DropAcceptance {
  return verdict === null ? false : "refusal" in verdict ? verdict.refusal : true
}

/** `data-drop-target` of a target: it takes the drag ("ready", "over"), refuses it ("refused"), or no drag is for it */
export function dropTargetState(target: CommitDropTarget): "ready" | "over" | "refused" | undefined {
  if (target.refusal !== null) {
    return "refused"
  }
  return target.canDrop ? (target.isOver ? "over" : "ready") : undefined
}

/**
 * Renders again with `select` when a store that a drop target decides by changes: the drag, the branches and the
 * operations of the view, the robots, the busy states this window gave its operations. `select` reads them with
 * `getState()` and returns a primitive.
 */
export function useDragStores<T>(select: () => T): T {
  useStore(branchSyncStore, select)
  useStore(patronusStore, select)
  useStore(indicatorsStore, select)
  return useStore(commitMoveStore, select)
}

/**
 * The drag with its commits as the last sync left them: a sync since its start may have given them other ids, or a
 * rename took a branch; the reason when they are gone
 */
function refreshDrag(drag: CommitDrag): CommitDrag | string {
  if (drag.branches.length > 0) {
    const gone = drag.branches.find(name => getBranch(name) === undefined)
    return gone === undefined ? { ...drag, commits: getBranchCommits(drag.branches) } : `Cannot move ${gone}: The branch is gone`
  }
  const commits = resolveCommits(drag.commits)
  return commits === null ? "Cannot move the commits: They changed, drag them again" : { ...drag, commits }
}

/**
 * Drop target for commits dragged out of a list other than `targetKey` (a branch name, or `UNASSIGNED_SOURCE`). The
 * `dropProps` may be spread on several sibling elements of one target (the rows of a branch): the enter/leave depth
 * covers them all.
 * `accept` may ignore a drag (e.g. no dragged commit has a prefix to remove), or refuse it with a reason: the pointer
 * shows no drop there, and the drag bar tells why while the pointer is over it. It reads the stores with `getState()`
 * and is asked again whenever one of them changes (`useDragStores`), so a sync or a rewrite that starts or ends during
 * the drag changes the answer; a drop asks it once more and refreshes the commits (`refreshDrag`). A target inside
 * another one stops the events it handles, so the outer one never takes its drop.
 */
export function useCommitDropTarget(targetKey: string, onDrop: (drag: CommitDrag) => void, accept?: (drag: CommitDrag) => DropAcceptance): CommitDropTarget {
  const candidate = useDroppableDrag(targetKey)
  const acceptance = useDragStores(() => {
    const current = commitMoveStore.getState().drag
    if (current === null || !isDroppableOn(current, targetKey)) {
      return false
    }
    return accept === undefined ? true : accept(current)
  })
  const drag = acceptance === false ? null : candidate
  const refusal = typeof acceptance === "string" ? acceptance : null
  const [isOver, setIsOver] = useState(false)
  // enter/leave also fire for the children of the target
  const depthRef = useRef(0)
  if (drag === null && isOver) {
    setIsOver(false)
  }
  // the drag bar says why the target under the pointer refuses the drag
  const shownRefusal = isOver ? refusal : null
  useEffect(() => {
    if (shownRefusal === null) {
      return
    }
    setHoverRefusal(shownRefusal)
    return () => clearHoverRefusal(shownRefusal)
  }, [shownRefusal])

  if (drag === null) {
    return { canDrop: false, isOver: false, refusal: null, dropProps: {} }
  }
  const enter = (event: DragEvent) => {
    event.stopPropagation()
    // a drag that ended elsewhere leaves a stale depth
    depthRef.current = isOver ? depthRef.current + 1 : 1
    setIsOver(true)
  }
  const leave = (event: DragEvent) => {
    event.stopPropagation()
    depthRef.current = Math.max(0, depthRef.current - 1)
    if (depthRef.current === 0) {
      setIsOver(false)
    }
  }
  if (refusal !== null) {
    return {
      canDrop: false,
      isOver,
      refusal,
      dropProps: {
        onDragEnter: enter,
        // no `preventDefault`: the browser shows that nothing drops here
        onDragOver: event => {
          event.stopPropagation()
          event.dataTransfer.dropEffect = "none"
        },
        onDragLeave: leave,
      },
    }
  }
  return {
    canDrop: true,
    isOver,
    refusal: null,
    dropProps: {
      onDragEnter: event => {
        event.preventDefault()
        enter(event)
      },
      onDragOver: event => {
        event.preventDefault()
        event.stopPropagation()
        event.dataTransfer.dropEffect = "move"
      },
      onDragLeave: leave,
      onDrop: event => {
        event.preventDefault()
        event.stopPropagation()
        depthRef.current = 0
        setIsOver(false)
        endCommitDrag()
        // what the answer decided by may have changed since the last render (a rewrite started a moment ago)
        const verdict = accept === undefined ? true : accept(drag)
        const fresh = verdict === true ? refreshDrag(drag) : verdict
        if (typeof fresh === "string") {
          toast.add({ type: "info", title: fresh })
        } else if (fresh !== false) {
          onDrop(fresh)
        }
      },
    },
  }
}
