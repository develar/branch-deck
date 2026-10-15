import { lazy, Suspense, useCallback, useEffect, useRef, useState } from "react"
import type { KeyboardEvent } from "react"
import { GitBranchPlusIcon } from "lucide-react"
import { cn } from "cn"
import { Card } from "@/components/ui/card"
import { BranchRow } from "@/components/branch/BranchRow"
import { BranchSelectionBar } from "@/components/branch/BranchSelectionBar"
import { IssueFamilyRow } from "@/components/branch/IssueFamilyRow"
import { BranchPatronusSection } from "@/components/branch/PatronusChip"
import { BranchPullRequestSection } from "@/components/branch/PullRequestChip"
import { InlineBranchTarget } from "@/components/shared/InlineBranchTarget"
import { TableHeader } from "@/components/shared/TableHeader"
import { useCommitDropTarget } from "@/hooks/useCommitDrag"
import { useRowExpansion } from "@/hooks/useRowExpansion"
import { useSpacePreviewHotkey } from "@/hooks/useSpacePreviewHotkey"
import { useTauriListen } from "@/hooks/useTauriListen"
import { pruneSelection } from "@/lib/branch-selection"
import { hasKeyModifier } from "@/lib/preview-keys"
import { clearBranchSelection, getSelectedBranches, setBranchTargetOpen, setNewBranchDrop, setSelectedBranches, UNASSIGNED_SOURCE, useCommitMove } from "@/stores/commit-move"
import { syncBranches, useBranchTableRows, useFoundCommits } from "@/stores/branch-sync"

const BranchPreviewDialog = lazy(() => import("@/components/branch/BranchPreviewDialog").then(it => ({ default: it.BranchPreviewDialog })))

/** Drop target key that no list has, so commits from every list can be dropped on the "new branch" zone */
const NEW_BRANCH_TARGET = "\u0000new-branch"

/**
 * Table of active virtual branches, the branches of an issue nested under it and stacked branches under their parents. Subscribes to the row list only; each
 * row subscribes to its own branch.
 * Space (over a branch row, or on the last toggled row of the focused table) opens the branch preview. Cmd/Ctrl-click
 * and Shift-click select branch rows: M or Enter opens "Move to…" for them, Cmd+A selects all, Escape clears.
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
  const branchNames = rows.flatMap(row => row.kind === "branch" ? [row.name] : [])
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

  // a branch gone after a sync (renamed, merged) leaves the selection
  const selectedBranches = useCommitMove(state => state.selectedBranches)
  const branchKey = branchNames.join("\n")
  useEffect(() => {
    const pruned = pruneSelection(selectedBranches, new Set(branchKey.split("\n")))
    if (pruned !== selectedBranches) {
      setSelectedBranches(pruned)
    }
  }, [branchKey, selectedBranches])
  const hasDrop = useCommitMove(state => state.drag !== null || state.newBranchDrop !== null)

  if (rows.length === 0) {
    // with no branch yet, the zone is where dragged commits go
    return hasDrop
      ? (
          <Card className="gap-0 py-0">
            <NewBranchDropZone />
          </Card>
        )
      : null
  }

  const onTableKeyDown = (event: KeyboardEvent<HTMLTableElement>) => {
    // the keys of the commit lists in the expanded rows are theirs
    const target = event.target as HTMLElement
    if (event.defaultPrevented || (target !== event.currentTarget && target.closest("[data-testid=branch-row]") === null)) {
      return
    }
    const hasSelection = getSelectedBranches().length > 0
    if (event.key === "Escape" && hasSelection) {
      event.preventDefault()
      clearBranchSelection()
    }
    else if (hasSelection && (event.key === "Enter" || (event.code === "KeyM" && !hasKeyModifier(event)))) {
      event.preventDefault()
      setBranchTargetOpen(true)
    }
    else if (event.code === "KeyA" && (event.metaKey || event.ctrlKey) && !event.altKey && !event.shiftKey) {
      event.preventDefault()
      setSelectedBranches(branchNames)
    }
  }

  return (
    <Card className="gap-0 py-0">
      <BranchSelectionBar />
      <NewBranchDropZone />
      <div className="overflow-x-auto">
        {/* focusable, so a click on a row gives it the focus that Space falls back on */}
        <table ref={tableRef} tabIndex={-1} className="w-full outline-none" onKeyDown={onTableKeyDown}>
          <thead className="border-b border-border">
            <tr>
              <TableHeader>Branch Name</TableHeader>
              <TableHeader className="hidden w-px px-4 whitespace-nowrap sm:table-cell">Commits</TableHeader>
              {/* room for the status text and a chip or two, so the table doesn't reflow when the chips arrive */}
              <TableHeader className="hidden w-px min-w-[16rem] px-4 whitespace-nowrap sm:table-cell">Status</TableHeader>
              <TableHeader className="w-px px-4 whitespace-nowrap">Actions</TableHeader>
            </tr>
          </thead>
          <tbody className="divide-y divide-border">
            {rows.map((row, rowIndex) => {
              if (row.kind === "issue") {
                return <IssueFamilyRow key={row.key} issueKey={row.issueKey} branchCount={row.branchCount} />
              }
              const name = row.name
              const next = rows.at(rowIndex + 1)
              return (
                <BranchRow
                  key={name}
                  name={name}
                  depth={row.depth}
                  showIssueKey={row.showIssueKey}
                  hasChildren={next?.kind === "branch" && next.depth > row.depth}
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
          <BranchPreviewDialog
            names={branchNames}
            name={previewName}
            onNameChange={setPreviewName}
            onClose={() => setPreviewName(null)}
            renderReviewSections={branch => (
              <>
                <BranchPullRequestSection branchName={branch.name} />
                <BranchPatronusSection branchName={branch.name} />
              </>
            )}
          />
        </Suspense>
      )}
    </Card>
  )
}

/**
 * Shown while commits or branches are dragged: dropping them opens the "Move to…" form for a new branch here. The form
 * keeps the ids of the dropped commits and closes when a rewrite gave one of them another id.
 */
function NewBranchDropZone() {
  const dropped = useCommitMove(state => state.newBranchDrop)
  const dropTarget = useCommitDropTarget(NEW_BRANCH_TARGET, drag => setNewBranchDrop({ sourceKeys: drag.sourceKeys, commitIds: drag.commits.map(commit => commit.originalHash) }))
  const commits = useFoundCommits(dropped?.commitIds ?? [])
  const isGone = dropped !== null && commits === null
  useEffect(() => {
    if (isGone) {
      setNewBranchDrop(null)
    }
  }, [isGone])

  if (dropped !== null && commits !== null) {
    return (
      <div className="border-b border-border">
        <InlineBranchTarget
          commits={commits}
          sourceBranches={dropped.sourceKeys.filter(key => key !== UNASSIGNED_SOURCE)}
          isActive
          onCancel={() => setNewBranchDrop(null)}
          onSuccess={() => {
            setNewBranchDrop(null)
            clearBranchSelection()
          }}
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
