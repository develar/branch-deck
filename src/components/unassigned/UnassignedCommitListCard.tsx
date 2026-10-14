import { cn } from "cn"
import { Card } from "@/components/ui/card"
import { CommitSelectionActions, MovableCommitList } from "@/components/commit/MovableCommitList"
import { SquashFixupsButton } from "@/components/commit/SquashFixupsButton"
import { SectionCardHeader } from "@/components/shared/SectionCardHeader"
import { UnassignedCommitsHelpPopover } from "@/components/unassigned/UnassignedCommitsHelpPopover"
import { useCommitDropTarget } from "@/hooks/useCommitDrag"
import { getRemovePrefixLabel, useCommitMoveList } from "@/hooks/useCommitMoveList"
import { moveCommits } from "@/hooks/useMoveCommits"
import type { Commit } from "@/lib/bindings"
import { UNASSIGNED_SOURCE } from "@/stores/commit-move"

export interface UnassignedCommitListCardProps {
  commits: Commit[]
}

/** Commits that match no grouping pattern; select them to move them to a branch, or drop branch commits here. */
export function UnassignedCommitListCard({ commits }: UnassignedCommitListCardProps) {
  const moveList = useCommitMoveList({ commits, sourceBranch: null })
  // dropping branch commits removes their prefix (a commit with an issue key goes back to its issue branch)
  const dropTarget = useCommitDropTarget(
    UNASSIGNED_SOURCE,
    drag => void moveCommits(drag.commits, null),
    drag => getRemovePrefixLabel(drag.commits) !== null,
  )

  return (
    <Card
      className={cn("gap-0 overflow-hidden py-0", dropTarget.canDrop && "ring-1 ring-primary/40", dropTarget.isOver && "bg-primary/5 ring-2 ring-primary")}
      data-testid="unassigned-commits-section"
      data-drop-target={dropTarget.canDrop ? (dropTarget.isOver ? "over" : "ready") : undefined}
      {...dropTarget.dropProps}
    >
      <div className="border-b border-border p-4 sm:px-6">
        <SectionCardHeader
          title="Unassigned Commits"
          count={commits.length}
          itemSingular="commit"
          itemPlural="commits"
          badgeColor="neutral"
          actions={commits.length > 0 && (
            <>
              {/* the selection's actions live in the card header, next to what they act on */}
              <CommitSelectionActions moveList={moveList} />
              <SquashFixupsButton branch={null} />
              <UnassignedCommitsHelpPopover />
            </>
          )}
        />
      </div>

      <MovableCommitList moveList={moveList} commits={commits} variant="compact" showAuthor editable />
    </Card>
  )
}
