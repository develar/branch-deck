import { Alert, AlertDescription } from "@/components/ui/alert"
import { CommitSelectionActions, MovableCommitList } from "@/components/commit/MovableCommitList"
import { SquashFixupsButton } from "@/components/commit/SquashFixupsButton"
import { hasSelectionActions, useCommitMoveList } from "@/hooks/useCommitMoveList"
import { useListFixups } from "@/hooks/useSquashFixups"
import type { Branch } from "@/stores/branch-sync"

export interface ActiveBranchExpandedProps {
  branch: Branch
  /** The amend form is open: mark the tip commit as the amend target */
  highlightTipCommit?: boolean
}

export function ActiveBranchExpanded({ branch, highlightTipCommit = false }: ActiveBranchExpandedProps) {
  const genericError = branch.errorDetails?.Generic
  return (
    // neutral rule under the folder icon; the subjects start at the branch name x
    <div className="ml-8 border-l border-border pr-6">
      {genericError !== undefined
        ? (
            <Alert variant="destructive" className="border-transparent bg-destructive/10">
              <AlertDescription>{genericError}</AlertDescription>
            </Alert>
          )
        : branch.commits.length > 0
          ? <BranchCommitList branch={branch} highlightTipCommit={highlightTipCommit} />
          : <div className="text-sm text-muted-foreground">No commits found</div>}
    </div>
  )
}

/** The commits of the branch; select them to move them to another branch (or drag them onto one). */
function BranchCommitList({ branch, highlightTipCommit }: { branch: Branch, highlightTipCommit: boolean }) {
  const moveList = useCommitMoveList({ commits: branch.commits, sourceBranch: branch.name })
  const { count: fixupCount } = useListFixups(branch.name)
  return (
    <div className="flex flex-col gap-2">
      {(hasSelectionActions(moveList) || fixupCount > 0) && (
        <div className="flex items-center justify-end gap-2 px-4 sm:px-6">
          <CommitSelectionActions moveList={moveList} />
          <SquashFixupsButton branch={branch.name} />
        </div>
      )}
      <MovableCommitList moveList={moveList} commits={branch.commits} variant="status" showFileCount={false} highlightTipCommit={highlightTipCommit} editable />
    </div>
  )
}
