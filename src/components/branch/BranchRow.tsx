import { lazy, memo, Suspense, useEffect } from "react"
import { TriangleAlertIcon, XIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { ActiveBranchExpanded } from "@/components/branch/ActiveBranchExpanded"
import { BranchActions } from "@/components/branch/BranchActions"
import { BranchContextMenuRow } from "@/components/branch/BranchContextMenuRow"
import { BranchNameCell } from "@/components/branch/BranchNameCell"
import { BranchStatusBadge } from "@/components/branch/BranchStatusBadge"
import { LandedChip } from "@/components/branch/LandedChip"
import { PatronusChip } from "@/components/branch/PatronusChip"
import { PullRequestChip } from "@/components/branch/PullRequestChip"
import { UnpushedParentLabel } from "@/components/branch/StackBaseLabel"
import { ContextMenuItems } from "@/components/shared/ContextMenuItems"
import { InlineRowProgressBar } from "@/components/shared/InlineForm"
import { getBranchContextMenuItems } from "@/hooks/useBranchContextActions"
import { useCommitDropTarget } from "@/hooks/useCommitDrag"
import { moveCommits } from "@/hooks/useMoveCommits"
import { useListFixups } from "@/hooks/useSquashFixups"
import { useBranch } from "@/stores/branch-sync"
import { requestReveal, useRevealRequest } from "@/stores/commit-move"
import type { InlineActionType } from "@/stores/inline-row-action"
import { closeInline, portalTargetIdFor, useActiveInlineForRow } from "@/stores/inline-row-action"

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
  /** Levels nested: under the row of its issue and under the branches it stacks on */
  depth?: number
  /** Shows the issue the branch belongs to next to its name (no issue row above it) */
  showIssueKey?: boolean
  /** The next rows are nested under this one: the rail of its expanded commits continues into their rule */
  hasChildren?: boolean
  expanded: boolean
  onToggleExpanded: (name: string) => void
  setExpanded: (name: string, expanded: boolean) => void
}

/**
 * Rows of one active branch: the branch row, then (if open) the inline form row, the amend conflict row and the expanded commits row.
 * Subscribes to its own branch only, so a sync event for another branch does not re-render it.
 */
export const BranchRow = memo(function BranchRow({ name, depth = 0, showIssueKey = false, hasChildren = false, expanded, onToggleExpanded, setExpanded }: BranchRowProps) {
  const branch = useBranch(name)
  const activeInline = useActiveInlineForRow(name)
  const { count: fixupCount } = useListFixups(name)
  // commits dragged from another list move here
  const dropTarget = useCommitDropTarget(name, drag => void moveCommits(drag.commits, name))

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

  return (
    <>
      <BranchContextMenuRow
        data-testid="branch-row"
        name={name}
        rowId={name}
        depth={depth}
        expanded={expanded}
        processing={processing}
        group
        className={cn(
          expanded && "data-[state=open]:bg-muted/50",
          dropTarget.canDrop && "outline-1 -outline-offset-1 outline-primary/30 outline-dashed",
          dropTarget.isOver && "bg-primary/10 outline-2 outline-primary outline-solid hover:bg-primary/10",
        )}
        dropTarget={dropTarget.canDrop ? (dropTarget.isOver ? "over" : "ready") : undefined}
        dropProps={dropTarget.dropProps}
        onToggle={toggle}
        menu={<BranchContextMenuItems name={name} setExpanded={setExpanded} />}
      >
        <BranchNameCell
          name={name}
          depth={depth}
          issueKey={showIssueKey ? branch.issueKey : null}
          fixupCount={fixupCount}
          summary={branch.summary}
          expanded={expanded}
          canExpand={branch.commitCount > 0 || branch.hasError}
          onToggleExpanded={toggle}
        />
        <td className="hidden w-px px-4 py-4 whitespace-nowrap sm:table-cell">
          <span className="text-sm text-muted-foreground">
            {branch.commitCount === 1 ? "1 commit" : `${branch.commitCount} commits`}
          </span>
        </td>
        <td className="hidden w-px px-4 py-4 whitespace-nowrap sm:table-cell">
          {/*
            three columns, so each chip kind keeps its column across rows: the status text (7rem at least), the pull
            request chip, then the Patronus and Landed chips and the parent a push leaves out; `justify-start` keeps the
            auto columns at their content width
          */}
          <div className="grid grid-cols-[minmax(7rem,auto)_auto_auto] items-center justify-start gap-2">
            <BranchStatusBadge branch={branch} />
            <div className="col-start-2 flex">
              <PullRequestChip branchName={name} />
            </div>
            <div className="col-start-3 flex items-center gap-2">
              <PatronusChip branchName={name} />
              <LandedChip branchName={name} />
              <UnpushedParentLabel branch={branch} />
            </div>
          </div>
        </td>
        <td className="w-px px-4 py-4 whitespace-nowrap">
          <BranchActions branch={branch} />
        </td>
      </BranchContextMenuRow>

      {activeInline !== null && inlineFormTypes.has(activeInline.type) && (
        <tr>
          <td id={portalTargetIdFor(name)} colSpan={columnCount} className="relative overflow-hidden p-0">
            {processing && <InlineRowProgressBar />}
            <Suspense fallback={null}>
              {activeInline.type === "issue-reference" && <InlineIssueReferenceInput key={`issue-reference:${name}`} branchName={name} />}
              {activeInline.type === "rename" && <InlineRenameBranchInput key={`rename:${name}`} branchName={name} />}
              {activeInline.type === "amend-changes" && <InlineAmendChangesInput key={`amend-changes:${name}`} branchName={name} />}
              {activeInline.type === "unapply" && <InlineUnapplyConfirm key={`unapply:${name}`} branchName={name} />}
              {activeInline.type === "ungroup" && <InlineUngroupConfirm key={`ungroup:${name}`} branchName={name} />}
              {activeInline.type === "safe-push" && <InlinePatronusStart key={`safe-push:${name}`} branchName={name} mode="rebaseAutosquash" />}
              {activeInline.type === "dry-run" && <InlinePatronusStart key={`dry-run:${name}`} branchName={name} mode="dryRun" />}
            </Suspense>
          </td>
        </tr>
      )}

      {activeInline?.type === "amend-conflict" && activeInline.conflictInfo && (
        <tr>
          <td colSpan={columnCount} className="p-0">
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
                <MergeConflictViewer conflict={activeInline.conflictInfo} branchName={name} />
              </Suspense>
            </div>
          </td>
        </tr>
      )}

      {expanded && (
        <tr>
          <td colSpan={columnCount} className="px-0 py-4">
            <ActiveBranchExpanded branch={branch} depth={depth} hasChildren={hasChildren} highlightTipCommit={activeInline?.type === "amend-changes"} />
          </td>
        </tr>
      )}
    </>
  )
})

/** Rendered only while the menu is open, so the items are computed lazily from the current branch state. */
function BranchContextMenuItems({ name, setExpanded }: { name: string, setExpanded: (name: string, expanded: boolean) => void }) {
  const branch = useBranch(name)
  if (branch === undefined) {
    return null
  }
  return <ContextMenuItems groups={getBranchContextMenuItems(branch, setExpanded)} />
}
