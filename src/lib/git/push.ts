import { toast } from "@/components/ui/toast"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { showStackedPushToast } from "@/lib/git/stacked-push-toast"
import { stackPushOrder } from "@/lib/stack"
import { branchSyncStore, getBranch, isBranchPushing, setBranchPushing, setBranchRemoteStatus, toRemoteStatus } from "@/stores/branch-sync"
import { refreshPullRequestsAfterPush } from "@/stores/pull-requests"
import { vcsRequestFactory } from "@/stores/repository"

/**
 * Pushes a virtual branch and updates its remote status from the result, then the pull requests (the pull request of
 * the branch now shows the pushed commit). Errors are reported with a toast. Resolves to whether the push succeeded.
 */
export async function pushBranch(branchName: string): Promise<boolean> {
  if (isBranchPushing(branchName)) {
    return false
  }

  const branch = getBranch(branchName)
  if (branch === undefined) {
    notifyError("Push Failed", `Branch ${branchName} not found`)
    return false
  }

  setBranchPushing(branchName, true)
  try {
    const request = vcsRequestFactory.createRequest()
    const result = await commands.pushBranch({
      repositoryPath: request.repositoryPath,
      branchPrefix: request.branchPrefix,
      branchName,
      myEmail: branch.myEmail,
      baselineBranch: branchSyncStore.getState().baselineBranch!,
      // a stacked branch's own commits are those after its parent
      parentBranch: branch.stack?.effectiveBase ?? null,
    })

    if (result.status === "ok") {
      if (branch.stack === null) {
        toast.add({
          type: "success",
          title: "Success",
          description: `Branch ${branchName} pushed successfully`,
        })
      }
      else {
        showStackedPushToast(branch)
      }
      setBranchRemoteStatus(branchName, toRemoteStatus(result.data))
      void refreshPullRequestsAfterPush(branchName, branch.commits[0]?.hash)
      return true
    }
    notifyError("Push Failed", result.error)
  }
  catch (error) {
    notifyError("Push Failed", error)
  }
  finally {
    setBranchPushing(branchName, false)
  }
  return false
}

/** Pushes the ancestors of a stacked branch that have work for the remote, root first, then the branch; stops at a failure */
export async function pushStack(branchName: string): Promise<void> {
  for (const name of stackPushOrder(branchName, getBranch)) {
    if (!await pushBranch(name)) {
      return
    }
  }
}
