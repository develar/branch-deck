import { GitPullRequestClosedIcon } from "lucide-react"
import { InlineForm } from "@/components/shared/InlineForm"
import { useOperationWords } from "@/hooks/useOperationWords"
import type { ActionId } from "@/lib/bindings"
import { openPullRequest } from "@/lib/pull-request"
import { closeShownReview } from "@/lib/shown-review"
import { showToast } from "@/lib/toast-message"
import { useRowPart } from "@/model/store"
import { actionBlocker } from "@/presenters/actions"
import { describeCancelShownReview, describeCloseShownReview, formatShownCanceledTitle, formatShownClosedTitle } from "@/presenters/shown"
import { closeInline, useInlineProcessing, withRowProgress } from "@/stores/inline-row-action"
import { useShownReview } from "@/stores/pull-requests"
import { getSelectedRepositoryPath } from "@/stores/repository"

/** The actions whose words the form says: the close of an open review, the cancel of a mark */
const CLOSE_SHOWN_REVIEW: readonly ActionId[] = ["close-shown-review"]
const CANCEL_SHOWN_REVIEW: readonly ActionId[] = ["cancel-shown-review"]

/**
 * Close Review… under the row of a branch with a review after landing: what the review waits for (approved, or the
 * approvals and the fix it closes without), then Close Review, which closes it with its comment and deletes its branches,
 * approved or not (as `brd close-review --allow-unapproved`); as Rust offers the close (a close that runs holds it). Of a
 * draft marked ahead of the landing (`cancel`, Cancel Review After Landing…) the same close: nothing opens once the
 * branch lands, its comment says it was canceled
 */
export function InlineCloseShownReview({ branchName, cancel = false }: { branchName: string; cancel?: boolean }) {
  const review = useShownReview(branchName)
  const actions = useRowPart(branchName, "actions")
  const id: ActionId = cancel ? "cancel-shown-review" : "close-shown-review"
  // the operations of the session only while the close runs, waits or is busy behind one, whose words it says
  const words = useOperationWords(actions, cancel ? CANCEL_SHOWN_REVIEW : CLOSE_SHOWN_REVIEW)
  const blocker = actionBlocker(actions, id, { ...words, subject: branchName }) ?? null
  const processing = useInlineProcessing(branchName) !== null
  if (review === undefined) {
    // closed meanwhile (every reviewer approved, another window): nothing is left to confirm
    return (
      <InlineForm title="Close review after landing" description={`${branchName} has no open review after landing.`} canSubmit={false} onCancel={closeInline}>
        {null}
      </InlineForm>
    )
  }
  const { title, description } = cancel ? describeCancelShownReview(review, branchName) : describeCloseShownReview(review, branchName)
  const { pullRequest } = review
  return (
    <InlineForm
      title={title}
      description={description}
      dataTestId={cancel ? "cancel-shown-review" : "close-shown-review"}
      canSubmit={!processing && blocker === null}
      submitDisabledReason={blocker}
      loading={processing}
      submitText={cancel ? `Close Draft #${pullRequest.number}` : "Close Review"}
      submitIcon={<GitPullRequestClosedIcon data-icon="inline-start" />}
      submitVariant="destructive"
      focusSubmit
      onCancel={closeInline}
      onSubmit={() => {
        // the repository the user asked in: a switch meanwhile closes nothing there
        const repositoryPath = getSelectedRepositoryPath()
        const close = () =>
          closeShownReview(repositoryPath, branchName).then(closed => {
            if (closed) {
              showToast({
                title: cancel ? formatShownCanceledTitle(pullRequest.number, branchName) : formatShownClosedTitle(pullRequest.number, branchName),
                description: pullRequest.title,
                action: { children: "Open Pull Request", onClick: () => openPullRequest(pullRequest) },
              })
            }
            return closed
          })
        void withRowProgress(branchName, close, { keepFormOnFailure: true })
      }}
    >
      {null}
    </InlineForm>
  )
}
