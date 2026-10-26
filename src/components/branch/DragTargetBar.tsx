import { useEffect, useEffectEvent, useLayoutEffect, useRef } from "react"
import type { RefObject } from "react"
import { GitBranchPlusIcon } from "lucide-react"
import { DropZone, ParkDropZone, UnparkDropZone, UnstackDropZone } from "@/components/branch/BranchDropZones"
import { useCommitDropTarget } from "@/hooks/useCommitDrag"
import { commitMoveStore, endCommitDrag, setNewBranchDrop, useCommitMove } from "@/stores/commit-move"

/** Drop target key that no list has, so commits from every list can be dropped on the "new branch" zone */
const NEW_BRANCH_TARGET = "\u0000new-branch"

/**
 * The targets of a drag that change the shape of what is dragged, in one bar over the bottom of the window while a
 * drag goes on ([Move Commits](../../../spec/branch/move-commits.spec.md#drag-and-drop)): Move to New Branch, Park,
 * Unpark, Unstack, each while the drag is for it, and above them why the target under the pointer refuses the drag. Over
 * the content, out of its flow, so no row moves when a drag starts; the content gets room at its end meanwhile, so every
 * row scrolls out from under the bar. The toasts keep their column on the right.
 */
export function DragTargetBar() {
  const isDragging = useCommitMove(state => state.drag !== null)
  return isDragging ? <DragTargetBarContent /> : null
}

function DragTargetBarContent() {
  const barRef = useRef<HTMLDivElement>(null)
  const refusal = useCommitMove(state => state.hoverRefusal)
  useArming(barRef)
  useDragEndFallback()
  return (
    <>
      {/* the room at the end of the content: the last rows scroll out from under the bar */}
      <div aria-hidden className="h-32" />
      <div className="pointer-events-none fixed inset-x-0 bottom-6 z-30 flex justify-center px-4 lg:right-[26rem]">
        <div
          ref={barRef}
          data-testid="drag-target-bar"
          data-armed="true"
          className="pointer-events-auto flex max-w-full flex-col gap-2 rounded-xl bg-popover p-2 text-popover-foreground shadow-lg ring-1 ring-foreground/5 not-has-[[data-drop-target],p]:hidden data-[armed=false]:pointer-events-none"
        >
          {/* above the zones: a refusal that shows moves no zone from under the pointer */}
          {refusal !== null && (
            <p className="max-w-xl px-1 text-xs text-muted-foreground" data-testid="drag-target-status">
              {refusal}
            </p>
          )}
          <div className="flex flex-wrap gap-2">
            <NewBranchDropZone />
            <ParkDropZone />
            <UnparkDropZone />
            <UnstackDropZone />
          </div>
        </div>
      </div>
    </>
  )
}

/**
 * A drag that starts under the bar (a row at the bottom of the window) takes no drop on it until the pointer left it:
 * a short drag of that row never parks it or opens a new branch by accident (`data-armed`)
 */
function useArming(barRef: RefObject<HTMLDivElement | null>): void {
  const origin = useCommitMove(state => state.dragOrigin)
  useLayoutEffect(() => {
    const bar = barRef.current
    if (bar === null || origin === null || !isInside(bar.getBoundingClientRect(), origin.x, origin.y)) {
      return
    }
    bar.dataset.armed = "false"
    const onDrag = (event: DragEvent) => {
      if (!isInside(bar.getBoundingClientRect(), event.clientX, event.clientY)) {
        bar.dataset.armed = "true"
        stop()
      }
    }
    // in the capture phase: the targets stop the events they handle; Chromium sends no `dragover` where no target takes
    // the drag, only the `dragenter` of each element
    const types = ["dragover", "dragenter"] as const
    const stop = () => {
      for (const type of types) {
        document.removeEventListener(type, onDrag, true)
      }
    }
    for (const type of types) {
      document.addEventListener(type, onDrag, true)
    }
    return stop
  }, [barRef, origin])
}

function isInside(rect: DOMRect, x: number, y: number): boolean {
  return x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom
}

/**
 * Ends a drag whose `dragend` never came (its row unmounted, the browser dropped it outside the window): the browser
 * sends no mouse or key events during a drag, so a move with no button pressed, a press or a key comes after it
 */
function useDragEndFallback(): void {
  const onInput = useEffectEvent((event: Event) => {
    if (commitMoveStore.getState().drag !== null && (event.type !== "mousemove" || (event as MouseEvent).buttons === 0)) {
      endCommitDrag()
    }
  })
  useEffect(() => {
    const listener = (event: Event) => onInput(event)
    const types = ["mousemove", "pointerdown", "keydown"] as const
    for (const type of types) {
      document.addEventListener(type, listener)
    }
    return () => {
      for (const type of types) {
        document.removeEventListener(type, listener)
      }
    }
  }, [])
}

/** While commits or branches are dragged, and no form of a drop waits: dropping them opens the "Move to…" form above the branch table */
function NewBranchDropZone() {
  const hasForm = useCommitMove(state => state.newBranchDrop !== null)
  const target = useCommitDropTarget(NEW_BRANCH_TARGET, drag => setNewBranchDrop({ sourceKeys: drag.sourceKeys, commitIds: drag.commits.map(commit => commit.originalHash) }))
  if (hasForm) {
    return null
  }
  return <DropZone target={target} icon={GitBranchPlusIcon} label="Move to New Branch" data-testid="new-branch-drop-zone" />
}
