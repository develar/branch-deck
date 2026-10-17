import { lazy, memo, Suspense, useEffect } from "react"
import type { DragEvent, MouseEvent } from "react"
import { cn } from "cn"
import { TriangleAlertIcon, XIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { ActiveBranchExpanded, CommitRail } from "@/components/branch/ActiveBranchExpanded"
import { BranchActions, BranchMenuItems } from "@/components/branch/BranchActions"
import { BranchContextMenuRow } from "@/components/branch/BranchContextMenuRow"
import { BranchNameCell } from "@/components/branch/BranchNameCell"
import { BranchRowStatus } from "@/components/branch/BranchRowStatus"
import { LandedChip } from "@/components/branch/LandedChip"
import { LandingSpanLabel } from "@/components/branch/LandingSpanLabel"
import { PullRequestChip } from "@/components/branch/PullRequestChip"
import { PassedParentLabel } from "@/components/branch/StackBaseLabel"
import { StackChip } from "@/components/branch/StackChip"
import { LaneCell, LaneGutter } from "@/components/branch/StackLanes"
import { StackCommitLines } from "@/components/commit/StackCommitLines"
import { ContextMenuItems } from "@/components/shared/ContextMenuItems"
import { InlineRowProgressBar } from "@/components/shared/InlineForm"
import { beginCommitDrag, useCommitDropTarget } from "@/hooks/useCommitDrag"
import { moveCommits } from "@/hooks/useMoveCommits"
import { useListFixups } from "@/hooks/useSquashFixups"
import { formatBranchesAndCommits } from "@/lib/branch"
import { inTableOrder } from "@/lib/branch-selection"
import { formatCommitCount } from "@/lib/commit"
import { isRobotActive } from "@/lib/patronus"
import type { LaneSegments, StackLanes } from "@/lib/stack-lanes"
import { branchRowDivider, dividerAttributes, laneContentInsetClass, rowBelowDivider } from "@/lib/stack-lanes"
import { getBranchCommits, getBranchTableOrder, useBranch, useBranchSync } from "@/stores/branch-sync"
import {
  clearBranchSelection,
  endCommitDrag,
  getSelectedBranches,
  requestReveal,
  selectBranches,
  startCommitDrag,
  useIsBranchSelected,
  useRevealRequest,
} from "@/stores/commit-move"
import type { InlineActionType } from "@/stores/inline-row-action"
import { closeInline, portalTargetIdFor, useActiveInlineForRow } from "@/stores/inline-row-action"
import { useBranchRobot } from "@/stores/patronus"
import { getShownBranchTableOrder, toggleSideStackCollapsed, toggleStackCollapsed } from "@/stores/stack-collapse"

const InlineRenameBranchInput = lazy(() => import("@/components/branch/InlineRenameBranchInput").then(it => ({ default: it.InlineRenameBranchInput })))
const InlineIssueReferenceInput = lazy(() => import("@/components/branch/InlineIssueReferenceInput").then(it => ({ default: it.InlineIssueReferenceInput })))
const InlineAmendChangesInput = lazy(() => import("@/components/branch/InlineAmendChangesInput").then(it => ({ default: it.InlineAmendChangesInput })))
const InlineUngroupConfirm = lazy(() => import("@/components/branch/InlineUngroupConfirm").then(it => ({ default: it.InlineUngroupConfirm })))
const InlinePatronusStart = lazy(() => import("@/components/branch/InlinePatronusStart").then(it => ({ default: it.InlinePatronusStart })))
const InlineUnapplyConfirm = lazy(() => import("@/components/branch/InlineUnapplyConfirm").then(it => ({ default: it.InlineUnapplyConfirm })))
const MergeConflictViewer = lazy(() => import("@/components/conflict/MergeConflictViewer").then(it => ({ default: it.MergeConflictViewer })))

const columnCount = 4

const inlineFormTypes: ReadonlySet<InlineActionType> = new Set(["issue-reference", "rename", "amend-changes", "unapply", "ungroup", "safe-push", "dry-run"])

export interface BranchRowProps {
  name: string
  /** Levels nested: under the row of its issue and under the branches it stacks on (`data-depth`) */
  depth?: number
  /** Under the row of its issue: one indent step, with the guide of the family */
  familyIndent?: 0 | 1
  /** Its place in the stack graph; null in a table without stacks */
  lanes?: StackLanes | null
  /** The segments of a landing span in the row (`landingSegments`), in the colour of the robot; null outside one */
  landing?: LaneSegments | null
  /** Shows the issue the branch belongs to next to its name (no issue row above it) */
  showIssueKey?: boolean
  /** Not a match of the filter, shown because a match stacks on it: faded, still interactive (`data-dimmed`) */
  dimmed?: boolean
  /** The next rows are the members of its issue family: the family guide runs beside its expanded commits into theirs */
  familyBelow?: boolean
  /** A stack root: the branches of its stack the table shows, itself included (`StackChip`); undefined for another row */
  stackShown?: number
  /** `side`: the root of a side stack (`sideStackSizes`), whose chip collapses it alone, for the session */
  stackKind?: "stack" | "side"
  /** A stack root collapsed into its row: the branches stacked on it are hidden */
  stackCollapsed?: boolean
  /** A filter shows its matches: the stack doesn't collapse */
  stackFiltered?: boolean
  /** In a stack island: the commits show on the lane, a line each (`StackCommitLines`), while the row is not expanded */
  commitLines?: boolean
  /** The root of an island: the push of its stack (Push with Children) shows in the island header, not in the row */
  stackPushInHeader?: boolean
  expanded: boolean
  onToggleExpanded: (name: string) => void
  setExpanded: (name: string, expanded: boolean) => void
}

/**
 * Rows of one active branch: the branch row, then (if open) the inline form row, the amend conflict row and the expanded commits row.
 * Subscribes to its own branch only, so a sync event for another branch does not re-render it.
 */
export const BranchRow = memo(function BranchRow({
  name,
  depth = 0,
  familyIndent = 0,
  lanes = null,
  landing = null,
  showIssueKey = false,
  dimmed = false,
  familyBelow = false,
  stackShown,
  stackKind = "stack",
  stackCollapsed = false,
  stackFiltered = false,
  commitLines = false,
  stackPushInHeader = false,
  expanded,
  onToggleExpanded,
  setExpanded,
}: BranchRowProps) {
  const branch = useBranch(name)
  // the node of a branch whose robot runs has the ring of the Safe Push slot
  const robot = useBranchRobot(name)
  const robotRuns = robot !== undefined && isRobotActive(robot)
  const activeInline = useActiveInlineForRow(name)
  const { count: fixupCount } = useListFixups(name)
  // commits and branches dragged from elsewhere move here; the rows of the branch below its row are the target too
  const dropTarget = useCommitDropTarget(name, drag => void moveCommits(drag.commits, name))
  const selected = useIsBranchSelected(name)
  const isSyncing = useBranchSync(state => state.isSyncing)

  // commits just moved here: show them (the commit list selects them); a request without commits is done here
  const reveal = useRevealRequest(name)
  const isRevealTarget = reveal !== null && branch !== undefined
  const revealsCommits = reveal !== null && reveal.commitIds.length > 0
  useEffect(() => {
    if (!isRevealTarget) {
      return
    }
    setExpanded(name, true)
    document.querySelector(`tr[data-branch-name="${CSS.escape(name)}"]`)?.scrollIntoView({ behavior: "smooth", block: "nearest" })
    if (!revealsCommits) {
      requestReveal(null)
    }
  }, [isRevealTarget, revealsCommits, name, setExpanded])

  if (branch === undefined) {
    return null
  }

  const processing = activeInline?.processing === true
  const toggle = () => onToggleExpanded(name)
  // Cmd/Ctrl-click and Shift-click select branches; a plain click expands the row and clears the selection
  const onRowClick = (event: MouseEvent) => {
    if (event.metaKey || event.ctrlKey) {
      selectBranches(name, "toggle", getBranchTableOrder())
    } else if (event.shiftKey) {
      // the rows shown: a filter or a collapsed stack hides the others
      selectBranches(name, "range", getShownBranchTableOrder())
    } else {
      clearBranchSelection()
      toggle()
    }
  }
  // a move is a rewrite: it waits for a running sync, also of a row that shows its last state meanwhile
  const canDrag = branch.commitCount > 0 && !processing && branch.status !== "Syncing" && !isSyncing
  const onDragStart = (event: DragEvent) => {
    const order = getBranchTableOrder()
    // what moves is what is selected: an unselected row dragged becomes the selection
    if (!getSelectedBranches().includes(name)) {
      selectBranches(name, "only", order)
    }
    const names = inTableOrder(getSelectedBranches(), order)
    const commits = getBranchCommits(names)
    const label = names.length === 1 ? formatCommitCount(commits.length) : formatBranchesAndCommits(names.length, commits.length)
    beginCommitDrag(event, commits.length, label)
    startCommitDrag({ sourceKeys: names, branches: names, commits })
  }
  // the rows below the branch row: a drop on them is a drop on the branch
  const blockDropClass = dropTarget.isOver ? "bg-primary/5" : undefined
  // the expanded commits sit on the lane of the branch: its line runs down from the node to them
  const hasCommitList = branch.commits.length > 0 && branch.errorDetails?.Generic === undefined
  // in an island the commits show without the row expanded, a line each
  const showsCommitLines = commitLines && !expanded && hasCommitList && lanes !== null
  const commitsOnLane = (expanded && hasCommitList) || showsCommitLines
  // the lines beside the inline form and the amend conflict, and where their content starts
  const gutter = lanes === null ? null : <LaneGutter name={name} lanes={lanes} commitsBelow={commitsOnLane} familyIndent={familyIndent} landing={landing} />
  const insetClass = lanes === null ? undefined : laneContentInsetClass(familyIndent)
  // inside its unit the divider above a row starts at the tree, so the unit reads as one block
  const below = dividerAttributes(rowBelowDivider(familyIndent))

  return (
    <>
      <BranchContextMenuRow
        data-testid="branch-row"
        name={name}
        rowId={name}
        depth={depth}
        lane={lanes?.lane}
        dimmed={dimmed}
        divider={branchRowDivider(depth, familyIndent)}
        expanded={expanded}
        processing={processing}
        group
        className={cn(
          expanded && !selected && "data-[state=open]:bg-muted/50",
          dropTarget.canDrop && "outline-1 -outline-offset-1 outline-primary/30 outline-dashed",
          dropTarget.isOver && "bg-primary/10 outline-2 outline-primary outline-solid hover:bg-primary/10",
        )}
        dropTarget={dropTarget.canDrop ? (dropTarget.isOver ? "over" : "ready") : undefined}
        dropProps={dropTarget.dropProps}
        selected={selected}
        onDragStart={canDrag ? onDragStart : undefined}
        onDragEnd={endCommitDrag}
        onToggle={onRowClick}
        menu={<BranchMenuItems name={name} setExpanded={setExpanded} Items={ContextMenuItems} />}
      >
        <BranchNameCell
          name={name}
          familyIndent={familyIndent}
          lanes={
            lanes === null ? undefined : (
              <LaneCell
                lanes={lanes}
                branch={branch}
                familyIndent={familyIndent}
                lineDown={lanes.down || commitsOnLane}
                collapsedBelow={stackCollapsed && !commitsOnLane}
                ring={robotRuns}
                highlight={landing}
              />
            )
          }
          issueKey={showIssueKey ? branch.issueKey : null}
          fixupCount={fixupCount}
          mark={
            stackShown === undefined ? undefined : (
              <StackChip
                root={name}
                collapsed={stackCollapsed}
                shown={stackShown}
                filtered={stackFiltered}
                onToggle={() => (stackKind === "side" ? toggleSideStackCollapsed(name, !stackCollapsed) : toggleStackCollapsed(name, !stackCollapsed))}
              />
            )
          }
          summary={branch.summary}
          descriptionTitle={branch.descriptionTitle}
          expanded={expanded}
          canExpand={branch.commitCount > 0 || branch.hasError}
          onToggleExpanded={toggle}
        />
        <td className="hidden w-px px-4 py-(--bd-cell-y) whitespace-nowrap sm:table-cell">
          <span className="text-sm text-muted-foreground">{branch.commitCount === 1 ? "1 commit" : `${branch.commitCount} commits`}</span>
        </td>
        <td className="hidden w-px px-4 py-(--bd-cell-y) whitespace-nowrap sm:table-cell">
          {/*
            three columns, so each chip kind keeps its column across rows: the state of the branch (its robot, else the
            status text) in a fixed width, so a robot that arrives with the answer of Patronus moves no chip, the pull
            request chip, then the Landed chip, the robot of a branch built on it that lands it, or "on master" of a
            branch built elsewhere; `justify-start` keeps the auto columns at their content width
          */}
          <div className="grid grid-cols-[9rem_auto_auto] items-center justify-start gap-2">
            <div className="flex min-w-0">
              <BranchRowStatus branch={branch} showDot={lanes === null} />
            </div>
            <div className="col-start-2 flex">
              <PullRequestChip branchName={name} />
            </div>
            <div className="col-start-3 flex items-center gap-2">
              <LandedChip branchName={name} />
              <LandingSpanLabel name={name} fallback={<PassedParentLabel branch={branch} />} />
            </div>
          </div>
        </td>
        <td className="w-px px-4 py-(--bd-cell-y) whitespace-nowrap">
          <BranchActions branch={branch} setExpanded={setExpanded} stackPushInHeader={stackPushInHeader} />
        </td>
      </BranchContextMenuRow>

      {activeInline !== null && inlineFormTypes.has(activeInline.type) && (
        <tr className={blockDropClass} data-branch-of={name} {...below} {...dropTarget.dropProps}>
          <td id={portalTargetIdFor(name)} colSpan={columnCount} className={cn("relative overflow-hidden p-0", insetClass)}>
            {gutter}
            {processing && <InlineRowProgressBar className={lanes === null ? undefined : "right-0 left-(--bd-row-inset) w-auto"} />}
            <Suspense fallback={null}>
              {activeInline.type === "issue-reference" && <InlineIssueReferenceInput key={`issue-reference:${name}`} branchName={name} />}
              {activeInline.type === "rename" && <InlineRenameBranchInput key={`rename:${name}`} branchName={name} />}
              {activeInline.type === "amend-changes" && <InlineAmendChangesInput key={`amend-changes:${name}`} branchName={name} />}
              {activeInline.type === "unapply" && <InlineUnapplyConfirm key={`unapply:${name}`} branchName={name} />}
              {activeInline.type === "ungroup" && <InlineUngroupConfirm key={`ungroup:${name}`} branchName={name} />}
              {activeInline.type === "safe-push" && (
                <InlinePatronusStart
                  key={`safe-push:${name}:${activeInline.bypassReview === true ? "bypass" : ""}`}
                  branchName={name}
                  mode="rebaseAutosquash"
                  bypassReview={activeInline.bypassReview === true}
                />
              )}
              {activeInline.type === "dry-run" && <InlinePatronusStart key={`dry-run:${name}`} branchName={name} mode="dryRun" bypassReview={false} />}
            </Suspense>
          </td>
        </tr>
      )}

      {activeInline?.type === "amend-conflict" && activeInline.conflictInfo && (
        <tr data-branch-of={name} {...below}>
          <td colSpan={columnCount} className={cn("relative p-0", insetClass)}>
            {gutter}
            {/* destructive: the amend failed on a conflict */}
            <div className="flex flex-col gap-3 border-b border-border bg-destructive/5 bd-padding-content">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2 text-sm">
                  <TriangleAlertIcon className="size-4 shrink-0 text-destructive" />
                  <span className="font-medium text-destructive">Cannot amend: conflicts with subsequent commits</span>
                </div>
                <Tooltip>
                  <TooltipTrigger render={<Button size="icon-xs" variant="ghost" aria-label="Dismiss the amend conflict" onClick={closeInline} />}>
                    <XIcon />
                  </TooltipTrigger>
                  <TooltipContent>Dismiss</TooltipContent>
                </Tooltip>
              </div>
              <Suspense fallback={null}>
                <MergeConflictViewer conflict={activeInline.conflictInfo} branchName={name} onto="the amended commit" />
              </Suspense>
            </div>
          </td>
        </tr>
      )}

      {showsCommitLines && (
        <tr data-testid="branch-commit-lines-row" className={blockDropClass} data-branch-of={name} {...below} {...dropTarget.dropProps}>
          <td colSpan={columnCount} className="relative p-0">
            <CommitRail name={name} familyIndent={familyIndent} familyBelow={familyBelow} lanes={lanes} landing={landing}>
              <StackCommitLines branch={branch} />
            </CommitRail>
          </td>
        </tr>
      )}

      {expanded && (
        <tr data-testid="branch-commits-row" className={blockDropClass} data-branch-of={name} {...below} {...dropTarget.dropProps}>
          {/* with lanes, the cell positions their lines and the rail pads itself */}
          <td colSpan={columnCount} className={lanes === null ? "px-0 py-(--bd-cell-y)" : "relative p-0"}>
            <ActiveBranchExpanded
              branch={branch}
              familyIndent={familyIndent}
              lanes={lanes}
              landing={landing}
              familyBelow={familyBelow}
              highlightTipCommit={activeInline?.type === "amend-changes"}
            />
          </td>
        </tr>
      )}
    </>
  )
})
