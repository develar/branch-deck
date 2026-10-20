import { lazy, Suspense, useCallback, useEffect, useEffectEvent, useLayoutEffect, useMemo, useRef, useState } from "react"
import type { CSSProperties, KeyboardEvent, ReactNode, RefObject } from "react"
import { cn } from "cn"
import { GitBranchPlusIcon } from "lucide-react"
import { Card } from "@/components/ui/card"
import { BranchColumnsRoot, ColumnLabel, ColumnTrack } from "@/components/branch/BranchColumns"
import { DropZone, ParkDropCard, UnparkDropZone, UnstackDropZone } from "@/components/branch/BranchDropZones"
import { BranchFilterBar, BranchFilterEmptyRow } from "@/components/branch/BranchFilterBar"
import { BranchGraphHelpPopover } from "@/components/branch/BranchGraphHelpPopover"
import { BranchRow } from "@/components/branch/BranchRow"
import { BranchSelectionBar } from "@/components/branch/BranchSelectionBar"
import { IssueFamilyRow } from "@/components/branch/IssueFamilyRow"
import { ParkedBranchesCard } from "@/components/branch/ParkedBranchesCard"
import { BranchPatronusSection } from "@/components/branch/PatronusChip"
import { BranchPullRequestSection } from "@/components/branch/PullRequestChip"
import { StackIslandCard } from "@/components/branch/StackIslandCard"
import { InlineBranchTarget } from "@/components/shared/InlineBranchTarget"
import { TableHeader } from "@/components/shared/TableHeader"
import { useFilteredBranches, useRevealClearsFilter } from "@/hooks/useBranchFilter"
import { prefetchBranchRowReview, reviewBranchRow, useBranchRowKeys } from "@/hooks/useBranchRowKeys"
import { useCommitDropTarget } from "@/hooks/useCommitDrag"
import { usePreviewHotkeys } from "@/hooks/usePreviewHotkeys"
import type { PreviewHotkeys } from "@/hooks/usePreviewHotkeys"
import { useRowExpansion } from "@/hooks/useRowExpansion"
import { useStackPathPointer } from "@/hooks/useStackPathPointer"
import { pruneSelection } from "@/lib/branch-selection"
import { copyText, isCopyKey } from "@/lib/clipboard"
import { hasKeyModifier, KEY_OWNERS } from "@/lib/preview-keys"
import { collapsedLanes, isStackRoot, sideStackSizes, stackSizes } from "@/lib/stack-collapse"
import { landingSegments, laneCountOf } from "@/lib/stack-lanes"
import { closeBranchPreview, openBranchPreview, showBranchInPreview, useBranchPreview } from "@/stores/branch-preview"
import type { BranchTableRow } from "@/stores/branch-sync"
import { useBranchSync, useFoundCommits, useParkedTableRows } from "@/stores/branch-sync"
import { clearBranchSelection, getSelectedBranches, setBranchTargetOpen, setNewBranchDrop, setSelectedBranches, UNASSIGNED_SOURCE, useCommitMove } from "@/stores/commit-move"
import { useInlineRowAction } from "@/stores/inline-row-action"
import { useLandingSpans } from "@/stores/patronus"
import { getFullBranchName } from "@/stores/repository"
import { useRecentProjects } from "@/stores/settings/repositorySettings"
import { expandStackOf, useShownBranchTables } from "@/stores/stack-collapse"
import { setStackPathRows } from "@/stores/stack-view"

const BranchPreviewDialog = lazy(() => import("@/components/branch/BranchPreviewDialog").then(it => ({ default: it.BranchPreviewDialog })))

/** Drop target key that no list has, so commits from every list can be dropped on the "new branch" zone */
const NEW_BRANCH_TARGET = "\u0000new-branch"

/**
 * Table of active virtual branches, the branches of an issue nested under it and stacks as a graph (`StackLanes`), root
 * on top, under the filter bar (`BranchFilterBar`): the matches of the filter and, dimmed, the branches they stack on.
 * A big stack is an island of its own below the table (`StackIslandCard`, `stackIslandRoots`), with its commits on the
 * lanes; the keys, the selection, the preview and the highlighted path go through the table and the islands as one
 * list, in the order of the screen.
 * Hovering or focusing a branch highlights the path from the root of its stack down to it (`useStackPathPointer`); the
 * root of a stack heads it (`StackChip`), and a collapsed stack shows as its root alone (`useShownBranchTables`).
 * Subscribes to the row list and the matches only; each row subscribes to its own branch.
 * Space (over a branch row, on the row the arrows focused, or on the last toggled row of the focused table) opens the
 * branch preview; the focused row has its keys (`useBranchRowKeys`, M opens "Move to…" for it). Cmd/Ctrl-click and
 * Shift-click select branch rows: M or Enter opens "Move to…" for them, Cmd+A selects the rows the filter matched, Escape
 * clears.
 */
export function BranchTableCard() {
  const visible = useFilteredBranches()
  // without a filter, a collapsed stack shows as its root alone and a collapsed island as its header
  const { table, tableAll, islands } = useShownBranchTables(visible)
  // the branches kept local, in a card of their own after the islands
  const parkedRows = useParkedTableRows()
  const parkedOpen = useRecentProjects(projects => projects[0]?.parkedCollapsed !== true)
  // the rows shown, in the order of the screen: the table, the islands, then the Parked card
  const rows = useMemo(
    () => [...table.rows, ...islands.flatMap(island => (island.collapsed ? [] : island.shown.rows)), ...(parkedOpen ? parkedRows : [])],
    [table.rows, islands, parkedRows, parkedOpen],
  )
  // the roots whose stack or side stack hides rows
  const hidden = useMemo(() => new Map([...table.hidden, ...islands.flatMap(island => [...island.shown.hidden])]), [table.hidden, islands])
  const stackShown = useMemo(() => stackSizes(table.rows), [table.rows])
  // the roots of the side stacks, with the rows of each the tables show
  const sideStackShown = useMemo(() => sideStackSizes(rows), [rows])
  const branchCount = useBranchSync(state => state.branchOrder.length)
  // the lane segments a running Safe Push lands, from the root of its stack down to its branch
  const spans = useLandingSpans()
  const laneRows = useMemo(() => rows.flatMap(row => (row.kind === "branch" ? [row] : [])), [rows])
  const landing = useMemo(() => landingSegments(laneRows, spans), [laneRows, spans])
  // the highlighted path to the root of the hovered or focused branch is laid out over the rows before they paint
  useLayoutEffect(() => setStackPathRows(laneRows), [laneRows])
  const pathPointer = useStackPathPointer()
  const { expandedKeys, toggleExpanded: toggleRowExpanded, setExpanded } = useRowExpansion()

  // the fallback of Space while the table has the focus: the row last toggled or focused with the arrows
  const lastToggledRef = useRef<string | null>(null)
  const toggleExpanded = useCallback(
    (name: string) => {
      lastToggledRef.current = name
      toggleRowExpanded(name)
    },
    [toggleRowExpanded],
  )

  // the preview follows the branch by name across a sync; a branch gone (renamed, merged), or whose row is no longer
  // shown, closes it
  const preview = useBranchPreview()
  const previewName = preview.name
  // loaded on the first use, then kept mounted (the dialog moves the focus only when it opens or closes)
  const [isPreviewLoaded, setIsPreviewLoaded] = useState(false)
  if (previewName !== null && !isPreviewLoaded) {
    setIsPreviewLoaded(true)
  }
  const branchNames = rows.flatMap(row => (row.kind === "branch" ? [row.name] : []))
  // what F2 steps through: the rows of the collapsed stacks and islands too
  const unfoldedNames = useMemo(() => [...tableAll, ...islands.flatMap(island => island.rows)].flatMap(row => (row.kind === "branch" ? [row.name] : [])), [tableAll, islands])
  const previewGone = previewName !== null && !branchNames.includes(previewName)
  useEffect(() => {
    if (previewGone) {
      closeBranchPreview()
    }
  }, [previewGone])
  const tableRef = useRef<HTMLTableElement>(null)
  // Space previews a row, D reviews it in its window: the same in the table and in the islands
  const previewHotkeys = { preview: openBranchPreview, review: reviewBranchRow, intent: prefetchBranchRowReview }
  usePreviewHotkeys(tableRef, () => lastToggledRef.current, previewHotkeys)
  const { handleRowKey, handleStackKey, focusFirstRow, focusMoveRow } = useBranchRowKeys({
    rows,
    expandedKeys,
    setExpanded,
    toggleExpanded,
    lastFocusedRef: lastToggledRef,
    unfoldedNames,
  })
  useMoveKey(focusMoveRow)
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
  // the lane column of the stack graph, as wide in every row of a table (`bd-lanes` in index.css)
  // of the rows before a collapse, so the names don't move when a stack that forks collapses
  const laneCount = laneCountOf(tableAll.map(row => (row.kind === "branch" ? row.lanes : null)))

  if (branchCount === 0) {
    // with no branch yet, the zone is where dragged commits go
    return hasDrop ? (
      <Card className="gap-0 py-0">
        <NewBranchForm />
        {/* the targets of a drag: the zones a drag shows, side by side, so the rows below shift once */}
        <div className="flex gap-3 p-3 empty:hidden">
          <NewBranchDropZone />
          <UnparkDropZone />
          <UnstackDropZone />
        </div>
      </Card>
    ) : null
  }

  const onTableKeyDown = (event: KeyboardEvent<HTMLTableElement>) => {
    // the keys of the commit lists in the expanded rows are theirs
    const target = event.target as HTMLElement
    if (event.defaultPrevented || (target !== event.currentTarget && target.closest("[data-testid=branch-row]") === null)) {
      return
    }
    const hasSelection = getSelectedBranches().length > 0
    const focusedRow = target.closest<HTMLElement>("tr[data-testid=branch-row]")
    if (focusedRow !== null && isCopyKey(event)) {
      // as Copy Full Branch Name of its menu
      event.preventDefault()
      void copyText(getFullBranchName(focusedRow.dataset.branchName ?? ""))
    } else if (event.key === "Escape" && hasSelection) {
      event.preventDefault()
      clearBranchSelection()
    } else if (hasSelection && (event.key === "Enter" || (event.code === "KeyM" && !hasKeyModifier(event)))) {
      event.preventDefault()
      setBranchTargetOpen(true)
    } else if (event.code === "KeyA" && (event.metaKey || event.ctrlKey) && !event.altKey && !event.shiftKey) {
      event.preventDefault()
      // what the filter matched, not the branches shown dimmed because a match stacks on them
      setSelectedBranches(rows.flatMap(row => (row.kind === "branch" && !row.dimmed ? [row.name] : [])))
    } else if ((event.key === "ArrowLeft" || event.key === "ArrowRight") && event.altKey && !event.metaKey && !event.ctrlKey && !event.shiftKey) {
      handleStackKey(event)
    } else if (!hasKeyModifier(event)) {
      handleRowKey(event)
    }
  }

  const renderRows = (shownRows: readonly BranchTableRow[], islandRoot: string | null): ReactNode =>
    shownRows.map((row, rowIndex) => {
      const inIsland = islandRoot !== null
      if (row.kind === "issue") {
        return <IssueFamilyRow key={row.key} issueKey={row.issueKey} members={row.members} />
      }
      const name = row.name
      const next = shownRows.at(rowIndex + 1)
      const collapsed = hidden.has(name)
      return (
        <BranchRow
          key={name}
          name={name}
          depth={row.depth}
          familyIndent={row.familyIndent}
          lanes={collapsed ? collapsedLanes(row.lanes) : row.lanes}
          // the header of an island sizes its stack
          stackShown={!inIsland && isStackRoot(row) ? (stackShown.get(name) ?? 1) : sideStackRoots.has(name) ? (sideStackShown.get(name) ?? 1) : undefined}
          stackKind={isStackRoot(row) ? "stack" : "side"}
          stackCollapsed={collapsed}
          stackFiltered={visible !== null}
          commitLines={inIsland}
          stackPushInHeader={name === islandRoot}
          landing={landing.get(name) ?? null}
          showIssueKey={row.showIssueKey}
          dimmed={row.dimmed}
          familyBelow={next?.kind === "branch" && next.familyIndent > row.familyIndent}
          expanded={expandedKeys.has(name)}
          onToggleExpanded={toggleExpanded}
          setExpanded={setExpanded}
        />
      )
    })
  // the side stacks of all rows: a collapsed one shows its root alone
  const sideStackRoots = sideStackSizes([...tableAll, ...islands.flatMap(island => island.rows)])
  // the table without rows while islands or the Parked card show the rest: no "No branches match", unless a filter left it empty
  const showsTable = table.rows.length > 0 || (islands.length === 0 && (visible !== null || parkedRows.length === 0))

  return (
    // the table and its islands: their columns line up (`ColumnTrack`)
    <BranchColumnsRoot>
      <Card className="gap-0 py-0">
        <BranchFilterBar onFocusRows={focusFirstRow} onLeave={() => tableRef.current?.focus()} />
        <BranchSelectionBar />
        <NewBranchForm />
        {/* the targets of a drag: the zones a drag shows, side by side, so the rows below shift once */}
        <div className="flex gap-3 p-3 empty:hidden">
          <NewBranchDropZone />
          <UnparkDropZone />
          <UnstackDropZone />
        </div>
        {showsTable && (
          <div className="overflow-x-auto">
            {/* focusable, so a click on a row gives it the focus that Space falls back on */}
            <table
              ref={tableRef}
              tabIndex={-1}
              // a table the filter left empty is one quiet line, without the column heads
              className={cn("w-full outline-none", laneCount > 0 && "bd-lanes", table.rows.length === 0 && "[&>thead]:hidden")}
              style={laneCount > 0 ? ({ "--bd-lane-count": laneCount } as CSSProperties) : undefined}
              onKeyDown={onTableKeyDown}
            >
              <thead className="border-b border-border">
                <tr>
                  <TableHeader>
                    <span className="inline-flex items-center gap-1">
                      Branch Name
                      <BranchGraphHelpPopover />
                    </span>
                  </TableHeader>
                  <TableHeader className="hidden w-px px-4 whitespace-nowrap sm:table-cell">
                    <ColumnTrack track="commits">Commits</ColumnTrack>
                  </TableHeader>
                  <TableHeader className="hidden w-px px-4 whitespace-nowrap sm:table-cell">
                    <ColumnLabel group="status">Status</ColumnLabel>
                  </TableHeader>
                  <TableHeader className="w-px px-4 whitespace-nowrap">
                    <ColumnLabel group="actions">Actions</ColumnLabel>
                  </TableHeader>
                </tr>
              </thead>
              <tbody className="bd-branch-rows" {...pathPointer}>
                {table.rows.length === 0 && <BranchFilterEmptyRow columnCount={4} />}
                {renderRows(table.rows, null)}
              </tbody>
            </table>
          </div>
        )}
        {isPreviewLoaded && (
          <Suspense fallback={null}>
            <BranchPreviewDialog
              names={branchNames}
              name={previewGone ? null : previewName}
              onNameChange={showBranchInPreview}
              onClose={closeBranchPreview}
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
      {islands.map(island => (
        <StackIslandCard key={island.root} island={island} filtered={visible !== null} landing={spans}>
          <IslandTable rows={island.rows} lastFocusedRef={lastToggledRef} hotkeys={previewHotkeys} onKeyDown={onTableKeyDown} pathPointer={pathPointer}>
            {renderRows(island.shown.rows, island.root)}
          </IslandTable>
        </StackIslandCard>
      ))}
      {parkedRows.length === 0 && <ParkDropCard />}
      {parkedRows.length > 0 && (
        <ParkedBranchesCard count={parkedRows.filter(row => row.kind === "branch").length}>
          <IslandTable rows={parkedRows} lastFocusedRef={lastToggledRef} hotkeys={previewHotkeys} onKeyDown={onTableKeyDown} pathPointer={pathPointer}>
            {renderRows(parkedRows, null)}
          </IslandTable>
        </ParkedBranchesCard>
      )}
    </BranchColumnsRoot>
  )
}

interface IslandTableProps {
  rows: readonly BranchTableRow[]
  lastFocusedRef: RefObject<string | null>
  hotkeys: PreviewHotkeys
  onKeyDown: (event: KeyboardEvent<HTMLTableElement>) => void
  pathPointer: ReturnType<typeof useStackPathPointer>
  children: ReactNode
}

/**
 * The rows of a stack island, a table of its own without the column heads: its lane column is as wide as its stack
 * needs; Space opens the preview of its rows, D and W their review, the keys are those of the branch table
 */
function IslandTable({ rows, lastFocusedRef, hotkeys, onKeyDown, pathPointer, children }: IslandTableProps) {
  const tableRef = useRef<HTMLTableElement>(null)
  usePreviewHotkeys(tableRef, () => lastFocusedRef.current, hotkeys)
  const laneCount = laneCountOf(rows.map(row => (row.kind === "branch" ? row.lanes : null)))
  return (
    <table
      ref={tableRef}
      tabIndex={-1}
      className={cn("w-full outline-none", laneCount > 0 && "bd-lanes")}
      style={laneCount > 0 ? ({ "--bd-lane-count": laneCount } as CSSProperties) : undefined}
      onKeyDown={onKeyDown}
      data-testid="stack-island-table"
    >
      <tbody className="bd-branch-rows" {...pathPointer}>
        {children}
      </tbody>
    </table>
  )
}

/**
 * F2 and Shift+F2 from anywhere in the window but a text field, a dialog or a menu: the focus moves to the next or the
 * previous branch row that is the user's move, a fix first
 */
function useMoveKey(focusMoveRow: (step: 1 | -1) => void): void {
  const onKeyDown = useEffectEvent((event: globalThis.KeyboardEvent) => {
    if (event.key !== "F2" || event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey) {
      return
    }
    if (event.target instanceof Element && event.target.closest(KEY_OWNERS) !== null) {
      return
    }
    event.preventDefault()
    focusMoveRow(event.shiftKey ? -1 : 1)
  })
  useEffect(() => {
    const listener = (event: globalThis.KeyboardEvent) => onKeyDown(event)
    document.addEventListener("keydown", listener)
    return () => document.removeEventListener("keydown", listener)
  }, [])
}

/**
 * The "Move to…" form of the commits dropped on the "new branch" zone: it keeps their ids and closes when a rewrite gave
 * one of them another id
 */
function NewBranchForm() {
  const dropped = useCommitMove(state => state.newBranchDrop)
  const commits = useFoundCommits(dropped?.commitIds ?? [])
  const isGone = dropped !== null && commits === null
  useEffect(() => {
    if (isGone) {
      setNewBranchDrop(null)
    }
  }, [isGone])

  if (dropped === null || commits === null) {
    return null
  }
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

/** Shown while commits or branches are dragged, and no form of a drop waits: dropping them opens `NewBranchForm` */
function NewBranchDropZone() {
  const hasForm = useCommitMove(state => state.newBranchDrop !== null)
  const dropTarget = useCommitDropTarget(NEW_BRANCH_TARGET, drag => setNewBranchDrop({ sourceKeys: drag.sourceKeys, commitIds: drag.commits.map(commit => commit.originalHash) }))
  if (hasForm) {
    return null
  }
  return <DropZone target={dropTarget} icon={GitBranchPlusIcon} label="Drop here to move the commits to a new branch" data-testid="new-branch-drop-zone" />
}
