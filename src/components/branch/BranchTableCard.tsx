import { lazy, Suspense, useCallback, useEffect, useEffectEvent, useLayoutEffect, useMemo, useRef, useState } from "react"
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
import { useStackPathPointer } from "@/hooks/useStackPathPointer"
import { useTauriListen } from "@/hooks/useTauriListen"
import { pruneSelection } from "@/lib/branch-selection"
import { hasKeyModifier, KEY_OWNERS } from "@/lib/preview-keys"
import { collapsedLanes, isStackRoot, stackSizes } from "@/lib/stack-collapse"
import { landingSegments, laneCountOf } from "@/lib/stack-lanes"
import { clearBranchSelection, getSelectedBranches, setBranchTargetOpen, setNewBranchDrop, setSelectedBranches, UNASSIGNED_SOURCE, useCommitMove } from "@/stores/commit-move"
import { syncBranches, useBranchSync, useFoundCommits } from "@/stores/branch-sync"
import { useInlineRowAction } from "@/stores/inline-row-action"
import { useLandingSpans } from "@/stores/patronus"
import { expandStackOf, useShownBranchTableRows } from "@/stores/stack-collapse"
import { setStackPathRows } from "@/stores/stack-view"

const BranchPreviewDialog = lazy(() => import("@/components/branch/BranchPreviewDialog").then(it => ({ default: it.BranchPreviewDialog })))

/** Drop target key that no list has, so commits from every list can be dropped on the "new branch" zone */
const NEW_BRANCH_TARGET = "\u0000new-branch"

/**
 * Table of active virtual branches, the branches of an issue nested under it and stacks as a graph (`StackLanes`), root
 * on top, under the filter bar (`BranchFilterBar`): the matches of the filter and, dimmed, the branches they stack on.
 * Hovering or focusing a branch highlights the path from the root of its stack down to it (`useStackPathPointer`); the
 * root of a stack heads it (`StackChip`), and a collapsed stack shows as its root alone (`useShownBranchTableRows`).
 * Subscribes to the row list and the matches only; each row subscribes to its own branch.
 * Space (over a branch row, on the row the arrows focused, or on the last toggled row of the focused table) opens the
 * branch preview; the focused row has its keys (`useBranchRowKeys`, M opens "Move to…" for it). Cmd/Ctrl-click and
 * Shift-click select branch rows: M or Enter opens "Move to…" for them, Cmd+A selects the rows the filter matched, Escape
 * clears.
 */
export function BranchTableCard() {
  const visible = useFilteredBranches()
  // without a filter, a collapsed stack shows as its root alone
  const { rows, all: unfolded, hidden } = useShownBranchTableRows(visible)
  const stackShown = useMemo(() => stackSizes(rows), [rows])
  const branchCount = useBranchSync(state => state.branchOrder.length)
  // the lane segments a running Safe Push lands, from the root of its stack down to its branch
  const spans = useLandingSpans()
  const laneRows = useMemo(() => rows.flatMap(row => row.kind === "branch" ? [row] : []), [rows])
  const landing = useMemo(() => landingSegments(laneRows, spans), [laneRows, spans])
  // the highlighted path to the root of the hovered or focused branch is laid out over the rows before they paint
  useLayoutEffect(() => setStackPathRows(laneRows), [laneRows])
  const pathPointer = useStackPathPointer()
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
  const unfoldedNames = useMemo(() => unfolded.flatMap(row => row.kind === "branch" ? [row.name] : []), [unfolded])
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
  const { handleRowKey, handleStackKey, focusFirstRow, focusAttentionRow } = useBranchRowKeys({ tableRef, rows, expandedKeys, setExpanded, toggleExpanded, lastFocusedRef: lastToggledRef, unfoldedNames })
  useAttentionKey(focusAttentionRow)
  // a reveal of a branch the filter hides clears it; one in a collapsed stack, or an inline form opened there, expands it
  useRevealClearsFilter(unfoldedNames)
  const revealTarget = useCommitMove(state => state.reveal?.branchName ?? null)
  const inlineTarget = useInlineRowAction(state => state.activeInline?.branchName ?? null)
  const shownKey = branchNames.join("\n")
  useEffect(() => {
    const shown = new Set(shownKey.split("\n"))
    for (const name of [revealTarget, inlineTarget]) {
      if (name !== null && !shown.has(name)) {
        expandStackOf(name)
      }
    }
  }, [revealTarget, inlineTarget, shownKey])

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
  // of the rows before a collapse, so the names don't move when a stack that forks collapses
  const laneCount = laneCountOf(unfolded.map(row => row.kind === "branch" ? row.lanes : null))

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
    else if ((event.key === "ArrowLeft" || event.key === "ArrowRight") && event.altKey && !event.metaKey && !event.ctrlKey && !event.shiftKey) {
      handleStackKey(event)
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
          <tbody className="bd-branch-rows" {...pathPointer}>
            {rows.length === 0 && <BranchFilterEmptyRow columnCount={4} />}
            {rows.map((row, rowIndex) => {
              if (row.kind === "issue") {
                return <IssueFamilyRow key={row.key} issueKey={row.issueKey} members={row.members} />
              }
              const name = row.name
              const next = rows.at(rowIndex + 1)
              const collapsed = hidden.has(name)
              return (
                <BranchRow
                  key={name}
                  name={name}
                  depth={row.depth}
                  familyIndent={row.familyIndent}
                  lanes={row.lanes !== null && collapsed ? collapsedLanes(row.lanes) : row.lanes}
                  stackShown={isStackRoot(row) ? (stackShown.get(name) ?? 1) : undefined}
                  stackCollapsed={collapsed}
                  stackFiltered={visible !== null}
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
/**
 * F2 and Shift+F2 from anywhere in the window but a text field, a dialog or a menu: the focus moves to the next or the
 * previous branch row that waits for the user
 */
function useAttentionKey(focusAttentionRow: (step: 1 | -1) => void): void {
  const onKeyDown = useEffectEvent((event: globalThis.KeyboardEvent) => {
    if (event.key !== "F2" || event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey) {
      return
    }
    if (event.target instanceof Element && event.target.closest(KEY_OWNERS) !== null) {
      return
    }
    event.preventDefault()
    focusAttentionRow(event.shiftKey ? -1 : 1)
  })
  useEffect(() => {
    const listener = (event: globalThis.KeyboardEvent) => onKeyDown(event)
    document.addEventListener("keydown", listener)
    return () => document.removeEventListener("keydown", listener)
  }, [])
}

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
