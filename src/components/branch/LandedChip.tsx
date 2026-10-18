import type { ReactNode } from "react"
import { PackageCheckIcon } from "lucide-react"
import { StatusChip } from "@/components/branch/StatusChip"
import { CommitSubjectLine } from "@/components/commit/CommitSubject"
import { InlineCode } from "@/components/shared/InlineCode"
import { RelativeTime } from "@/components/shared/RelativeTime"
import { useUpstreamName } from "@/hooks/useLandedIn"
import type { LandedCommit } from "@/lib/bindings"
import { formatShortHash } from "@/lib/hash"
import { formatStayLocal, landedChipLabel } from "@/lib/landed"
import type { LandedBy } from "@/lib/patronus"
import { formatLandedWith, formatLandingSubject, formatPushMode, openRobot } from "@/lib/patronus"
import { useBranchLanded, useIncomingLandedCommit } from "@/stores/landed"

/** "abc1234 · Patronus · 2 hours ago" (the hash in the monospace font, the time aging while shown), or "squashed upstream" */
function UpstreamDescription({ commit }: { commit: LandedCommit }) {
  const upstream = commit.upstream
  if (upstream === null) {
    return "squashed upstream"
  }
  return (
    <>
      <InlineCode plain>{formatShortHash(upstream.id)}</InlineCode>
      {` · ${upstream.committer} · `}
      <RelativeTime time={upstream.time * 1000} />
    </>
  )
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
  const count = branch.commits.length
  const text = landedChipLabel(branch, incoming)
  const stays = branch.remaining > 0 ? ` · ${formatStayLocal(branch.remaining)}` : ""
  return (
    <StatusChip
      label={text === "Landed" ? `Landed in ${upstream}` : `${count} landed in ${upstream}${stays}`}
      color="info"
      data={{ "data-testid": "landed-chip", "data-incoming": incoming || undefined }}
      details={
        <LandedDetails heading={text} title={incoming ? `Already in ${upstream} · the update removes the local copies` : `Landed in ${upstream} with the update`}>
          <dt className="text-muted-foreground">{count === 1 ? "Commit" : "Commits"}</dt>
          <dd className="flex min-w-0 flex-col gap-1" data-testid="landed-details-commits">
            {branch.commits.map(commit => (
              <span key={commit.id} className="flex min-w-0 flex-col">
                <CommitSubjectLine subject={commit.subject} />
                <span className="text-muted-foreground">
                  <UpstreamDescription commit={commit} />
                </span>
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
      }
      detailsTestId="landed-popover"
    >
      <PackageCheckIcon className="size-3.5" />
      {text}
    </StatusChip>
  )
}

/**
 * `info` "Landed with oauth" ("Landed with the whole air stack") after the status of a branch a merged Safe Push of a
 * branch built on it or of the whole stack landed (`useLandedBy`), until the fetched baseline has its commits and the
 * Landed chip shows them; hover says when it merged, a click opens the robot in Patronus
 */
export function LandedWithChip({ landed }: { landed: LandedBy }) {
  const label = formatLandedWith(landed)
  const { robot } = landed
  return (
    <StatusChip
      label={label}
      color="info"
      data={{ "data-testid": "landed-with", "data-landed-by": landed.branch }}
      onActivate={() => openRobot(robot)}
      details={
        <LandedDetails
          heading="Landed"
          title={`In ${robot.targetBranch} with the ${formatPushMode(robot.pushMode)} of ${formatLandingSubject(landed)} · the update removes the local copies`}
        >
          {robot.finishTime !== null && (
            <>
              <dt className="text-muted-foreground">Merged</dt>
              <dd data-testid="landed-with-time">
                <RelativeTime time={robot.finishTime * 1000} />
              </dd>
            </>
          )}
        </LandedDetails>
      }
      detailsTestId="landed-with-popover"
    >
      <PackageCheckIcon className="size-3.5" />
      {label}
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
      details={
        <LandedDetails heading="Landed" title={`Already in ${upstream} · the update removes the local copy`}>
          <dt className="text-muted-foreground">Landed as</dt>
          <dd className="break-words text-muted-foreground">
            <UpstreamDescription commit={commit} />
          </dd>
        </LandedDetails>
      }
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
function LandedDetails({ heading, title, children }: { heading: string; title: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-3 text-sm" data-testid="landed-details">
      <div className="flex flex-col gap-1">
        <span className="flex items-center gap-1 text-xs font-medium text-info" data-testid="landed-details-state">
          <PackageCheckIcon className="size-3.5" />
          {heading}
        </span>
        <p className="leading-snug font-medium break-words">{title}</p>
      </div>
      <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1 text-xs">{children}</dl>
    </div>
  )
}
