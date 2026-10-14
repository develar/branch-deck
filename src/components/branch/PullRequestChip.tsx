import type { ComponentType } from "react"
import { CircleCheckIcon, CircleDotIcon, CircleXIcon, ExternalLinkIcon, GitMergeIcon, GitPullRequestClosedIcon, GitPullRequestDraftIcon, GitPullRequestIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { ReviewCommitNotice } from "@/components/branch/ReviewCommitNotice"
import { StatusChip, StatusChipDot } from "@/components/branch/StatusChip"
import type { ChecksState, PullRequest } from "@/lib/bindings"
import type { PullRequestIconKind } from "@/lib/pull-request"
import { formatChecks, formatPullRequestCommitNotice, formatReviewer, getPullRequestChipState, openPullRequest } from "@/lib/pull-request"
import { statusChipTextColors } from "@/lib/status-chip"
import { formatTimestamp } from "@/lib/time"
import { useBranchRemoteStatus, useBranchTip } from "@/stores/branch-sync"
import { usePullRequest } from "@/stores/pull-requests"

const icons: Readonly<Record<PullRequestIconKind, ComponentType<{ className?: string }>>> = {
  open: GitPullRequestIcon,
  draft: GitPullRequestDraftIcon,
  merged: GitMergeIcon,
  closed: GitPullRequestClosedIcon,
}

const checksIcons: Readonly<Record<ChecksState, { icon: ComponentType<{ className?: string }>, className: string }>> = {
  passing: { icon: CircleCheckIcon, className: "text-success" },
  failing: { icon: CircleXIcon, className: "text-destructive" },
  pending: { icon: CircleDotIcon, className: "text-warning" },
}

/**
 * `#760` with the pull request icon, in the hue of the review decision; a `destructive` dot for failing checks and a
 * `warning` dot when the branch changed since the pull request's head (a branch only rebuilt on a newer baseline did
 * not). Hover shows the details, a click opens it on GitHub. Nothing without a pull request.
 */
export function PullRequestChip({ branchName }: { branchName: string }) {
  const pullRequest = usePullRequest(branchName)
  const localTip = useBranchTip(branchName)
  const remote = useBranchRemoteStatus(branchName)
  if (pullRequest === undefined) {
    return null
  }

  const chip = getPullRequestChipState(pullRequest, localTip, remote)
  const Icon = icons[chip.icon]
  return (
    <StatusChip
      label={`Pull request #${pullRequest.number}: ${chip.label}`}
      color={chip.color}
      data={{
        "data-testid": "pull-request-chip",
        "data-kind": chip.icon,
        "data-color": chip.color,
        "data-checks-failing": chip.checksFailing || undefined,
        "data-outdated": chip.outdated || undefined,
      }}
      details={<PullRequestDetails branchName={branchName} pullRequest={pullRequest} />}
      detailsTestId="pull-request-popover"
      onActivate={() => openPullRequest(pullRequest)}
    >
      <Icon className="size-3.5" />
      {`#${pullRequest.number}`}
      {chip.checksFailing && <StatusChipDot color="error" testId="pull-request-checks-dot" />}
      {chip.outdated && <StatusChipDot color="warning" testId="pull-request-outdated-dot" />}
    </StatusChip>
  )
}

export interface PullRequestDetailsProps {
  branchName: string
  pullRequest: PullRequest
}

/**
 * The state line (number, review decision), the title, the checks, the reviewers and the last update, a notice when the
 * branch changed since the pull request's head (a quiet line when it was only rebuilt on a newer baseline), and "Open on
 * GitHub". The chip popover and the branch preview show it.
 */
export function PullRequestDetails({ branchName, pullRequest }: PullRequestDetailsProps) {
  const localTip = useBranchTip(branchName)
  const remote = useBranchRemoteStatus(branchName)
  const chip = getPullRequestChipState(pullRequest, localTip, remote)
  const Icon = icons[chip.icon]
  const checks = pullRequest.checks
  return (
    <div className="flex flex-col gap-3 text-sm" data-testid="pull-request-details">
      <div className="flex flex-col gap-1">
        <div className="flex items-center gap-2 text-xs">
          <span className={cn("flex items-center gap-1 font-medium tabular-nums", statusChipTextColors[chip.color])}>
            <Icon className="size-3.5" />
            {`#${pullRequest.number}`}
          </span>
          <span className={cn("whitespace-nowrap", statusChipTextColors[chip.color])} data-testid="pull-request-state">{chip.label}</span>
        </div>
        <p className="leading-snug font-medium break-words" data-testid="pull-request-title">{pullRequest.title}</p>
      </div>

      {(checks !== null || pullRequest.reviewers.length > 0 || pullRequest.updatedAt > 0) && (
        <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-1 text-xs">
          {checks !== null && (
            <>
              <dt className="text-muted-foreground">Checks</dt>
              <dd className="flex min-w-0 items-start gap-1.5" data-testid="pull-request-checks">
                <ChecksIcon state={checks.state} />
                <span className="break-words">{formatChecks(checks)}</span>
              </dd>
            </>
          )}
          {pullRequest.reviewers.length > 0 && (
            <>
              <dt className="text-muted-foreground">Reviewers</dt>
              <dd className="flex min-w-0 flex-col" data-testid="pull-request-reviewers">
                {pullRequest.reviewers.map(reviewer => (
                  <span key={reviewer.login} className={cn("break-words", reviewer.state === "changesRequested" && "text-destructive", reviewer.state === "approved" && "text-success")}>
                    {formatReviewer(reviewer)}
                  </span>
                ))}
              </dd>
            </>
          )}
          {pullRequest.updatedAt > 0 && (
            <>
              <dt className="text-muted-foreground">Updated</dt>
              <dd className="text-muted-foreground">{formatTimestamp(pullRequest.updatedAt)}</dd>
            </>
          )}
        </dl>
      )}

      <ReviewCommitNotice text={formatPullRequestCommitNotice(pullRequest, chip, remote?.remoteTip ?? null)} outdated={chip.outdated} testId="pull-request" />

      <div>
        <Button variant="outline" size="xs" onClick={() => openPullRequest(pullRequest)}>
          <ExternalLinkIcon data-icon="inline-start" />
          Open on GitHub
        </Button>
      </div>
    </div>
  )
}

function ChecksIcon({ state }: { state: ChecksState }) {
  const { icon: StateIcon, className } = checksIcons[state]
  return <StateIcon className={cn("mt-px size-3.5 shrink-0", className)} />
}

/** The pull request of the branch in the branch preview, above the commits; nothing without one */
export function BranchPullRequestSection({ branchName }: { branchName: string }) {
  const pullRequest = usePullRequest(branchName)
  if (pullRequest === undefined) {
    return null
  }
  return (
    <section aria-label="Pull request" className="rounded-lg border border-border p-3" data-testid="branch-preview-pull-request">
      <PullRequestDetails branchName={branchName} pullRequest={pullRequest} />
    </section>
  )
}
