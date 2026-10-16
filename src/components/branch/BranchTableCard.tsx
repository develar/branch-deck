import { lazy, Suspense, useCallback, useEffect, useMemo, useRef, useState } from "react"
import type { CSSProperties, KeyboardEvent } from "react"
import { GitBranchPlusIcon } from "lucide-react"
import { cn } from "cn"
import { Card } from "@/components/ui/card"
import { BranchFilterBar, BranchFilterEmptyRow } from "@/components/branch/BranchFilterBar"
import { BranchRow } from "@/components/branch/BranchRow"
import { BranchSelectionBar } from "@/components/branch/BranchSelectionBar"
import { IssueFamilyRow } from "@/components/branch/IssueFamilyRow"
import { BranchPatronusSection } from "@/components/branch/PatronusChip"
import { BranchPullRequestSection } from "@/components/branch/PullRequestChip"
import { InlineBranchTarget } from "@/components/shared/InlineBranchTarget"
import { TableHeader } from "@/components/shared/TableHeader"
import { useFilteredBranches, useRevealClearsFilter } from "@/hooks/useBranchFilter"
import { useBranchRowKeys } from "@/hooks/useBranchRowKeys"
import { useCommitDropTarget } from "@/hooks/useCommitDrag"
import { useRowExpansion } from "@/hooks/useRowExpansion"
import { useSpacePreviewHotkey } from "@/hooks/useSpacePreviewHotkey"
import { useTauriListen } from "@/hooks/useTauriListen"
import { pruneSelection } from "@/lib/branch-selection"
import { hasKeyModifier } from "@/lib/preview-keys"
import { landingSegments, laneCountOf } from "@/lib/stack-lanes"
import { clearBranchSelection, getSelectedBranches, setBranchTargetOpen, setNewBranchDrop, setSelectedBranches, UNASSIGNED_SOURCE, useCommitMove } from "@/stores/commit-move"
import { syncBranches, useBranchSync, useFoundCommits, useVisibleBranchTableRows } from "@/stores/branch-sync"
import { useLandingSpans } from "@/stores/patronus"

const BranchPreviewDialog = lazy(() => import("@/components/branch/BranchPreviewDialog").then(it => ({ default: it.BranchPreviewDialog })))

/** Drop target key that no list has, so commits from every list can be dropped on the "new branch" zone */
const NEW_BRANCH_TARGET = "\u0000new-branch"

/**
 * Table of active virtual branches, the branches of an issue nested under it and stacks as a graph (`StackLanes`), root
 * on top, under the filter bar (`BranchFilterBar`): the matches of the filter and, dimmed, the branches they stack on.
 * Subscribes to the row list and the matches only; each row subscribes to its own branch.
 * Space (over a branch row, on the row the arrows focused, or on the last toggled row of the focused table) opens the
 * branch preview; the focused row has its keys (`useBranchRowKeys`). Cmd/Ctrl-click and Shift-click select branch rows:
 * M or Enter opens "Move to…" for them, Cmd+A selects the rows the filter matched, Escape clears.
 */
export function BranchTableCard() {
  const visible = useFilteredBranches()
  const rows = useVisibleBranchTableRows(visible)
  const branchCount = useBranchSync(state => state.branchOrder.length)
  // the lane segments a running Safe Push lands, from the root of its stack down to its branch
  const spans = useLandingSpans()
  const landing = useMemo(() => landingSegments(rows.flatMap(row => row.kind === "branch" ? [row] : []), spans), [rows, spans])
  const { expandedKeys, toggleExpanded: toggleRowExpanded, setExpanded } = useRowExpansion()

  // the fallback of Space while the table has the focus: the row last toggled or focused with the arrows
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
  const { handleRowKey, focusFirstRow } = useBranchRowKeys({ tableRef, rows, expandedKeys, setExpanded, toggleExpanded, lastFocusedRef: lastToggledRef })
  useRevealClearsFilter(branchNames)

  // "Sync" menu item
  useTauriListen("sync-branches", () => {
    void syncBranches()
  })

  // a branch gone after a sync (renamed, merged) leaves the selection; one the filter hides stays in it
  const selectedBranches = useCommitMove(state => state.selectedBranches)
  const branchKey = useBranchSync(state => state.branchOrder.join("\n"))
  useEffect(() => {
    const pruned = pruneSelection(selectedBranches, new Set(branchKey.split("\n")))
    if (pruned !== selectedBranches) {
      setSelectedBranches(pruned)
    }
  }, [branchKey, selectedBranches])
  const hasDrop = useCommitMove(state => state.drag !== null || state.newBranchDrop !== null)
  // the lane column of the stack graph, as wide in every row (`bd-lanes` in index.css)
  const laneCount = laneCountOf(rows.map(row => row.kind === "branch" ? row.lanes : null))

  if (branchCount === 0) {
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
      // what the filter matched, not the branches shown dimmed because a match stacks on them
      setSelectedBranches(rows.flatMap(row => row.kind === "branch" && !row.dimmed ? [row.name] : []))
    }
    else if (!hasKeyModifier(event)) {
      handleRowKey(event)
    }
  }

  return (
    <Card className="gap-0 py-0">
      <BranchFilterBar onFocusRows={focusFirstRow} onLeave={() => tableRef.current?.focus()} />
      <BranchSelectionBar />
      <NewBranchDropZone />
      <div className="overflow-x-auto">
        {/* focusable, so a click on a row gives it the focus that Space falls back on */}
        <table
          ref={tableRef}
          tabIndex={-1}
          // a table the filter left empty is one quiet line, without the column heads
          className={cn("w-full outline-none", laneCount > 0 && "bd-lanes", rows.length === 0 && "[&>thead]:hidden")}
          style={laneCount > 0 ? { "--bd-lane-count": laneCount } as CSSProperties : undefined}
          onKeyDown={onTableKeyDown}
        >
          <thead className="border-b border-border">
            <tr>
              <TableHeader>Branch Name</TableHeader>
              <TableHeader className="hidden w-px px-4 whitespace-nowrap sm:table-cell">Commits</TableHeader>
              {/* room for the status text, the pull request chip and the Landed chip, so the table doesn't reflow when the chips arrive */}
              <TableHeader className="hidden w-px min-w-[12rem] px-4 whitespace-nowrap sm:table-cell">Status</TableHeader>
              <TableHeader className="w-px px-4 whitespace-nowrap">Actions</TableHeader>
            </tr>
          </thead>
          <tbody className="divide-y divide-border">
            {rows.length === 0 && <BranchFilterEmptyRow columnCount={4} />}
            {rows.map((row, rowIndex) => {
              if (row.kind === "issue") {
                return <IssueFamilyRow key={row.key} issueKey={row.issueKey} members={row.members} />
              }
              const name = row.name
              const next = rows.at(rowIndex + 1)
              return (
                <BranchRow
                  key={name}
                  name={name}
                  depth={row.depth}
                  familyIndent={row.familyIndent}
                  lanes={row.lanes}
                  landing={landing.get(name) ?? null}
                  showIssueKey={row.showIssueKey}
                  dimmed={row.dimmed}
                  familyBelow={next?.kind === "branch" && next.familyIndent > row.familyIndent}
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
