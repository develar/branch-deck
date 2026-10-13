import { Alert, AlertDescription } from "@/components/ui/alert"
import { CommitList } from "@/components/commit/CommitList"
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
          ? <CommitList commits={branch.commits} variant="status" showFileCount={false} highlightTipCommit={highlightTipCommit} editable />
          : <div className="text-sm text-muted-foreground">No commits found</div>}
    </div>
  )
}
