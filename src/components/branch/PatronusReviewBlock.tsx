import { CircleCheckIcon, ExternalLinkIcon, GitPullRequestCreateIcon, RefreshCwIcon } from "lucide-react"
import { openUrl } from "@tauri-apps/plugin-opener"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { Callout } from "@/components/shared/Callout"
import type { ReviewRequirement } from "@/hooks/usePatronusReviewRequirement"
import { notifyError } from "@/lib/errors"
import { formatReviewerGroup } from "@/lib/patronus"

function openMergeRequest(url: string): void {
  openUrl(url).catch((error: unknown) => notifyError("Cannot open the merge request", error))
}

export interface PatronusReviewBlockProps {
  review: ReviewRequirement
  /** Pushes the branch if needed and opens its merge request; resolves to its URL, or null when the push failed */
  onCreateMergeRequest: () => Promise<string | null>
}

/**
 * The code-owner review of a Safe Push: nothing needed; approved in the merge request; or the groups that still have to
 * approve, with the merge request to open or to create, and Recheck. A Safe Push without the approvals fails, but the
 * start stays possible: approvals may arrive while it runs up to its review check.
 */
export function PatronusReviewBlock({ review, onCreateMergeRequest }: PatronusReviewBlockProps) {
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

  return (
    <Callout
      tone="warning"
      title={mergeRequest === null ? "Needs review by the code owners" : `Waiting for approval in ${mergeRequest.reviewKey}`}
      data-testid="patronus-review"
      data-status="required"
    >
      <ul className="flex flex-col gap-0.5 text-foreground" data-testid="patronus-review-groups">
        {requirement.groups.map(group => <li key={group.name}>{formatReviewerGroup(group)}</li>)}
      </ul>
      <p>A Safe Push fails without these approvals.</p>
      <div className="flex flex-wrap gap-2">
        {mergeRequest === null
          ? (
              <Button
                variant="outline"
                size="xs"
                disabled={review.isCreatingMergeRequest}
                onClick={() => {
                  void onCreateMergeRequest().then((url) => {
                    if (url !== null) {
                      openMergeRequest(url)
                    }
                  }, (error: unknown) => notifyError("Cannot create the merge request", error))
                }}
              >
                {review.isCreatingMergeRequest ? <Spinner data-icon="inline-start" /> : <GitPullRequestCreateIcon data-icon="inline-start" />}
                Create Merge Request
              </Button>
            )
          : (
              <Button variant="outline" size="xs" onClick={() => openMergeRequest(mergeRequest.url)}>
                <ExternalLinkIcon data-icon="inline-start" />
                {`Open ${mergeRequest.reviewKey}`}
              </Button>
            )}
        <Button variant="ghost" size="xs" onClick={review.recheck}>
          <RefreshCwIcon data-icon="inline-start" />
          Recheck
        </Button>
      </div>
    </Callout>
  )
}
