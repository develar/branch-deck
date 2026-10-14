import { lazy, Suspense, useCallback, useRef, useState } from "react"
import { GitBranchPlusIcon } from "lucide-react"
import { cn } from "cn"
import { Card } from "@/components/ui/card"
import { BranchRow } from "@/components/branch/BranchRow"
import { IssueFamilyRow } from "@/components/branch/IssueFamilyRow"
import { InlineBranchTarget } from "@/components/shared/InlineBranchTarget"
import { TableHeader } from "@/components/shared/TableHeader"
import { useCommitDropTarget } from "@/hooks/useCommitDrag"
import { useRowExpansion } from "@/hooks/useRowExpansion"
import { useSpacePreviewHotkey } from "@/hooks/useSpacePreviewHotkey"
import { useTauriListen } from "@/hooks/useTauriListen"
import type { CommitDrag } from "@/stores/commit-move"
import { UNASSIGNED_SOURCE } from "@/stores/commit-move"
import { syncBranches, useBranchTableRows } from "@/stores/branch-sync"

const BranchPreviewDialog = lazy(() => import("@/components/branch/BranchPreviewDialog").then(it => ({ default: it.BranchPreviewDialog })))

/** Drop target key that no list has, so commits from every list can be dropped on the "new branch" zone */
const NEW_BRANCH_TARGET = "\u0000new-branch"

/**
 * Table of active virtual branches, the branches of an issue nested under it. Subscribes to the row list only; each
 * row subscribes to its own branch.
 * Space (over a branch row, or on the last toggled row of the focused table) opens the branch preview.
 */
export function BranchTableCard() {
  const rows = useBranchTableRows()
  const { expandedKeys, toggleExpanded: toggleRowExpanded, setExpanded } = useRowExpansion()

  // the fallback of Space while the table has the focus
  const lastToggledRef = useRef<string | null>(null)
  const toggleExpanded = useCallback((name: string) => {
    lastToggledRef.current = name
    toggleRowExpanded(name)
  }, [toggleRowExpanded])

  // the preview follows the branch by name across a sync; a branch gone (renamed, merged) closes it
  const [previewName, setPreviewName] = useState<string | null>(null)
  // loaded on the first use, then kept mounted (the dialog moves the focus only when it opens or closes)
  const [isPreviewLoaded, setIsPreviewLoaded] = useState(false)
  const branchNames = rows.filter(row => !row.startsWith("i:")).map(row => row.slice(2))
  if (previewName !== null && !branchNames.includes(previewName)) {
    setPreviewName(null)
  }
  const openPreview = useCallback((name: string) => {
    // an open context menu keeps its keys
    if (document.querySelector("[role=menu]") === null) {
      setPreviewName(name)
      setIsPreviewLoaded(true)
    }
  }, [])
  const tableRef = useRef<HTMLTableElement>(null)
  useSpacePreviewHotkey(tableRef, () => lastToggledRef.current, openPreview)

  // "Sync" menu item
  useTauriListen("sync-branches", () => {
    void syncBranches()
  })

  if (rows.length === 0) {
    return null
  }

  return (
    <Card className="gap-0 py-0">
      <NewBranchDropZone />
      <div className="overflow-x-auto">
        {/* focusable, so a click on a row gives it the focus that Space falls back on */}
        <table ref={tableRef} tabIndex={-1} className="w-full outline-none">
          <thead className="border-b border-border bg-muted/50">
            <tr>
              <TableHeader>Branch Name</TableHeader>
              <TableHeader className="hidden w-px px-4 whitespace-nowrap sm:table-cell">Commits</TableHeader>
              <TableHeader className="hidden w-px px-4 whitespace-nowrap sm:table-cell">Status</TableHeader>
              <TableHeader className="w-px px-4 whitespace-nowrap">Actions</TableHeader>
            </tr>
          </thead>
          <tbody className="divide-y divide-border">
            {rows.map((row) => {
              const name = row.slice(2)
              if (row.startsWith("i:")) {
                const separator = name.indexOf(":")
                return <IssueFamilyRow key={row} issueKey={name.slice(separator + 1)} branchCount={Number(name.slice(0, separator))} />
              }
              return (
                <BranchRow
                  key={name}
                  name={name}
                  nested={row.startsWith("n:")}
                  showIssueKey={row.startsWith("c:")}
                  expanded={expandedKeys.has(name)}
                  onToggleExpanded={toggleExpanded}
                  setExpanded={setExpanded}
                />
              )
            })}
          </tbody>
        </table>
      </div>
      {isPreviewLoaded && (
        <Suspense fallback={null}>
          <BranchPreviewDialog names={branchNames} name={previewName} onNameChange={setPreviewName} onClose={() => setPreviewName(null)} />
        </Suspense>
      )}
    </Card>
  )
}

/** Shown while commits are dragged: dropping them opens the "Move to…" form for a new branch here. */
function NewBranchDropZone() {
  const [dropped, setDropped] = useState<CommitDrag | null>(null)
  const dropTarget = useCommitDropTarget(NEW_BRANCH_TARGET, setDropped)

  if (dropped !== null) {
    return (
      <div className="border-b border-border">
        <InlineBranchTarget
          commits={dropped.commits}
          sourceBranch={dropped.sourceKey === UNASSIGNED_SOURCE ? null : dropped.sourceKey}
          isActive
          onCancel={() => setDropped(null)}
          onSuccess={() => setDropped(null)}
        />
      </div>
    )
  }
  if (!dropTarget.canDrop) {
    return null
  }
  return (
    <div className="p-3" {...dropTarget.dropProps}>
      <div
        data-testid="new-branch-drop-zone"
        data-drop-target={dropTarget.isOver ? "over" : "ready"}
        className={cn(
          "flex items-center justify-center gap-2 rounded-lg border border-dashed border-primary/50 py-4 text-sm text-primary",
          dropTarget.isOver && "border-primary bg-primary/10",
        )}
      >
        <GitBranchPlusIcon className="size-4" />
        Drop here to move the commits to a new branch
      </div>
    </div>
  )
}
