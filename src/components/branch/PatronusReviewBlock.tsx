import { CircleCheckIcon, ExternalLinkIcon, GitPullRequestCreateIcon, RefreshCwIcon, TriangleAlertIcon } from "lucide-react"
import { openUrl } from "@tauri-apps/plugin-opener"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
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
      <p className="flex items-center gap-2 text-sm text-muted-foreground" data-testid="patronus-review" data-state="loading">
        <Spinner className="size-3.5" />
        Checking the code-owner review…
      </p>
    )
  }
  if (state.status === "error") {
    return <p className="text-sm text-muted-foreground" data-testid="patronus-review" data-state="error">{`Review check: ${state.message}`}</p>
  }

  const { requirement } = state
  const mergeRequest = requirement.mergeRequest
  if (requirement.skipped) {
    return <p className="text-sm text-muted-foreground" data-testid="patronus-review" data-state="skipped">Too many commits to check the review before the start; the Safe Push checks it.</p>
  }
  if (!requirement.required) {
    return (
      <p className="flex items-center gap-1.5 text-sm text-muted-foreground" data-testid="patronus-review" data-state="approved">
        <CircleCheckIcon className="size-4 shrink-0 text-success" />
        {mergeRequest === null ? "No code-owner review needed." : `Approved in ${mergeRequest.reviewKey}.`}
      </p>
    )
  }

  return (
    <div className="flex items-start gap-3 rounded-lg border border-warning/20 bg-warning/10 p-3" data-testid="patronus-review" data-state="required">
      <TriangleAlertIcon className="mt-0.5 size-4 shrink-0 text-warning" />
      <div className="flex min-w-0 flex-col gap-2 text-sm">
        <p className="font-medium text-warning">
          {mergeRequest === null ? "Needs review by the code owners" : `Waiting for approval in ${mergeRequest.reviewKey}`}
        </p>
        <ul className="flex flex-col gap-0.5" data-testid="patronus-review-groups">
          {requirement.groups.map(group => <li key={group.name}>{formatReviewerGroup(group)}</li>)}
        </ul>
        <p className="text-muted-foreground">A Safe Push fails without these approvals.</p>
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
      </div>
    </div>
  )
}
