import type { ReactNode } from "react"
import { PackageCheckIcon } from "lucide-react"
import { StatusChip } from "@/components/branch/StatusChip"
import type { LandedCommit } from "@/lib/bindings"
import { formatShortHash } from "@/lib/hash"
import { formatRelativeTime } from "@/lib/time"
import { useBaselineStatus } from "@/stores/branch-sync"
import { useBranchLanded, useIncomingLandedCommit } from "@/stores/landed"

/** "abc1234 · Patronus · 2 hours ago", or "squashed upstream" */
function describeUpstream(commit: LandedCommit): string {
  const upstream = commit.upstream
  return upstream === null ? "squashed upstream" : `${formatShortHash(upstream.id)} · ${upstream.committer} · ${formatRelativeTime(upstream.time * 1000)}`
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
        <LandedDetails heading={text} title={incoming ? `In ${upstream}: the update drops these commits` : `Landed in ${upstream} with the update`}>
          <dt className="text-muted-foreground">{count === 1 ? "Commit" : "Commits"}</dt>
          <dd className="flex min-w-0 flex-col gap-1" data-testid="landed-details-commits">
            {branch.commits.map(commit => (
              <span key={commit.id} className="flex min-w-0 flex-col">
                <span className="break-words">{commit.subject}</span>
                <span className="text-muted-foreground">{describeUpstream(commit)}</span>
              </span>
            ))}
          </dd>
          {branch.remaining > 0 && (
            <>
              <dt className="text-muted-foreground">Stays local</dt>
              <dd data-testid="landed-details-remaining">{branch.remaining === 1 ? "1 commit" : `${branch.remaining} commits`}</dd>
            </>
          )}
        </LandedDetails>
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
        <LandedDetails heading="Landed" title={`In ${upstream}: the update drops this commit`}>
          <dt className="text-muted-foreground">Landed as</dt>
          <dd className="break-words text-muted-foreground">{describeUpstream(commit)}</dd>
        </LandedDetails>
      )}
      detailsTestId="landed-commit-popover"
    >
      {`in ${upstream}`}
    </StatusChip>
  )
}

/**
 * The anatomy of the pull request and Patronus details: the state line (the package icon and the state in `info`), a
 * sentence on what it means, then `dl` rows
 */
function LandedDetails({ heading, title, children }: { heading: string, title: string, children: ReactNode }) {
  return (
    <div className="flex flex-col gap-3 text-sm" data-testid="landed-details">
      <div className="flex flex-col gap-1">
        <span className="flex items-center gap-1 text-xs font-medium text-info" data-testid="landed-details-state">
          <PackageCheckIcon className="size-3.5" />
          {heading}
        </span>
        <p className="leading-snug font-medium break-words">{title}</p>
      </div>
      <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1 text-xs">
        {children}
      </dl>
    </div>
  )
}
