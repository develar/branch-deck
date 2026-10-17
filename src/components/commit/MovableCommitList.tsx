import { lazy, Suspense, useEffect, useRef, useState } from "react"
import { cn } from "cn"
import { CombineIcon, FolderInputIcon, UndoDotIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { CommitList } from "@/components/commit/CommitList"
import type { CommitListProps } from "@/components/commit/CommitList"
import { InlineBranchTarget } from "@/components/shared/InlineBranchTarget"
import type { CommitMoveList } from "@/hooks/useCommitMoveList"
import { hasSelectionActions } from "@/hooks/useCommitMoveList"
import type { Commit } from "@/lib/bindings"
import { endCommitDrag } from "@/stores/commit-move"

const SquashCommitsDialog = lazy(() => import("@/components/commit/SquashCommitsDialog").then(it => ({ default: it.SquashCommitsDialog })))

export interface MovableCommitListProps<T extends Commit> extends Omit<
  CommitListProps<T>,
  "selection" | "contextMenuItems" | "onKeyDown" | "onDragCommits" | "onDragEnd" | "portalTarget"
> {
  moveList: CommitMoveList<T>
}

/**
 * Commit list whose commits can be selected, moved to another branch ("Move to…" form, context menu, drag) and
 * squashed.
 */
export function MovableCommitList<T extends Commit>({ moveList, ...listProps }: MovableCommitListProps<T>) {
  const { selection, isTargetOpen, closeTarget, sourceBranch, squashCommits, closeSquash } = moveList
  const targetRef = useRef<HTMLDivElement>(null)

  // loaded once two commits are selected and kept mounted, so the dialog opens (and closes) like a mounted one
  const [isSquashMounted, setIsSquashMounted] = useState(false)
  if (!isSquashMounted && (selection.selectedCount > 1 || squashCommits !== null)) {
    setIsSquashMounted(true)
  }

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
    <>
      <CommitList
        listLabel={sourceBranch ?? "Unassigned"}
        {...listProps}
        selection={selection}
        contextMenuItems={moveList.contextMenuItems}
        onKeyDown={moveList.onKeyDown}
        onDragCommits={moveList.onDragCommits}
        onDragEnd={endCommitDrag}
        portalTarget={
          isTargetOpen && (
            <div ref={targetRef}>
              <InlineBranchTarget
                commits={selection.selectedItems}
                sourceBranches={sourceBranch === null ? [] : [sourceBranch]}
                isActive={isTargetOpen}
                onCancel={closeTarget}
                onSuccess={() => {
                  closeTarget()
                  selection.clearSelection()
                }}
              />
            </div>
          )
        }
      />
      {isSquashMounted && (
        <Suspense fallback={null}>
          <SquashCommitsDialog
            commits={squashCommits}
            branchName={sourceBranch}
            onClose={closeSquash}
            onSquashed={() => {
              closeSquash()
              selection.clearSelection()
            }}
          />
        </Suspense>
      )}
    </>
  )
}

/**
 * "N selected", "Move to…", "Squash" (for two commits or more) and "Move Back to ABC-1" (if a selected commit has a
 * prefix); nothing without a selection.
 */
export function CommitSelectionActions<T extends Commit>({ moveList, className }: { moveList: CommitMoveList<T>; className?: string }) {
  const { selection, removePrefixLabel } = moveList
  if (!hasSelectionActions(moveList)) {
    return null
  }
  return (
    <div className={cn("flex items-center gap-2", className)} data-testid="selection-actions">
      <span className="text-xs text-muted-foreground">{`${selection.selectedCount} selected`}</span>
      <Button size="xs" variant="outline" className="border-primary/50 text-primary hover:bg-primary/10 hover:text-primary" onClick={() => moveList.openTarget()}>
        <FolderInputIcon data-icon="inline-start" />
        Move to…
      </Button>
      {selection.selectedCount > 1 && (
        <Button size="xs" variant="ghost" onClick={() => moveList.openSquash()}>
          <CombineIcon data-icon="inline-start" />
          Squash
        </Button>
      )}
      {removePrefixLabel !== null && (
        <Button size="xs" variant="ghost" onClick={moveList.removePrefix}>
          <UndoDotIcon data-icon="inline-start" />
          {removePrefixLabel}
        </Button>
      )}
    </div>
  )
}
