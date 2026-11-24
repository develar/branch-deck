import { useState } from "react"
import { UserPlusIcon } from "lucide-react"
import { InlineForm } from "@/components/shared/InlineForm"
import { ReviewerPicker } from "@/components/shared/ReviewerPicker"
import { useOperationWords } from "@/hooks/useOperationWords"
import { useReviewerSearch } from "@/hooks/useReviewerSearch"
import type { ActionId, GitHubUser } from "@/lib/bindings"
import { isLogin } from "@/lib/create-pull-request"
import { openPullRequest } from "@/lib/pull-request"
import { requestShownReview } from "@/lib/shown-review"
import { joinNames } from "@/lib/text"
import { showToast } from "@/lib/toast-message"
import { useRowPart } from "@/model/store"
import { actionBlocker } from "@/presenters/actions"
import { formatShownRequestedTitle } from "@/presenters/shown"
import { closeInline, useInlineProcessing, withRowProgress } from "@/stores/inline-row-action"
import { useShownReview } from "@/stores/pull-requests"
import { getSelectedRepositoryPath, useSelectedProjectPath } from "@/stores/repository"

/** The action whose words the form says */
const REQUEST_SHOWN_REVIEW: readonly ActionId[] = ["request-shown-review"]

/** Nobody picked yet */
const NOBODY: GitHubUser[] = []

/**
 * Request Review… under the row of a branch with a review after landing: whom it asks now, and the search of the users
 * of the repository for more; Request Review asks them and records them in its body (of a draft only the record: its
 * open asks them once the Safe Push merged), as Rust offers it
 */
export function InlineShownReviewers({ branchName }: { branchName: string }) {
  const review = useShownReview(branchName)
  const actions = useRowPart(branchName, "actions")
  const words = useOperationWords(actions, REQUEST_SHOWN_REVIEW)
  const blocker = actionBlocker(actions, "request-shown-review", { ...words, subject: branchName }) ?? null
  const processing = useInlineProcessing(branchName) !== null
  const search = useReviewerSearch(useSelectedProjectPath() ?? "")
  const [added, setAdded] = useState<GitHubUser[]>(NOBODY)
  const asked = review?.reviewers ?? []
  // those it records already are asked again by the open, never added twice
  const logins = added.map(user => user.login).filter(login => !asked.some(it => isLogin(it, login)))
  const number = review?.pullRequest.number
  return (
    <InlineForm
      title={number === undefined ? "Request review after landing" : `Request review after landing on #${number}`}
      description={
        review === undefined
          ? `${branchName} has no open review after landing.`
          : asked.length === 0
            ? "It asks nobody yet: the reviewers you add are asked and recorded in its body."
            : `It asks ${joinNames(asked)}: the reviewers you add are asked too and recorded in its body.`
      }
      dataTestId="shown-reviewers"
      canSubmit={review !== undefined && !processing && blocker === null && logins.length > 0}
      submitDisabledReason={blocker ?? (logins.length === 0 ? "Add a reviewer" : null)}
      loading={processing}
      submitText="Request Review"
      submitIcon={<UserPlusIcon data-icon="inline-start" />}
      onCancel={closeInline}
      onSubmit={() => {
        if (review === undefined || logins.length === 0) {
          return
        }
        // the repository the user asked in: a switch meanwhile asks nobody there
        const repositoryPath = getSelectedRepositoryPath()
        const { pullRequest } = review
        const request = () =>
          requestShownReview(repositoryPath, branchName, logins).then(requested => {
            if (requested) {
              showToast({
                title: formatShownRequestedTitle(pullRequest.number, logins, pullRequest.isDraft),
                description: pullRequest.title,
                action: { children: "Open Pull Request", onClick: () => openPullRequest(pullRequest) },
              })
            }
            return requested
          })
        void withRowProgress(branchName, request, { keepFormOnFailure: true })
      }}
    >
      <ReviewerPicker
        testIdPrefix="shown-reviewers"
        reviewers={added}
        setReviewers={setAdded}
        search={search}
        viewerLogin={review?.pullRequest.author ?? null}
        disabled={processing}
      />
    </InlineForm>
  )
}
