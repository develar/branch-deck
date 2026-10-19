import { GitPullRequestClosedIcon } from "lucide-react"
import { InlineForm } from "@/components/shared/InlineForm"
import { notifyError } from "@/lib/errors"
import { openPullRequest } from "@/lib/pull-request"
import { showToast } from "@/lib/toast-message"
import { describeOrphanedStackPullRequest } from "@/lib/whole-stack"
import { recordPullRequestClosed } from "@/stores/activity"
import { useStackBranch } from "@/stores/branch-sync"
import { closeInline, inlineSurface, useInlineProcessing, withRowProgress } from "@/stores/inline-row-action"
import { useStackLandedBy } from "@/stores/landed-stacks"
import { runOperation } from "@/stores/operations"
import { closeStackPullRequest, useStackPullRequest } from "@/stores/pull-requests"
import { getFullStackBranchName } from "@/stores/repository"

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
        const close = () =>
          closeStackPullRequest(root, `Closed in Branch Deck: ${reason}.`).then(
            () => {
              const title = `Closed pull request #${pullRequest.number}`
              showToast({ title, description: reason, action: { children: "Open Pull Request", onClick: () => openPullRequest(pullRequest) } })
              recordPullRequestClosed(title, reason, pullRequest.url)
              return true
            },
            (error: unknown) => {
              notifyError(`Cannot close pull request #${pullRequest.number}`, error)
              return false
            },
          )
        const operation = {
          id: "close-stack-pull-request",
          kind: "remote",
          subject: `pr:${pullRequest.number}`,
          objects: [root],
          label: `Closing pull request #${pullRequest.number}`,
          surface: inlineSurface(root),
        } as const
        void withRowProgress(root, () => runOperation(operation, close).then(done => done === true), { keepFormOnFailure: true })
      }}
    >
      {null}
    </InlineForm>
  )
}
