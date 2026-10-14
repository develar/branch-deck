import { toast } from "@/components/ui/toast"
import { copyText } from "@/lib/clipboard"
import type { PushNews } from "@/lib/push-summary"
import { pullRequestBaseOf } from "@/lib/stack"
import { ACTION_TOAST_TIMEOUT_MS } from "@/lib/toast-message"
import type { Branch } from "@/stores/branch-sync"
import { branchSyncStore } from "@/stores/branch-sync"
import { getPullRequest } from "@/stores/pull-requests"
import { getFullBranchName } from "@/stores/repository"

/** The pull request base of a branch, as `gh` takes it */
export function getPullRequestBase(branch: Branch): string {
  return pullRequestBaseOf(branch, branchSyncStore.getState().baselineBranch ?? "", getFullBranchName)
}

/**
 * The `gh` command that retargets the open pull request of a branch whose base is not the one it should have, e.g.
 * after its parent was integrated or the branch was restacked; null if there is nothing to retarget
 */
export function getRetargetCommand(branch: Branch): string | null {
  const pullRequest = getPullRequest(branch.name)
  if (pullRequest === undefined || pullRequest.state !== "open" || !pullRequest.baseRefName) {
    return null
  }
  const base = getPullRequestBase(branch)
  return pullRequest.baseRefName === base ? null : `gh pr edit ${pullRequest.number} --base ${base}`
}

/**
 * A stacked branch's pull request targets its parent: the toast says what the push did (`news`, as Activity says it),
 * names the base and copies the `gh` command that opens the pull request, or that retargets an open one with another
 * base
 */
export function showStackedPushToast(branch: Branch, news: PushNews): void {
  const base = getPullRequestBase(branch)
  const head = getFullBranchName(branch.name)
  const pullRequest = getPullRequest(branch.name)
  const retargetCommand = getRetargetCommand(branch)
  const retarget = retargetCommand !== null
  const command = retargetCommand ?? `gh pr create --base ${base} --head ${head}`
  // the short name fits the toast; the copied command has the full branch name
  const baseName = <span className="font-medium">{branch.stack?.effectiveBase ?? base}</span>
  const lead = retarget ? "the pull request targets another base: retarget it to " : "pull request base: "
  // after what the push did ("1 commit · pull request base: login"), or as the sentence
  const text = news.description === undefined ? lead.charAt(0).toUpperCase() + lead.slice(1) : `${news.description} · ${lead}`
  toast.add({
    type: "success",
    title: news.title,
    description: (
      <>
        {text}
        {baseName}
      </>
    ),
    timeout: ACTION_TOAST_TIMEOUT_MS,
    ...((pullRequest === undefined || retarget) && {
      actionProps: { children: retarget ? "Copy gh pr edit" : "Copy gh pr create", onClick: () => void copyText(command) },
    }),
  })
}
