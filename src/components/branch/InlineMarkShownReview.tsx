import { useState } from "react"
import { UserPlusIcon } from "lucide-react"
import { Spinner } from "@/components/ui/spinner"
import { InlineForm } from "@/components/shared/InlineForm"
import { ReviewerPicker } from "@/components/shared/ReviewerPicker"
import { useOperationWords } from "@/hooks/useOperationWords"
import { useReviewerSearch } from "@/hooks/useReviewerSearch"
import { useShowReviewers } from "@/hooks/useShowReviewers"
import type { ActionId, GitHubUser } from "@/lib/bindings"
import { markShownReview } from "@/lib/shown-review"
import { showToast } from "@/lib/toast-message"
import { useRowPart } from "@/model/store"
import { actionBlocker } from "@/presenters/actions"
import { describeMarkShownReview, describeShowCandidates, formatShownMarkedTitle } from "@/presenters/shown"
import { closeInline, useInlineProcessing, withRowProgress } from "@/stores/inline-row-action"
import { useShownReview } from "@/stores/pull-requests"
import { getSelectedRepositoryPath, useSelectedProjectPath } from "@/stores/repository"

/** The action whose words the form says */
const MARK_SHOWN_REVIEW: readonly ActionId[] = ["mark-shown-review"]

/** The logins `users` as the picker shows them */
function usersOf(logins: readonly string[]): GitHubUser[] {
  return logins.map(login => ({ login, name: null }))
}

/**
 * Request Review After Landing… under the row of a branch (`Intent::MarkShownReview`): the picker of whom its review
 * after landing asks, starting with the reviewers its draft records, else with the code owners of what the branch
 * changes (`get_show_reviewers`), as Safe Push and Request Review… starts; its submit marks the branch ahead of the
 * landing (a draft that asks nobody yet), records the reviewers of its draft again, or opens the review of a branch that
 * landed, as Rust plans it. Of a branch that left the table, under its row of the Landed island: the actions of
 * `actionsKey` (`landed:<name>`)
 */
export function InlineMarkShownReview({ branchName, actionsKey = branchName }: { branchName: string; actionsKey?: string }) {
  const review = useShownReview(branchName)
  const draft = review?.pullRequest.isDraft === true ? review : undefined
  const actions = useRowPart(actionsKey, "actions")
  const words = useOperationWords(actions, MARK_SHOWN_REVIEW)
  const blocker = actionBlocker(actions, "mark-shown-review", { ...words, subject: branchName }) ?? null
  const processing = useInlineProcessing(branchName) !== null
  const search = useReviewerSearch(useSelectedProjectPath() ?? "")
  // the code owners only where no draft records whom it asks
  const candidates = useShowReviewers(branchName, draft === undefined)
  const [picked, setPicked] = useState<GitHubUser[] | null>(null)
  const start = draft === undefined ? (candidates?.kind === "loaded" ? usersOf(candidates.value.reviewers) : []) : usersOf(draft.reviewers)
  const reviewers = picked ?? start
  const logins = reviewers.map(user => user.login)
  const loading = candidates?.kind === "loading"
  const note = candidates === null ? null : describeShowCandidates(candidates)
  const { title, description } = describeMarkShownReview(branchName, draft)
  return (
    <InlineForm
      title={title}
      description={description}
      dataTestId="mark-shown-review"
      canSubmit={!processing && !loading && blocker === null && logins.length > 0}
      submitDisabledReason={blocker ?? (logins.length === 0 ? "Add a reviewer: a review after landing asks at least one" : null)}
      loading={processing}
      submitText="Request Review After Landing"
      submitIcon={<UserPlusIcon data-icon="inline-start" />}
      onCancel={closeInline}
      onSubmit={() => {
        if (logins.length === 0) {
          return
        }
        // the repository the user asked in: a switch meanwhile marks nothing there
        const repositoryPath = getSelectedRepositoryPath()
        const mark = () =>
          markShownReview(repositoryPath, branchName, logins).then(marked => {
            if (marked) {
              showToast(formatShownMarkedTitle(branchName, logins))
            }
            return marked
          })
        void withRowProgress(branchName, mark, { keepFormOnFailure: true })
      }}
    >
      <ReviewerPicker
        testIdPrefix="mark-shown-review"
        reviewers={reviewers}
        setReviewers={setPicked}
        search={search}
        viewerLogin={review?.pullRequest.author ?? null}
        disabled={processing || loading}
        placeholder="Add a reviewer by login or name"
      />
      {loading ? (
        <p className="flex items-center gap-2 text-muted-foreground" data-testid="mark-shown-review-candidates" data-kind="loading">
          <Spinner className="size-4" />
          Reading the code owners of the changes…
        </p>
      ) : (
        note !== null && (
          <p className={candidates?.kind === "failed" ? "text-warning" : "text-muted-foreground"} data-testid="mark-shown-review-candidates">
            {note}
          </p>
        )
      )}
    </InlineForm>
  )
}
