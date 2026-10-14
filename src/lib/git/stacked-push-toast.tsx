import { toast } from "@/components/ui/toast"
import { notifyError } from "@/lib/errors"
import { pullRequestBaseOf } from "@/lib/stack"
import type { Branch } from "@/stores/branch-sync"
import { branchSyncStore } from "@/stores/branch-sync"
import { getPullRequest } from "@/stores/pull-requests"
import { getFullBranchName } from "@/stores/repository"

export function copyText(text: string): void {
  navigator.clipboard.writeText(text).catch((error: unknown) => notifyError("Copy Failed", error))
}

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
 * A stacked branch's pull request targets its parent: the toast names the base and copies the `gh` command that opens
 * the pull request, or that retargets an open one with another base
 */
export function showStackedPushToast(branch: Branch): void {
  const base = getPullRequestBase(branch)
  const head = getFullBranchName(branch.name)
  const pullRequest = getPullRequest(branch.name)
  const retargetCommand = getRetargetCommand(branch)
  const retarget = retargetCommand !== null
  const command = retargetCommand ?? `gh pr create --base ${base} --head ${head}`
  // the short name fits the toast; the copied command has the full branch name
  const baseName = <span className="font-medium">{branch.stack?.effectiveBase ?? base}</span>
  toast.add({
    type: "success",
    title: `Pushed ${branch.name}`,
    description: retarget
      ? (
          <>
            {"The pull request targets another base: retarget it to "}
            {baseName}
          </>
        )
      : (
          <>
            {"Pull request base: "}
            {baseName}
          </>
        ),
    timeout: 10_000,
    ...((pullRequest === undefined || retarget) && {
      actionProps: { children: retarget ? "Copy gh pr edit" : "Copy gh pr create", onClick: () => copyText(command) },
    }),
  })
}
