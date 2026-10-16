import { useEffect } from "react"
import { GitCommitHorizontalIcon } from "lucide-react"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { CommitList } from "@/components/commit/CommitList"
import { CopyButton } from "@/components/shared/CopyButton"
import { InlineCode } from "@/components/shared/InlineCode"
import type { UpstreamCommit } from "@/lib/bindings"
import { formatShortHash } from "@/lib/hash"
import { describeIntegrationCommit } from "@/lib/integration-commit"
import type { ArchivedBranch } from "@/stores/archived-branches"
import { loadCommitsForBranch } from "@/stores/archived-branches"

/** Commits of an archived branch, loaded on first expansion, below the baseline commit that merged it. */
export function ArchivedBranchExpanded({ branch }: { branch: ArchivedBranch }) {
  const { name } = branch
  useEffect(() => {
    // no-op if already loaded or loading
    void loadCommitsForBranch(name)
  }, [name])

  return (
    // neutral rule under the folder icon; the subjects start at the branch name x
    <div className="ml-8 flex animate-in flex-col gap-2 border-l border-border pr-6 duration-150 fade-in-0">
      {branch.integrationCommit !== undefined && <IntegrationCommitLine commit={branch.integrationCommit} />}
      {branch.isLoadingCommits ? (
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <Spinner className="size-4" />
          Loading commits…
        </div>
      ) : branch.loadError ? (
        <p className="text-sm text-destructive">{`Cannot load the commits: ${branch.loadError}`}</p>
      ) : branch.commits && branch.commits.length > 0 ? (
        <CommitList commits={branch.commits} variant="compact" showFileCount={false} />
      ) : (
        <div className="text-sm text-muted-foreground">No commits found</div>
      )}
    </div>
  )
}

/** "Merged as abc12345 by …" (the row shows the time), the author on hover, with a button that copies the full hash */
function IntegrationCommitLine({ commit }: { commit: UpstreamCommit }) {
  const hash = formatShortHash(commit.id)
  const [before, after] = describeIntegrationCommit(commit).split(hash)
  return (
    <div className="group flex items-center gap-2 pl-4 text-sm text-muted-foreground" data-testid="archived-branch-integration-commit">
      <GitCommitHorizontalIcon className="size-4 shrink-0 text-info" />
      <Tooltip>
        <TooltipTrigger render={<span className="truncate" />}>
          {before}
          <InlineCode plain>{hash}</InlineCode>
          {after}
        </TooltipTrigger>
        <TooltipContent>{`Authored by ${commit.author}`}</TooltipContent>
      </Tooltip>
      <CopyButton text={() => commit.id} tooltip="Copy the commit hash" />
    </div>
  )
}
