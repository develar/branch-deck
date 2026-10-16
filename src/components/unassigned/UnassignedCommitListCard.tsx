import { cn } from "cn"
import { Card } from "@/components/ui/card"
import { CommitSelectionActions, MovableCommitList } from "@/components/commit/MovableCommitList"
import { SquashFixupsButton } from "@/components/commit/SquashFixupsButton"
import { InlineCode } from "@/components/shared/InlineCode"
import { SectionCardHeader } from "@/components/shared/SectionCardHeader"
import { UnassignedCommitsHelpPopover } from "@/components/unassigned/UnassignedCommitsHelpPopover"
import { useCommitDropTarget } from "@/hooks/useCommitDrag"
import { getRemovePrefixLabel, useCommitMoveList } from "@/hooks/useCommitMoveList"
import { moveCommits } from "@/hooks/useMoveCommits"
import type { Commit } from "@/lib/bindings"
import { getIssueGroups, useBranchSync } from "@/stores/branch-sync"
import { UNASSIGNED_SOURCE } from "@/stores/commit-move"

export interface UnassignedCommitListCardProps {
  commits: Commit[]
}

/** Commits that match no grouping pattern; select them to move them to a branch, or drop branch commits here. */
export function UnassignedCommitListCard({ commits }: UnassignedCommitListCardProps) {
  const moveList = useCommitMoveList({ commits, sourceBranch: null })
  // with no branch yet, the grouping convention is new: said in the card, not only in its help popover
  const hasNoBranches = useBranchSync(state => state.branchOrder.length === 0)
  // dropping branch commits or branches moves them back: the prefix goes (a commit with an issue key goes back to its
  // issue branch), and an issue that the issue groups put into a branch goes back to its own
  const dropTarget = useCommitDropTarget(
    UNASSIGNED_SOURCE,
    drag => void moveCommits(drag.commits, null),
    drag => getRemovePrefixLabel(drag.commits, getIssueGroups()) !== null,
  )

  return (
    <Card
      className={cn("gap-0 overflow-hidden py-0", dropTarget.canDrop && "ring-1 ring-primary/40", dropTarget.isOver && "bg-primary/5 ring-2 ring-primary")}
      data-testid="unassigned-commits-section"
      data-drop-target={dropTarget.canDrop ? (dropTarget.isOver ? "over" : "ready") : undefined}
      {...dropTarget.dropProps}
    >
      <div className="border-b border-border px-4 py-2.5 sm:px-6">
        <SectionCardHeader
          title="Unassigned Commits"
          count={commits.length}
          itemSingular="commit"
          itemPlural="commits"
          badgeColor="neutral"
          help={commits.length > 0 && <UnassignedCommitsHelpPopover />}
          actions={
            commits.length > 0 && (
              <>
                {/* the selection's actions live in the card header, next to what they act on */}
                <CommitSelectionActions moveList={moveList} />
                <SquashFixupsButton branch={null} />
              </>
            )
          }
        />
        {hasNoBranches && (
          <p className="mt-2 text-sm text-muted-foreground" data-testid="unassigned-commits-hint">
            {"Start a commit subject with "}
            <InlineCode>(name)</InlineCode>
            {" or an issue key like "}
            <InlineCode>ABC-123</InlineCode>
            {" to group it into a branch, or select commits and move them to one."}
          </p>
        )}
      </div>

      <MovableCommitList moveList={moveList} commits={commits} variant="compact" showAuthor editable />
    </Card>
  )
}
