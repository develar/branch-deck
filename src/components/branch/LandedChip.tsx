import { PackageCheckIcon } from "lucide-react"
import { StatusChip } from "@/components/branch/StatusChip"
import { RelativeTime } from "@/components/shared/RelativeTime"
import type { LandedCommit } from "@/lib/bindings"
import { formatShortHash } from "@/lib/hash"
import { useBaselineStatus } from "@/stores/branch-sync"
import { useBranchLanded, useIncomingLandedCommit } from "@/stores/landed"

/** "abc1234 · Patronus · 2 hours ago", or "squashed upstream" */
function UpstreamDescription({ commit }: { commit: LandedCommit }) {
  const upstream = commit.upstream
  if (upstream === null) {
    return "squashed upstream"
  }
  return (
    <>
      {`${formatShortHash(upstream.id)} · ${upstream.committer} · `}
      <RelativeTime time={upstream.time * 1000} />
    </>
  )
}

function useUpstreamName(): string {
  const status = useBaselineStatus()
  return status?.update?.upstream ?? status?.baselineBranch ?? "the baseline"
}

/**
 * `info` "Landed" after the status of an active branch whose commits are in the baseline: before the update, "Landed"
 * (all of them) or "3 of 12 landed"; after it, "3 landed" for the part that landed while the rest stays. Hover lists
 * the commits with the baseline commits that carry them. Nothing otherwise.
 */
export function LandedChip({ branchName }: { branchName: string }) {
  const landed = useBranchLanded(branchName)
  const upstream = useUpstreamName()
  if (landed === null) {
    return null
  }

  const { branch, incoming } = landed
  const count = branch.commits.length
  const text = incoming
    ? (branch.remaining === 0 ? "Landed" : `${count} of ${count + branch.remaining} landed`)
    : `${count} landed`
  return (
    <StatusChip
      label={`${text} in ${upstream}`}
      color="info"
      data={{ "data-testid": "landed-chip", "data-incoming": incoming || undefined }}
      details={(
        <div className="flex flex-col gap-2 text-xs">
          <span className="font-medium">{incoming ? `In ${upstream}: the update drops these commits` : `Landed in ${upstream} with the update`}</span>
          <ul className="flex flex-col gap-1">
            {branch.commits.map(commit => (
              <li key={commit.id} className="flex flex-col">
                <span className="truncate">{commit.subject}</span>
                <span className="text-muted-foreground"><UpstreamDescription commit={commit} /></span>
              </li>
            ))}
          </ul>
          {branch.remaining > 0 && <span className="text-muted-foreground">{`${branch.remaining} stay local`}</span>}
        </div>
      )}
      detailsTestId="landed-popover"
    >
      <PackageCheckIcon className="size-3.5" />
      {text}
    </StatusChip>
  )
}

/** "in origin/master" after the subject of a local commit the fetched baseline already has, with its baseline commit */
export function LandedCommitMarker({ commitId }: { commitId: string }) {
  const commit = useIncomingLandedCommit(commitId)
  const upstream = useUpstreamName()
  if (commit === null) {
    return null
  }
  return (
    <StatusChip
      label={`In ${upstream}`}
      color="info"
      data={{ "data-testid": "landed-commit-marker" }}
      details={(
        <div className="flex flex-col gap-1 text-xs">
          <span className="font-medium">{`In ${upstream}: the update drops this commit`}</span>
          <span className="text-muted-foreground"><UpstreamDescription commit={commit} /></span>
        </div>
      )}
      detailsTestId="landed-commit-popover"
    >
      {`in ${upstream}`}
    </StatusChip>
  )
}
