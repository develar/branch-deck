import { useRef, useState } from "react"
import type { DragEvent } from "react"
import type { CommitDrag } from "@/stores/commit-move"
import { endCommitDrag, useDroppableDrag } from "@/stores/commit-move"

/** `dataTransfer` type of a commit drag; the commits themselves are in the commit move store */
export const COMMIT_DRAG_TYPE = "application/x-branch-deck-commits"

/** Starts a native drag of `count` commits with a badge as the drag image: "3 commits", or `label` ("2 branches · 5 commits"). */
export function beginCommitDrag(event: DragEvent, count: number, label?: string): void {
  event.dataTransfer.effectAllowed = "move"
  event.dataTransfer.setData(COMMIT_DRAG_TYPE, String(count))
  const badge = document.createElement("div")
  badge.textContent = label ?? (count === 1 ? "1 commit" : `${count} commits`)
  // rendered off screen for the snapshot the browser takes, then removed
  badge.className = "fixed -top-24 left-0 rounded-lg bg-primary px-2 py-1 text-xs font-medium text-primary-foreground"
  document.body.append(badge)
  event.dataTransfer.setDragImage(badge, -12, -8)
  requestAnimationFrame(() => badge.remove())
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
 * string refuses it with the reason, which the target shows
 */
export type DropAcceptance = boolean | string

/**
 * Drop target for commits dragged out of a list other than `targetKey` (a branch name, or `UNASSIGNED_SOURCE`). The
 * `dropProps` may be spread on several sibling elements of one target (the rows of a branch): the enter/leave depth
 * covers them all.
 * `accept` may ignore a drag (e.g. no dragged commit has a prefix to remove), or refuse it with a reason: the pointer
 * shows no drop there, and the target tells why (`refusal`). A target inside another one stops the events it handles,
 * so the outer one never takes its drop.
 */
export function useCommitDropTarget(targetKey: string, onDrop: (drag: CommitDrag) => void, accept?: (drag: CommitDrag) => DropAcceptance): CommitDropTarget {
  const candidate = useDroppableDrag(targetKey)
  const acceptance = candidate === null ? false : accept === undefined ? true : accept(candidate)
  const drag = acceptance === false ? null : candidate
  const refusal = typeof acceptance === "string" ? acceptance : null
  const [isOver, setIsOver] = useState(false)
  // enter/leave also fire for the children of the target
  const depthRef = useRef(0)
  if (drag === null && isOver) {
    setIsOver(false)
  }

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
        onDrop(drag)
      },
    },
  }
}
