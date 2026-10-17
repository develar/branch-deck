import { useState } from "react"
import { cn } from "cn"
import { ChevronRightIcon, CircleCheckIcon, ExternalLinkIcon, FileDiffIcon, GitPullRequestCreateIcon, RefreshCwIcon, UserPlusIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { RequestReviewPopover } from "@/components/branch/RequestReviewPopover"
import { Callout } from "@/components/shared/Callout"
import type { ReviewRequirement } from "@/hooks/usePatronusReviewRequirement"
import type { MissingReviewGroup, PatronusReviewerGroup, PatronusReviewRequirement, Person, PullRequest, ReviewReadiness } from "@/lib/bindings"
import { formatMoreApprovals, formatPerson } from "@/lib/branch-next-step"
import { openBranchReviewWindow, toReviewGroups } from "@/lib/branch-review-window"
import { CREATE_PULL_REQUEST_LABEL } from "@/lib/git/new-pull-request"
import { formatReviewerGroup, formatReviewerGroupFiles } from "@/lib/patronus"
import { openMergeRequest } from "@/lib/patronus-merge-request"
import { openPullRequest } from "@/lib/pull-request"
import { getRequestReviewItem } from "@/lib/request-review"
import { joinNames } from "@/lib/text"
import { useBranch } from "@/stores/branch-sync"

export interface PatronusReviewBlockProps {
  /** The branch of the Safe Push: its review window shows the files of a group */
  branchName: string
  review: ReviewRequirement
  /**
   * The GitHub repository of the branches is known (`RepositoryPatronus.githubRepository`), with the pull request of
   * the branch if it has one and the target: Patronus counts the approvals of the pull request the robot starts from.
   * null = unknown: no pull request can be opened, as Space merge requests are deprecated.
   */
  github: { pullRequest: PullRequest | undefined; target: string } | null
  /** Pushes the branch if needed and opens the GitHub form of its pull request */
  onCreatePullRequest: () => Promise<void>
  /** While the push of Create Pull Request runs, its label ("Pushing 45%"): the button shows it, the row's doesn't */
  pushLabel: string | null
}

/**
 * The code-owner review of a Safe Push, as Patronus judges it for the start the form makes (`ReviewReadiness`): nothing
 * needed; approved, with whose approvals count; the groups still short of approvals, each with who is asked, who
 * requested changes and how many more count, the approvals Patronus doesn't count, and why a start through the API
 * counts none; or unknown, naming the approvals nobody but Patronus can judge yet. Patronus checks the approvals once,
 * when the robot starts, and fails without them at once: with approvals missing the form's Start is disabled. The
 * actions: Request Review… (in the pull request), Open Pull Request or Create Pull Request…, Review Changes, Recheck.
 * Off GitHub (a repository only on Space) the merge request path stays: an open merge request found counts and opens.
 */
export function PatronusReviewBlock({ branchName, review, github, onCreatePullRequest, pushLabel }: PatronusReviewBlockProps) {
  const { state } = review
  if (state.status === "loading") {
    return (
      <p className="flex items-center gap-2 text-sm text-muted-foreground" data-testid="patronus-review" data-status="loading">
        <Spinner className="size-4" />
        Checking the code-owner review…
      </p>
    )
  }
  if (state.status === "error") {
    return <p className="text-sm text-muted-foreground" data-testid="patronus-review" data-status="error">{`Review check: ${state.message}`}</p>
  }

  const { requirement, readiness } = state
  if (requirement.skipped) {
    return (
      <p className="text-sm text-muted-foreground" data-testid="patronus-review" data-status="skipped">
        Too many commits to check the review before the start; the robot checks it.
      </p>
    )
  }
  if (github === null) {
    return <SpaceReviewBlock branchName={branchName} review={review} requirement={requirement} />
  }
  const pullRequest = github.pullRequest
  switch (readiness.kind) {
    case "notRequired":
      return (
        <p className="flex items-center gap-1.5 text-sm text-muted-foreground" data-testid="patronus-review" data-status="notRequired">
          <CircleCheckIcon className="size-4 shrink-0 text-success" />
          No code-owner review needed.
        </p>
      )
    case "approved":
      return (
        <p className="flex items-start gap-1.5 text-sm text-muted-foreground" data-testid="patronus-review" data-status="approved">
          <CircleCheckIcon className="mt-0.5 size-4 shrink-0 text-success" />
          <span>{formatApproved(readiness.groups)}</span>
        </p>
      )
    case "unknown":
    case "unavailable":
      return (
        <Callout tone="info" title="Patronus judges the approvals when the robot starts" data-testid="patronus-review" data-status={readiness.kind}>
          <p data-testid="patronus-review-unknown">{formatUnknown(readiness, pullRequest)}</p>
          <ReviewerGroups branchName={branchName} requirement={requirement} />
          <ReviewActions
            branchName={branchName}
            review={review}
            requirement={requirement}
            pullRequest={pullRequest}
            onCreatePullRequest={onCreatePullRequest}
            pushLabel={pushLabel}
          />
        </Callout>
      )
    case "missing":
      return (
        <Callout tone="warning" title="Needs review by the code owners" data-testid="patronus-review" data-status="missing">
          <MissingGroups branchName={branchName} requirement={requirement} groups={readiness.groups} />
          {readiness.notCounted.length > 0 && <p data-testid="patronus-review-not-counted">{formatNotCounted(readiness.notCounted)}</p>}
          <p data-testid="patronus-review-reason">{formatMissingReason(readiness, pullRequest, github.target)}</p>
          <ReviewActions
            branchName={branchName}
            review={review}
            requirement={requirement}
            pullRequest={pullRequest}
            onCreatePullRequest={onCreatePullRequest}
            pushLabel={pushLabel}
            requestReview={readiness.reason !== "noPullRequestContext" && readiness.groups.some(group => group.requested.length === 0)}
            readiness={readiness}
          />
        </Callout>
      )
  }
}

/** "Approved by Ilya Korennoy for IJPL Code Platform; by Alice for Air Platform" */
function formatApproved(groups: readonly { name: string; approvedBy: Person[] }[]): string {
  const parts = groups.map(group => (group.approvedBy.length === 0 ? `for ${group.name}` : `by ${joinNames(group.approvedBy.map(formatPerson))} for ${group.name}`))
  return `Approved ${parts.join("; ")}.`
}

/** "junie-jetbrains approved, but isn't a code owner of these files: Patronus doesn't count it" */
function formatNotCounted(people: readonly Person[]): string {
  const names = joinNames(people.map(formatPerson))
  return people.length === 1
    ? `${names} approved, but isn't a code owner of these files: Patronus doesn't count it.`
    : `${names} approved, but aren't code owners of these files: Patronus doesn't count them.`
}

/** What a group still lacks: "Ilya Korennoy requested, not approved yet", "1 more approval", whom Patronus suggests */
function formatGroupState(group: MissingReviewGroup): string {
  const parts: string[] = []
  if (group.changesRequestedBy.length > 0) {
    parts.push(`${joinNames(group.changesRequestedBy.map(formatPerson))} requested changes`)
  }
  if (group.requested.length > 0) {
    parts.push(`${joinNames(group.requested.map(formatPerson))} requested, not approved yet`)
  } else if (group.changesRequestedBy.length === 0) {
    const suggested = group.suggested.map(formatPerson)
    parts.push(suggested.length === 0 ? "nobody requested" : `nobody requested; Patronus suggests ${joinNames(suggested)}`)
  }
  if (group.approvedBy.length > 0) {
    parts.push(`approved by ${joinNames(group.approvedBy.map(formatPerson))}`)
  }
  if (group.needed !== null && (group.needed > 1 || group.approvedBy.length > 0)) {
    parts.push(formatMoreApprovals(group.needed))
  }
  return parts.join(" · ")
}

/** Why the approvals are missing: who has to approve where, or why none counts for this start */
function formatMissingReason(readiness: Extract<ReviewReadiness, { kind: "missing" }>, pullRequest: PullRequest | undefined, target: string): string {
  switch (readiness.reason) {
    case "noEligibleReviewer":
      return `Nobody of ${joinNames(readiness.groups.map(group => group.name))} can approve these changes: Patronus counts no member's approval for them.`
    case "noPullRequestContext":
      return pullRequest?.state === "open"
        ? `A Safe Push started without the pull request counts no approvals: Patronus counts the approvals of #${pullRequest.number} only for a robot started from it, and only while it merges into ${target}.`
        : "A Safe Push started without a pull request counts no approvals: Patronus counts them only in the pull request the robot starts from."
    case "approvalsMissing":
      return `Patronus checks the approvals when the robot starts and fails without them${pullRequest === undefined ? "." : `: they count in #${pullRequest.number}.`}`
  }
}

/** "Patronus doesn't tell yet whose approvals count: Branch Deck can't check junie-jetbrains's approval." */
function formatUnknown(readiness: Extract<ReviewReadiness, { kind: "unknown" | "unavailable" }>, pullRequest: PullRequest | undefined): string {
  if (readiness.kind === "unavailable") {
    return `Patronus didn't answer the review check (${readiness.message}): the robot checks the approvals when it starts.`
  }
  const approvers = pullRequest?.reviewers.filter(reviewer => reviewer.state === "approved").map(reviewer => reviewer.login) ?? []
  if (approvers.length === 0) {
    return "Patronus doesn't tell yet whose approvals count: the robot checks them when it starts."
  }
  const names = joinNames(approvers)
  return `Patronus doesn't tell yet whose approvals count: Branch Deck can't check ${approvers.length === 1 ? `${names}'s approval` : `the approvals of ${names}`}.`
}

/** The groups still short of approvals, each with what it lacks and the files it reviews */
function MissingGroups({ branchName, requirement, groups }: { branchName: string; requirement: PatronusReviewRequirement; groups: readonly MissingReviewGroup[] }) {
  return (
    <ul className="flex flex-col gap-0.5 text-foreground" data-testid="patronus-review-groups">
      {groups.map(group => {
        const known = requirement.groups.find(it => it.name === group.name)
        return (
          <GroupItem
            key={group.name}
            branchName={branchName}
            requirement={requirement}
            group={known ?? { name: group.name, reviewers: [], paths: [], branchWide: false, pathsSource: "unknown" }}
            label={group.name}
            state={formatGroupState(group)}
          />
        )
      })}
    </ul>
  )
}

/**
 * The groups that still have to approve, as Patronus names them with the reviewers it suggests (an answer without the
 * evaluation of a pull request)
 */
function ReviewerGroups({ branchName, requirement }: { branchName: string; requirement: PatronusReviewRequirement }) {
  const groups = requirement.groups.filter(group => group.approved !== true)
  if (groups.length === 0) {
    return null
  }
  return (
    <ul className="flex flex-col gap-0.5 text-foreground" data-testid="patronus-review-groups">
      {groups.map(group => (
        <GroupItem key={group.name} branchName={branchName} requirement={requirement} group={group} label={formatReviewerGroup(group)} state={null} />
      ))}
    </ul>
  )
}

/**
 * A group with the files it has to review ("· 2 files", a disclosure of the paths; "· all files" for a branch-wide group),
 * then what it lacks; a group Patronus names without files and that owns no file by its name in the `OWNERSHIP` files
 * is only named. A file opens the review window at it.
 */
function GroupItem({
  branchName,
  requirement,
  group,
  label,
  state,
}: {
  branchName: string
  requirement: PatronusReviewRequirement
  group: PatronusReviewerGroup
  label: string
  state: string | null
}) {
  const files = formatReviewerGroupFiles(group)
  const suffix = state === null ? null : <span className="text-muted-foreground">{`: ${state}`}</span>
  // the files of a group, from where Patronus checks them: the whole stack down to the baseline
  const open = (path: string | null) =>
    void openBranchReviewWindow(branchName, { range: "all", filter: { kind: "group", name: group.name }, path, groups: toReviewGroups(requirement) })
  if (files === null) {
    return (
      <li data-testid="patronus-review-group" data-name={group.name}>
        {label}
        {suffix}
      </li>
    )
  }
  if (group.branchWide) {
    return (
      <li data-testid="patronus-review-group" data-name={group.name}>
        {label}
        <button type="button" className="ml-1 text-muted-foreground underline-offset-4 hover:text-foreground hover:underline" onClick={() => open(null)}>{`· ${files}`}</button>
        {suffix}
      </li>
    )
  }
  return (
    <li data-testid="patronus-review-group" data-name={group.name}>
      <Collapsible className="flex flex-col gap-0.5">
        <span className="flex w-fit flex-wrap items-center gap-x-1">
          <CollapsibleTrigger className="group flex items-center gap-1 text-left" data-testid="patronus-review-group-toggle">
            <ChevronRightIcon className="size-3.5 shrink-0 text-muted-foreground transition-transform group-data-[panel-open]:rotate-90" />
            <span>{label}</span>
            <span className="text-muted-foreground">{`· ${files}`}</span>
          </CollapsibleTrigger>
          {suffix}
        </span>
        <CollapsibleContent>
          <ul className="flex flex-col pl-5 text-xs" data-testid="patronus-review-group-files">
            {group.paths.map(path => (
              <li key={path} className="min-w-0">
                <button
                  type="button"
                  className="max-w-full truncate text-left text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
                  title={`Review ${path}`}
                  onClick={() => open(path)}
                >
                  {path}
                </button>
              </li>
            ))}
          </ul>
        </CollapsibleContent>
      </Collapsible>
    </li>
  )
}

/** Opens the review window of the branch with everything Patronus checks, the reviewer groups as filters */
function ReviewChangesButton({ branchName, requirement }: { branchName: string; requirement: PatronusReviewRequirement }) {
  return (
    <Button
      variant="outline"
      size="xs"
      data-testid="patronus-review-changes"
      onClick={() => void openBranchReviewWindow(branchName, { range: "all", groups: toReviewGroups(requirement) })}
    >
      <FileDiffIcon data-icon="inline-start" />
      Review Changes
    </Button>
  )
}

/**
 * The actions of the review on GitHub: Request Review… in the open pull request while a missing group has nobody asked,
 * Open Pull Request #7 (also a merged one, never a Create that would push and open nothing), Create Pull Request…
 * without one (the branch is pushed first), Review Changes and Recheck
 */
function ReviewActions({
  branchName,
  review,
  requirement,
  pullRequest,
  onCreatePullRequest,
  pushLabel,
  requestReview = false,
  readiness = null,
}: {
  branchName: string
  review: ReviewRequirement
  requirement: PatronusReviewRequirement
  pullRequest: PullRequest | undefined
  onCreatePullRequest: () => Promise<void>
  pushLabel: string | null
  requestReview?: boolean
  readiness?: ReviewReadiness | null
}) {
  const [isCreating, setCreating] = useState(false)
  // a merged pull request keeps the branch from a new one (`canCreatePullRequest`): it opens instead
  const live = pullRequest?.state === "closed" ? undefined : pullRequest
  return (
    <div className="flex flex-wrap gap-2">
      {requestReview && live?.state === "open" && <RequestReviewAction branchName={branchName} pullRequest={live} readiness={readiness} />}
      {live === undefined ? (
        <Button
          variant="outline"
          size="xs"
          className="tabular-nums"
          disabled={isCreating}
          aria-busy={isCreating || undefined}
          data-testid="patronus-create-pull-request"
          onClick={() => {
            setCreating(true)
            void onCreatePullRequest().finally(() => setCreating(false))
          }}
        >
          {isCreating ? <Spinner data-icon="inline-start" /> : <GitPullRequestCreateIcon data-icon="inline-start" />}
          {pushLabel ?? CREATE_PULL_REQUEST_LABEL}
        </Button>
      ) : (
        <Button variant="outline" size="xs" data-testid="patronus-open-pull-request" onClick={() => openPullRequest(live)}>
          <ExternalLinkIcon data-icon="inline-start" />
          {`Open Pull Request #${live.number}`}
        </Button>
      )}
      <ReviewChangesButton branchName={branchName} requirement={requirement} />
      <RecheckButton review={review} />
    </div>
  )
}

/**
 * "Request Review…" of the open pull request (`getRequestReviewItem`): its popover, or the button disabled with the
 * reason (a draft, work to push)
 */
function RequestReviewAction({ branchName, pullRequest, readiness }: { branchName: string; pullRequest: PullRequest; readiness: ReviewReadiness | null }) {
  const branch = useBranch(branchName)
  const item = branch === undefined ? null : getRequestReviewItem(branch, pullRequest, readiness, () => {})
  if (branch === undefined || item === null) {
    return null
  }
  const button = (
    <Button variant="outline" size="xs" disabled={item.disabled} focusableWhenDisabled className={cn(item.disabled === true && "opacity-50")} data-testid="patronus-request-review">
      <UserPlusIcon data-icon="inline-start" />
      {item.label}
    </Button>
  )
  if (item.disabled === true) {
    return (
      <Tooltip>
        <TooltipTrigger render={button} />
        <TooltipContent>{item.description}</TooltipContent>
      </Tooltip>
    )
  }
  return <RequestReviewPopover branch={branch} pullRequest={pullRequest} readiness={readiness} trigger={button} align="start" />
}

/**
 * The review of a repository only on Space: approved in the merge request, or the groups that still have to approve;
 * a merge request found open counts and opens, none is created (deprecated)
 */
function SpaceReviewBlock({ branchName, review, requirement }: { branchName: string; review: ReviewRequirement; requirement: PatronusReviewRequirement }) {
  const mergeRequest = requirement.mergeRequest
  if (!requirement.required) {
    return (
      <p className="flex items-center gap-1.5 text-sm text-muted-foreground" data-testid="patronus-review" data-status="approved">
        <CircleCheckIcon className="size-4 shrink-0 text-success" />
        {mergeRequest === null ? "No code-owner review needed." : `Approved in ${mergeRequest.reviewKey}.`}
      </p>
    )
  }
  return (
    <Callout
      tone="warning"
      title={mergeRequest === null ? "Needs review by the code owners" : `Waiting for approval in ${mergeRequest.reviewKey}`}
      data-testid="patronus-review"
      data-status="required"
    >
      <ReviewerGroups branchName={branchName} requirement={requirement} />
      <p>A Safe Push fails without these approvals.</p>
      {mergeRequest === null ? (
        // say which system: the approvals count in the GitHub pull request, which can't be opened without its repository
        <p data-testid="patronus-review-github-unknown">
          {"Patronus counts code-owner approvals in the GitHub pull request of the branch. The GitHub repository of the branches is unknown: run "}
          <code className="rounded-sm bg-muted px-1 font-mono text-xs">{"git config branchdeck.githubRepository <owner>/<name>"}</code>.
        </p>
      ) : (
        <p>{`Patronus counts the approvals of ${mergeRequest.reviewKey}.`}</p>
      )}
      <div className="flex flex-wrap gap-2">
        {mergeRequest !== null && (
          <Button variant="outline" size="xs" onClick={() => openMergeRequest(mergeRequest.url)}>
            <ExternalLinkIcon data-icon="inline-start" />
            {`Open ${mergeRequest.reviewKey}`}
          </Button>
        )}
        <ReviewChangesButton branchName={branchName} requirement={requirement} />
        <RecheckButton review={review} />
      </div>
    </Callout>
  )
}

/** Asks Patronus again instead of the kept answer, after the reviews of the pull request from GitHub */
function RecheckButton({ review }: { review: ReviewRequirement }) {
  return (
    <Button variant="ghost" size="xs" data-testid="patronus-review-recheck" onClick={review.recheck}>
      <RefreshCwIcon data-icon="inline-start" />
      Recheck
    </Button>
  )
}
