import { useState } from "react"
import { ChevronRightIcon, CircleCheckIcon, ExternalLinkIcon, FileDiffIcon, GitPullRequestCreateIcon, RefreshCwIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible"
import { Spinner } from "@/components/ui/spinner"
import { Callout } from "@/components/shared/Callout"
import type { ReviewRequirement } from "@/hooks/usePatronusReviewRequirement"
import type { PatronusReviewerGroup, PatronusReviewRequirement, PullRequest } from "@/lib/bindings"
import { openBranchReview, toReviewGroups } from "@/lib/branch-review-window"
import { formatReviewerGroup, formatReviewerGroupFiles } from "@/lib/patronus"
import { openMergeRequest } from "@/lib/patronus-merge-request"
import { openPullRequest } from "@/lib/pull-request"

export interface PatronusReviewBlockProps {
  /** The branch of the Safe Push: its review window shows the files of a group */
  branchName: string
  review: ReviewRequirement
  /**
   * The GitHub repository of the branches is known (`RepositoryPatronus.githubRepository`), with the pull request of
   * the branch if it has one: Patronus counts the approvals of the pull request. null = unknown: no pull request can be
   * opened, as Space merge requests are deprecated.
   */
  github: { pullRequest: PullRequest | undefined } | null
  /** Pushes the branch if needed and opens the GitHub form of its pull request */
  onCreatePullRequest: () => Promise<void>
}

/**
 * The code-owner review of a Safe Push: nothing needed; approved in the merge request; or the groups that still have to
 * approve, with the GitHub pull request Patronus counts the approvals in, to open or to create, and Recheck. A Space
 * merge request is never created (deprecated); one found open still counts and opens. A Safe Push without the
 * approvals fails, but the start stays possible: approvals may arrive while it runs up to its review check.
 */
export function PatronusReviewBlock({ branchName, review, github, onCreatePullRequest }: PatronusReviewBlockProps) {
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

  const { requirement } = state
  const mergeRequest = requirement.mergeRequest
  if (requirement.skipped) {
    return <p className="text-sm text-muted-foreground" data-testid="patronus-review" data-status="skipped">Too many commits to check the review before the start; the Safe Push checks it.</p>
  }
  if (!requirement.required) {
    return (
      <p className="flex items-center gap-1.5 text-sm text-muted-foreground" data-testid="patronus-review" data-status="approved">
        <CircleCheckIcon className="size-4 shrink-0 text-success" />
        {mergeRequest === null ? "No code-owner review needed." : `Approved in ${mergeRequest.reviewKey}.`}
      </p>
    )
  }

  if (github !== null) {
    return <GitHubReviewCallout branchName={branchName} review={review} pullRequest={github.pullRequest} onCreatePullRequest={onCreatePullRequest} />
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
      {mergeRequest === null
        ? (
            // say which system: the approvals count in the GitHub pull request, which can't be opened without its repository
            <p data-testid="patronus-review-github-unknown">
              {"Patronus counts code-owner approvals in the GitHub pull request of the branch. The GitHub repository of the branches is unknown: run "}
              <code className="rounded-sm bg-muted px-1 font-mono text-xs">{"git config branchdeck.githubRepository <owner>/<name>"}</code>
              .
            </p>
          )
        : <p>{`Patronus counts the approvals of ${mergeRequest.reviewKey}.`}</p>}
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

/**
 * The groups that still have to approve, each with the files it reviews (a group Patronus names without them and that
 * owns no file by its name in the `OWNERSHIP` files is only named); a file opens the review window at it
 */
function ReviewerGroups({ branchName, requirement }: { branchName: string, requirement: PatronusReviewRequirement }) {
  return (
    <ul className="flex flex-col gap-0.5 text-foreground" data-testid="patronus-review-groups">
      {requirement.groups.map(group => <ReviewerGroupItem key={group.name} branchName={branchName} group={group} requirement={requirement} />)}
    </ul>
  )
}

function ReviewerGroupItem({ branchName, group, requirement }: { branchName: string, group: PatronusReviewerGroup, requirement: PatronusReviewRequirement }) {
  const files = formatReviewerGroupFiles(group)
  // the files of a group, from where Patronus checks them: the whole stack down to the baseline
  const open = (path: string | null) => openBranchReview(branchName, { range: "all", filter: { kind: "group", name: group.name }, path, groups: toReviewGroups(requirement) })
  if (files === null) {
    return <li data-testid="patronus-review-group" data-name={group.name}>{formatReviewerGroup(group)}</li>
  }
  if (group.branchWide) {
    return (
      <li data-testid="patronus-review-group" data-name={group.name}>
        {formatReviewerGroup(group)}
        <button type="button" className="ml-1 text-muted-foreground underline-offset-4 hover:text-foreground hover:underline" onClick={() => open(null)}>{`· ${files}`}</button>
      </li>
    )
  }
  return (
    <li data-testid="patronus-review-group" data-name={group.name}>
      <Collapsible className="flex flex-col gap-0.5">
        <CollapsibleTrigger className="group flex w-fit items-center gap-1 text-left" data-testid="patronus-review-group-toggle">
          <ChevronRightIcon className="size-3.5 shrink-0 text-muted-foreground transition-transform group-data-[panel-open]:rotate-90" />
          <span>{formatReviewerGroup(group)}</span>
          <span className="text-muted-foreground">{`· ${files}`}</span>
        </CollapsibleTrigger>
        <CollapsibleContent>
          <ul className="flex flex-col pl-5 text-xs" data-testid="patronus-review-group-files">
            {group.paths.map(path => (
              <li key={path} className="min-w-0">
                <button type="button" className="max-w-full truncate text-left text-muted-foreground underline-offset-4 hover:text-foreground hover:underline" title={`Review ${path}`} onClick={() => open(path)}>
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
function ReviewChangesButton({ branchName, requirement }: { branchName: string, requirement: PatronusReviewRequirement }) {
  return (
    <Button variant="outline" size="xs" data-testid="patronus-review-changes" onClick={() => openBranchReview(branchName, { range: "all", groups: toReviewGroups(requirement) })}>
      <FileDiffIcon data-icon="inline-start" />
      Review Changes
    </Button>
  )
}

/**
 * The approvals missing on GitHub: the check before the start can't take a pull request (it never sees its approvals),
 * so the approvals the pull request already has are named, and the Safe Push from the pull request counts them
 */
function GitHubReviewCallout({ branchName, review, pullRequest, onCreatePullRequest }: {
  branchName: string
  review: ReviewRequirement
  pullRequest: PullRequest | undefined
  onCreatePullRequest: () => Promise<void>
}) {
  const [isCreating, setCreating] = useState(false)
  const requirement = review.state.status === "loaded" ? review.state.requirement : null
  const open = pullRequest?.state === "open" ? pullRequest : undefined
  const approvers = open?.reviewers.filter(reviewer => reviewer.state === "approved").map(reviewer => reviewer.login) ?? []
  return (
    <Callout tone="warning" title="Needs review by the code owners" data-testid="patronus-review" data-status="required">
      {requirement !== null && <ReviewerGroups branchName={branchName} requirement={requirement} />}
      {approvers.length > 0 && (
        <p data-testid="patronus-review-github-approvals">{`Approved on GitHub by ${approvers.join(", ")}; the Safe Push counts them, the check before it can't.`}</p>
      )}
      <p>A Safe Push fails without these approvals.</p>
      <p>
        {open === undefined
          ? "Patronus counts code-owner approvals in the pull request of the branch; the Safe Push starts from it."
          : `Patronus counts code-owner approvals in pull request #${open.number}.`}
      </p>
      <div className="flex flex-wrap gap-2">
        {open === undefined
          ? (
              <Button
                variant="outline"
                size="xs"
                disabled={isCreating}
                onClick={() => {
                  setCreating(true)
                  void onCreatePullRequest().finally(() => setCreating(false))
                }}
              >
                {isCreating ? <Spinner data-icon="inline-start" /> : <GitPullRequestCreateIcon data-icon="inline-start" />}
                Create Pull Request
              </Button>
            )
          : (
              <Button variant="outline" size="xs" onClick={() => openPullRequest(open)}>
                <ExternalLinkIcon data-icon="inline-start" />
                {`Open Pull Request #${open.number}`}
              </Button>
            )}
        {requirement !== null && <ReviewChangesButton branchName={branchName} requirement={requirement} />}
        <RecheckButton review={review} />
      </div>
    </Callout>
  )
}

/** Asks Patronus again instead of the cached answer: approvals may have arrived */
function RecheckButton({ review }: { review: ReviewRequirement }) {
  return (
    <Button variant="ghost" size="xs" onClick={review.recheck}>
      <RefreshCwIcon data-icon="inline-start" />
      Recheck
    </Button>
  )
}
