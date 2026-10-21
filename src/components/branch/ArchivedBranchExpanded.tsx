import { useEffect } from "react"
import { GitCommitHorizontalIcon } from "lucide-react"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { KeptBranchCommits } from "@/components/branch/KeptBranchCommits"
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
      <KeptBranchCommits isLoading={branch.isLoadingCommits} loadError={branch.loadError} commits={branch.commits} />
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
