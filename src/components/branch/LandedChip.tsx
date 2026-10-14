import { PackageCheckIcon } from "lucide-react"
import { StatusChip } from "@/components/branch/StatusChip"
import { CommitSubjectLine } from "@/components/commit/CommitSubject"
import type { LandedCommit } from "@/lib/bindings"
import { formatShortHash } from "@/lib/hash"
import { formatStayLocal, landedChipLabel } from "@/lib/landed"
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
 * (all of them) or "3 landed · 9 local"; after it, the same for the part that landed while the rest stays. Hover lists
 * the commits with the baseline commits that carry them. Nothing otherwise.
 */
export function LandedChip({ branchName }: { branchName: string }) {
  const landed = useBranchLanded(branchName)
  const upstream = useUpstreamName()
  if (landed === null) {
    return null
  }

  const { branch, incoming } = landed
  const text = landedChipLabel(branch, incoming)
  const stays = branch.remaining > 0 ? ` · ${formatStayLocal(branch.remaining)}` : ""
  return (
    <StatusChip
      label={text === "Landed" ? `Landed in ${upstream}` : `${branch.commits.length} landed in ${upstream}${stays}`}
      color="info"
      data={{ "data-testid": "landed-chip", "data-incoming": incoming || undefined }}
      details={(
        <div className="flex flex-col gap-2 text-xs">
          <span className="font-medium">{incoming ? `Already in ${upstream} · the update removes the local copies` : `Landed in ${upstream} with the update`}</span>
          <ul className="flex flex-col gap-1">
            {branch.commits.map(commit => (
              <li key={commit.id} className="flex flex-col">
                <CommitSubjectLine subject={commit.subject} />
                <span className="text-muted-foreground">{describeUpstream(commit)}</span>
              </li>
            ))}
          </ul>
          {branch.remaining > 0 && <span className="text-muted-foreground">{formatStayLocal(branch.remaining)}</span>}
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
          <span className="font-medium">{`Already in ${upstream} · the update removes the local copy`}</span>
          <span className="text-muted-foreground">{describeUpstream(commit)}</span>
        </div>
      )}
      detailsTestId="landed-commit-popover"
    >
      {`in ${upstream}`}
    </StatusChip>
  )
}
