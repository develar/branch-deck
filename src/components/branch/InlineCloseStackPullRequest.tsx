import { GitPullRequestClosedIcon } from "lucide-react"
import { InlineForm } from "@/components/shared/InlineForm"
import { notifyError } from "@/lib/errors"
import { openPullRequest } from "@/lib/pull-request"
import { closePullRequest } from "@/lib/pull-request-run"
import { showToast } from "@/lib/toast-message"
import { describeOrphanedStackPullRequest } from "@/lib/whole-stack"
import { useStackBranch } from "@/stores/branch-sync"
import { closeInline, useInlineProcessing, withRowProgress } from "@/stores/inline-row-action"
import { useStackLandedBy } from "@/stores/landed-stacks"
import { useStackPullRequest } from "@/stores/pull-requests"
import { getFullStackBranchName, getSelectedProject } from "@/stores/repository"

/**
 * Asks before closing the pull request of the whole stack of `root` that no longer matches the stack
 * (`describeOrphanedStackPullRequest`): Branch Deck never closes it by itself. Closes it on GitHub with a comment that
 * says why; its stack branch stays, on the remote too.
 */
export function InlineCloseStackPullRequest({ root }: { root: string }) {
  const pullRequest = useStackPullRequest(root)
  const reason = describeOrphanedStackPullRequest(root, pullRequest, useStackBranch(root), useStackLandedBy(root))
  const processing = useInlineProcessing(root) !== null
  const number = pullRequest?.number
  return (
    <InlineForm
      title={number === undefined ? "Close pull request" : `Close pull request #${number}`}
      description={`${reason ?? "The pull request matches the stack"}. Closes it on GitHub with a comment; its stack branch ${getFullStackBranchName(root)} stays.`}
      canSubmit={reason !== null && pullRequest !== undefined && !processing}
      submitDisabledReason={reason === null ? "The pull request matches the stack again" : null}
      loading={processing}
      submitText="Close Pull Request"
      submitIcon={<GitPullRequestClosedIcon data-icon="inline-start" />}
      submitVariant="destructive"
      focusSubmit
      onCancel={closeInline}
      onSubmit={() => {
        if (reason === null || pullRequest === undefined) {
          return
        }
        // the repository the user asked in: a switch meanwhile closes nothing there (`ClosePullRequest` holds its state)
        const repositoryPath = getSelectedProject()?.path ?? ""
        const close = () =>
          closePullRequest(repositoryPath, pullRequest.number, `Closed in Branch Deck: ${reason}.`).then(
            () => {
              const title = `Closed pull request #${pullRequest.number}`
              showToast({ title, description: reason, action: { children: "Open Pull Request", onClick: () => openPullRequest(pullRequest) } })
              return true
            },
            (error: unknown) => {
              notifyError(`Cannot close pull request #${pullRequest.number}`, error)
              return false
            },
          )
        void withRowProgress(root, close, { keepFormOnFailure: true })
      }}
    >
      {null}
    </InlineForm>
  )
}
