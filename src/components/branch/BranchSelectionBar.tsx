import { FolderInputIcon, UndoDotIcon, XIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { InlineBranchTarget } from "@/components/shared/InlineBranchTarget"
import { getRemovePrefixLabel } from "@/hooks/useCommitMoveList"
import { moveCommits } from "@/hooks/useMoveCommits"
import { inTableOrder } from "@/lib/branch-selection"
import { useBranchCommits, useBranchTableRows, useIssueGroups, useVisibleBranchTableRows } from "@/stores/branch-sync"
import { useShownBranches } from "@/stores/branch-filter"
import { clearBranchSelection, setBranchTargetOpen, useCommitMove } from "@/stores/commit-move"

/**
 * Above the branch table while branch rows are selected (Cmd/Ctrl-click, Shift-click): "N branches selected" (with
 * "· 1 hidden" for the selected branches the filter hides, which the actions take too), "Move to…" (the form that puts
 * the commits of all of them into one branch, existing or new) and the way back; the form itself while it is open.
 */
export function BranchSelectionBar() {
  const selected = useCommitMove(state => state.selectedBranches)
  const isTargetOpen = useCommitMove(state => state.isBranchTargetOpen)
  const issueGroups = useIssueGroups()
  const rows = useBranchTableRows()
  const names = inTableOrder(selected, rows.flatMap(row => row.kind === "branch" ? [row.name] : []))
  const commits = useBranchCommits(names)
  const shownRows = useVisibleBranchTableRows(useShownBranches())
  const hiddenCount = names.filter(name => !shownRows.some(row => row.kind === "branch" && row.name === name)).length
  if (names.length === 0) {
    return null
  }

  if (isTargetOpen) {
    return (
      <div className="border-b border-border">
        <InlineBranchTarget
          commits={commits}
          sourceBranches={names}
          isActive
          onCancel={() => setBranchTargetOpen(false)}
          onSuccess={clearBranchSelection}
        />
      </div>
    )
  }

  const backLabel = getRemovePrefixLabel(commits, issueGroups)
  return (
    <div className="flex items-center justify-end gap-2 border-b border-border px-4 py-2" data-testid="branch-selection-actions">
      <span className="text-xs text-muted-foreground">
        {names.length === 1 ? "1 branch selected" : `${names.length} branches selected`}
        {hiddenCount > 0 && ` · ${hiddenCount} hidden`}
      </span>
      <Button size="xs" variant="outline" className="border-primary/50 text-primary hover:bg-primary/10 hover:text-primary" onClick={() => setBranchTargetOpen(true)}>
        <FolderInputIcon data-icon="inline-start" />
        Move to…
      </Button>
      {backLabel !== null && (
        <Button
          size="xs"
          variant="ghost"
          onClick={() => {
            void moveCommits(commits, null).then((success) => {
              if (success) {
                clearBranchSelection()
              }
            })
          }}
        >
          <UndoDotIcon data-icon="inline-start" />
          {backLabel}
        </Button>
      )}
      <Tooltip>
        <TooltipTrigger render={<Button size="icon-xs" variant="ghost" aria-label="Clear the branch selection" onClick={clearBranchSelection} />}>
          <XIcon />
        </TooltipTrigger>
        <TooltipContent>Clear the selection (Esc)</TooltipContent>
      </Tooltip>
    </div>
  )
}
