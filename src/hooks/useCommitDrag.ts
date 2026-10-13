import { useRef, useState } from "react"
import type { DragEvent } from "react"
import type { CommitDrag } from "@/stores/commit-move"
import { endCommitDrag, useDroppableDrag } from "@/stores/commit-move"

/** `dataTransfer` type of a commit drag; the commits themselves are in the commit move store */
export const COMMIT_DRAG_TYPE = "application/x-branch-deck-commits"

/** Starts a native drag of `count` commits with a "3 commits" badge as the drag image. */
export function beginCommitDrag(event: DragEvent, count: number): void {
  event.dataTransfer.effectAllowed = "move"
  event.dataTransfer.setData(COMMIT_DRAG_TYPE, String(count))
  const badge = document.createElement("div")
  badge.textContent = count === 1 ? "1 commit" : `${count} commits`
  // rendered off screen for the snapshot the browser takes, then removed
  badge.className = "fixed -top-24 left-0 rounded-md bg-primary px-2 py-1 text-xs font-medium text-primary-foreground"
  document.body.append(badge)
  event.dataTransfer.setDragImage(badge, -12, -8)
  requestAnimationFrame(() => badge.remove())
}

export interface CommitDropTarget {
  /** Commits from another list are being dragged */
  canDrop: boolean
  /** They are over this target */
  isOver: boolean
  /** Spread on the target element; empty while nothing droppable is dragged */
  dropProps: {
    onDragEnter?: (event: DragEvent) => void
    onDragOver?: (event: DragEvent) => void
    onDragLeave?: (event: DragEvent) => void
    onDrop?: (event: DragEvent) => void
  }
}

/**
 * Drop target for commits dragged out of a list other than `targetKey` (a branch name, or `UNASSIGNED_SOURCE`).
 * `accept` may refuse a drag (e.g. no dragged commit has a prefix to remove).
 */
export function useCommitDropTarget(targetKey: string, onDrop: (drag: CommitDrag) => void, accept?: (drag: CommitDrag) => boolean): CommitDropTarget {
  const candidate = useDroppableDrag(targetKey)
  const drag = candidate !== null && (accept === undefined || accept(candidate)) ? candidate : null
  const [isOver, setIsOver] = useState(false)
  // enter/leave also fire for the children of the target
  const depthRef = useRef(0)
  if (drag === null && isOver) {
    setIsOver(false)
  }

  if (drag === null) {
    return { canDrop: false, isOver: false, dropProps: {} }
  }
  return {
    canDrop: true,
    isOver,
    dropProps: {
      onDragEnter: (event) => {
        event.preventDefault()
        // a drag that ended elsewhere leaves a stale depth
        depthRef.current = isOver ? depthRef.current + 1 : 1
        setIsOver(true)
      },
      onDragOver: (event) => {
        event.preventDefault()
        event.dataTransfer.dropEffect = "move"
      },
      onDragLeave: () => {
        depthRef.current = Math.max(0, depthRef.current - 1)
        if (depthRef.current === 0) {
          setIsOver(false)
        }
      },
      onDrop: (event) => {
        event.preventDefault()
        depthRef.current = 0
        setIsOver(false)
        endCommitDrag()
        onDrop(drag)
      },
    },
  }
}
