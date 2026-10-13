import { toast } from "@/components/ui/toast"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { branchSyncStore, getBranch, isBranchPushing, setBranchPushing, setBranchRemoteStatus, toRemoteStatus } from "@/stores/branch-sync"
import { vcsRequestFactory } from "@/stores/repository"

/** Pushes a virtual branch and updates its remote status from the result. Errors are reported with a toast. */
export async function pushBranch(branchName: string): Promise<void> {
  if (isBranchPushing(branchName)) {
    return
  }

  const branch = getBranch(branchName)
  if (branch === undefined) {
    notifyError("Push Failed", `Branch ${branchName} not found`)
    return
  }

  setBranchPushing(branchName, true)
  try {
    const request = vcsRequestFactory.createRequest()
    const result = await commands.pushBranch({
      repositoryPath: request.repositoryPath,
      branchPrefix: request.branchPrefix,
      branchName,
      totalCommits: branch.commitCount,
      myEmail: branch.myEmail,
      baselineBranch: branchSyncStore.getState().baselineBranch!,
    })

    if (result.status === "ok") {
      toast.add({
        type: "success",
        title: "Success",
        description: `Branch ${branchName} pushed successfully`,
      })
      setBranchRemoteStatus(branchName, toRemoteStatus(result.data))
    }
    else {
      notifyError("Push Failed", result.error)
    }
  }
  catch (error) {
    notifyError("Push Failed", error)
  }
  finally {
    setBranchPushing(branchName, false)
  }
}
