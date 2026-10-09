import { lazy, memo, Suspense } from "react"
import { TriangleAlertIcon, XIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { ContextMenu, ContextMenuContent, ContextMenuTrigger } from "@/components/ui/context-menu"
import { ActiveBranchExpanded } from "@/components/branch/ActiveBranchExpanded"
import { BranchActions } from "@/components/branch/BranchActions"
import { BranchNameCell } from "@/components/branch/BranchNameCell"
import { BranchStatusBadge } from "@/components/branch/BranchStatusBadge"
import { ContextMenuItems } from "@/components/shared/ContextMenuItems"
import { InlineRowProgressBar } from "@/components/shared/InlineForm"
import { getBranchContextMenuItems } from "@/hooks/useBranchContextActions"
import { useBranch } from "@/stores/branch-sync"
import type { InlineActionType } from "@/stores/inline-row-action"
import { closeInline, portalTargetIdFor, useActiveInlineForRow } from "@/stores/inline-row-action"

const InlineIssueReferenceInput = lazy(() => import("@/components/branch/InlineIssueReferenceInput").then(it => ({ default: it.InlineIssueReferenceInput })))
const InlineAmendChangesInput = lazy(() => import("@/components/branch/InlineAmendChangesInput").then(it => ({ default: it.InlineAmendChangesInput })))
const InlineUnapplyConfirm = lazy(() => import("@/components/branch/InlineUnapplyConfirm").then(it => ({ default: it.InlineUnapplyConfirm })))
const MergeConflictViewer = lazy(() => import("@/components/conflict/MergeConflictViewer").then(it => ({ default: it.MergeConflictViewer })))

const columnCount = 4

const inlineFormTypes: ReadonlySet<InlineActionType> = new Set(["issue-reference", "amend-changes", "unapply"])

export interface BranchRowProps {
  name: string
  expanded: boolean
  onToggleExpanded: (name: string) => void
  setExpanded: (name: string, expanded: boolean) => void
}

/**
 * Rows of one active branch: the branch row, then (if open) the inline form row, the amend conflict row and the expanded commits row.
 * Subscribes to its own branch only, so a sync event for another branch does not re-render it.
 */
export const BranchRow = memo(function BranchRow({ name, expanded, onToggleExpanded, setExpanded }: BranchRowProps) {
  const branch = useBranch(name)
  const activeInline = useActiveInlineForRow(name)
  if (branch === undefined) {
    return null
  }

  const processing = activeInline?.processing === true
  const toggle = () => onToggleExpanded(name)

  return (
    <>
      <ContextMenu disabled={processing}>
        <ContextMenuTrigger
          render={(
            <tr
              data-testid="branch-row"
              data-branch-name={name}
              data-state={expanded ? "open" : "closed"}
              className={cn("group cursor-pointer transition-all hover:bg-muted", expanded && "data-[state=open]:bg-muted")}
              onClick={toggle}
            />
          )}
        >
          <BranchNameCell
            name={name}
            summary={branch.summary}
            expanded={expanded}
            canExpand={branch.commitCount > 0 || branch.hasError}
            onToggleExpanded={toggle}
          />
          <td className="hidden px-6 py-4 sm:table-cell">
            <span className="text-sm text-muted-foreground">
              {branch.commitCount === 1 ? "1 commit" : `${branch.commitCount} commits`}
            </span>
          </td>
          <td className="hidden px-6 py-4 sm:table-cell">
            <BranchStatusBadge branch={branch} />
          </td>
          <td className="px-6 py-4">
            <BranchActions branch={branch} />
          </td>
        </ContextMenuTrigger>
        {/* the inline form opened from the menu takes the focus */}
        <ContextMenuContent finalFocus={false}>
          <BranchContextMenuItems name={name} setExpanded={setExpanded} />
        </ContextMenuContent>
      </ContextMenu>

      {activeInline !== null && inlineFormTypes.has(activeInline.type) && (
        <tr>
          <td id={portalTargetIdFor(name)} colSpan={columnCount} className="relative overflow-hidden p-0">
            {processing && <InlineRowProgressBar />}
            <Suspense fallback={null}>
              {activeInline.type === "issue-reference" && <InlineIssueReferenceInput key={`issue-reference:${name}`} branchName={name} />}
              {activeInline.type === "amend-changes" && <InlineAmendChangesInput key={`amend-changes:${name}`} branchName={name} />}
              {activeInline.type === "unapply" && <InlineUnapplyConfirm key={`unapply:${name}`} branchName={name} />}
            </Suspense>
          </td>
        </tr>
      )}

      {activeInline?.type === "amend-conflict" && activeInline.conflictInfo && (
        <tr>
          <td colSpan={columnCount} className="p-0">
            <div className="flex flex-col gap-3 border-b border-border bg-warning/5 bd-padding-content">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <TriangleAlertIcon className="size-5 text-warning" />
                  <span className="font-medium">Cannot amend: conflicts with subsequent commits</span>
                </div>
                <Button size="icon-xs" variant="ghost" aria-label="Dismiss" onClick={closeInline}>
                  <XIcon />
                </Button>
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
            <ActiveBranchExpanded branch={branch} highlightTipCommit={activeInline?.type === "amend-changes"} />
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
