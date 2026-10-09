import { useEffect } from "react"
import { Spinner } from "@/components/ui/spinner"
import { CommitList } from "@/components/commit/CommitList"
import type { ArchivedBranch } from "@/stores/archived-branches"
import { loadCommitsForBranch } from "@/stores/archived-branches"

/** Commits of an archived branch, loaded on first expansion. */
export function ArchivedBranchExpanded({ branch }: { branch: ArchivedBranch }) {
  const { name } = branch
  useEffect(() => {
    // no-op if already loaded or loading
    void loadCommitsForBranch(name)
  }, [name])

  return (
    <div className="ml-4 border-l-2 border-primary/50 pr-6 pl-2">
      {branch.isLoadingCommits
        ? (
            <div className="flex items-center gap-2 text-sm text-muted-foreground">
              <Spinner className="size-4" />
              Loading commits…
            </div>
          )
        : branch.loadError
          ? <div className="text-destructive">{`Failed to load commits: ${branch.loadError}`}</div>
          : branch.commits && branch.commits.length > 0
            ? <CommitList commits={branch.commits} variant="compact" showFileCount={false} />
            : <div className="text-sm text-muted-foreground">No commits found</div>}
    </div>
  )
}
