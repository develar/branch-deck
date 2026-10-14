import type { ComponentType } from "react"
import { CircleCheckIcon, CircleDotIcon, CircleXIcon, ExternalLinkIcon, GitMergeIcon, GitPullRequestClosedIcon, GitPullRequestDraftIcon, GitPullRequestIcon, TriangleAlertIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { StatusChip, StatusChipDot } from "@/components/branch/StatusChip"
import { InlineCode } from "@/components/shared/InlineCode"
import type { ChecksState, PullRequest } from "@/lib/bindings"
import { formatShortHash } from "@/lib/hash"
import type { PullRequestIconKind } from "@/lib/pull-request"
import { formatChecks, formatReviewer, getPullRequestChipState, openPullRequest } from "@/lib/pull-request"
import { statusChipTextColors } from "@/lib/status-chip"
import { formatTimestamp } from "@/lib/time"
import { useBranchTip } from "@/stores/branch-sync"
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
 * `warning` dot when the pull request does not show the local branch yet. Hover shows the details, a click opens it on
 * GitHub. Nothing without a pull request.
 */
export function PullRequestChip({ branchName }: { branchName: string }) {
  const pullRequest = usePullRequest(branchName)
  const localTip = useBranchTip(branchName)
  if (pullRequest === undefined) {
    return null
  }

  const chip = getPullRequestChipState(pullRequest, localTip)
  const Icon = icons[chip.icon]
  return (
    <StatusChip
      label={`Pull request #${pullRequest.number}: ${chip.label}`}
      color={chip.color}
      className={cn(chip.icon === "draft" && "border-dashed border-muted-foreground/50")}
      data={{
        "data-testid": "pull-request-chip",
        "data-kind": chip.icon,
        "data-color": chip.color,
        "data-checks-failing": chip.checksFailing || undefined,
        "data-outdated": chip.outdated || undefined,
      }}
      details={<PullRequestDetails pullRequest={pullRequest} localTip={localTip} />}
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
  pullRequest: PullRequest
  localTip: string | undefined
}

/**
 * The state line (number, review decision), the title, the checks, the reviewers and the last update, a notice when the
 * pull request shows an older commit than the local branch, and "Open on GitHub". The chip popover and the branch
 * preview show it.
 */
export function PullRequestDetails({ pullRequest, localTip }: PullRequestDetailsProps) {
  const chip = getPullRequestChipState(pullRequest, localTip)
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

      {chip.outdated && (
        <p className="flex items-start gap-1.5 text-xs text-warning" data-testid="pull-request-outdated">
          <TriangleAlertIcon className="mt-px size-3.5 shrink-0" />
          <span>
            {"Not updated: the pull request shows "}
            <InlineCode plain>{formatShortHash(pullRequest.headRefOid)}</InlineCode>
            . Push to update it.
          </span>
        </p>
      )}

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
  const localTip = useBranchTip(branchName)
  if (pullRequest === undefined) {
    return null
  }
  return (
    <section aria-label="Pull request" className="rounded-lg border border-border p-3" data-testid="branch-preview-pull-request">
      <PullRequestDetails pullRequest={pullRequest} localTip={localTip} />
    </section>
  )
}
