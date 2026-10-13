import { useEffect, useRef } from "react"
import { FolderInputIcon, UndoDotIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { CommitList } from "@/components/commit/CommitList"
import type { CommitListProps } from "@/components/commit/CommitList"
import { InlineBranchTarget } from "@/components/shared/InlineBranchTarget"
import type { CommitMoveList } from "@/hooks/useCommitMoveList"
import type { Commit } from "@/lib/bindings"
import { endCommitDrag } from "@/stores/commit-move"

export interface MovableCommitListProps<T extends Commit> extends Omit<CommitListProps<T>, "selection" | "contextMenuItems" | "onKeyDown" | "onDragCommits" | "onDragEnd" | "portalTarget"> {
  moveList: CommitMoveList<T>
}

/** Commit list whose commits can be selected and moved to another branch: "Move to…" form, context menu, drag. */
export function MovableCommitList<T extends Commit>({ moveList, ...listProps }: MovableCommitListProps<T>) {
  const { selection, isTargetOpen, closeTarget, sourceBranch } = moveList
  const targetRef = useRef<HTMLDivElement>(null)

  // make sure the inline form is visible
  useEffect(() => {
    const target = targetRef.current
    if (!isTargetOpen || target === null) {
      return
    }
    const rect = target.getBoundingClientRect()
    if (rect.top < 0 || rect.bottom > window.innerHeight) {
      target.scrollIntoView({ behavior: "smooth", block: "center" })
    }
  }, [isTargetOpen])

  return (
    <CommitList
      {...listProps}
      selection={selection}
      contextMenuItems={moveList.contextMenuItems}
      onKeyDown={moveList.onKeyDown}
      onDragCommits={moveList.onDragCommits}
      onDragEnd={endCommitDrag}
      portalTarget={isTargetOpen && (
        <div ref={targetRef}>
          <InlineBranchTarget
            commits={selection.selectedItems}
            sourceBranch={sourceBranch}
            isActive={isTargetOpen}
            onCancel={closeTarget}
            onSuccess={() => {
              closeTarget()
              selection.clearSelection()
            }}
          />
        </div>
      )}
    />
  )
}

/** "N selected", "Move to…" and "Move Back to ABC-1" (if a selected commit has a prefix); nothing without a selection. */
export function CommitSelectionActions<T extends Commit>({ moveList, className }: { moveList: CommitMoveList<T>, className?: string }) {
  const { selection, isTargetOpen, removePrefixLabel } = moveList
  if (selection.selectedCount === 0 || isTargetOpen) {
    return null
  }
  return (
    <div className={cn("flex items-center gap-2", className)} data-testid="selection-actions">
      <span className="text-xs text-muted-foreground">{`${selection.selectedCount} selected`}</span>
      <Button size="xs" variant="outline" className="border-primary/50 text-primary hover:bg-primary/10 hover:text-primary" onClick={() => moveList.openTarget()}>
        <FolderInputIcon data-icon="inline-start" />
        Move to…
      </Button>
      {removePrefixLabel !== null && (
        <Button size="xs" variant="ghost" onClick={moveList.removePrefix}>
          <UndoDotIcon data-icon="inline-start" />
          {removePrefixLabel}
        </Button>
      )}
    </div>
  )
}
