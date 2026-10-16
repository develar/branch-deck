import { GitPullRequestCreateIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { BranchPushButton } from "@/components/branch/BranchPushButton"
import { SafePushButton } from "@/components/branch/SafePushButton"
import { getNewPullRequestUrl, openNewPullRequest } from "@/lib/git/new-pull-request"
import { openPullRequest } from "@/lib/pull-request"
import type { Branch } from "@/stores/branch-sync"
import { usePatronusAvailable } from "@/stores/patronus"
import { usePullRequest } from "@/stores/pull-requests"

/**
 * The actions on a branch in the footer of its preview and of its review in the dialog, as its menu offers them
 * (`getBranchReviewItems`): O Open Pull Request (or Create Pull Request of a pushed branch on GitHub), S Safe Push…
 * where Patronus is and the branch has work, P Push for a branch with work; in the review Push has no key, P is the
 * previous change there. The Safe Push form is a form of the row: the dialog closes for it (`ClosePreviewContext`).
 */
export function BranchPreviewActions({ branch, review }: { branch: Branch; review: boolean }) {
  const pullRequest = usePullRequest(branch.name)
  const patronusAvailable = usePatronusAvailable()
  // as the branch menu offers it: a pushed branch without a live pull request, on GitHub
  const newPullRequestUrl = getNewPullRequestUrl(branch)
  // as the push button of the row: only a branch that synced without an error has something to push
  const hasPush = !branch.hasError && branch.commitCount > 0
  return (
    <>
      {newPullRequestUrl !== null ? (
        <Button variant="ghost" size="xs" onClick={() => openNewPullRequest(newPullRequestUrl)}>
          <GitPullRequestCreateIcon data-icon="inline-start" />
          Create Pull Request
        </Button>
      ) : (
        pullRequest !== undefined && (
          <Button variant="ghost" size="xs" onClick={() => openPullRequest(pullRequest)}>
            <Kbd>O</Kbd>
            Open Pull Request
          </Button>
        )
      )}
      {patronusAvailable && hasPush && <SafePushButton branch={branch} hint={<Kbd>S</Kbd>} />}
      {hasPush && <BranchPushButton branch={branch} colored={false} hint={review ? undefined : <Kbd>P</Kbd>} confirmTarget="preview" />}
    </>
  )
}
