import type { VcsRequestFactory } from "./vcsRequest"
import type { ReactiveBranch } from "~/composables/branchSyncProvider"
import { commands } from "~/utils/bindings"
// notifyError is auto-imported from shared-ui layer

export function usePush(vcsRequestFactory: VcsRequestFactory, branches: Ref<ReactiveBranch[]>, baselineBranch: Ref<string | null>) {
  const toast = useToast()

  const findBranch = (branchName: string): ReactiveBranch | undefined => {
    return branches.value.find(branch => branch.name === branchName)
  }

  const pushBranch = async (branchName: string) => {
    const branch = findBranch(branchName)
    if (branch) {
      branch.isPushing = true
    }

    try {
      const request = vcsRequestFactory.createRequest()

      if (!branch) {
        const error = `Branch ${branchName} not found`
        notifyError("Push Failed", error, toast)
        return { status: "error", error }
      }

      const result = await commands.pushBranch({
        repositoryPath: request.repositoryPath,
        branchPrefix: request.branchPrefix,
        branchName,
        totalCommits: branch.commitCount,
        myEmail: branch.myEmail,
        baselineBranch: baselineBranch.value!,
      })

      if (result.status === "ok") {
        toast.add({
          title: "Success",
          description: `Branch ${branchName} pushed successfully`,
          color: "success",
        })

        // Update remote status from push result
        const remoteStatus = result.data
        branch.remoteStatus = {
          exists: remoteStatus.remoteExists,
          unpushedCommits: remoteStatus.unpushedCommits,
          commitsAhead: remoteStatus.unpushedCommits.length,
          commitsBehind: remoteStatus.commitsBehind,
          myCommitsAhead: remoteStatus.myUnpushedCount ?? 0,
          lastPushTime: remoteStatus.lastPushTime ?? 0,
        }
      }
      else {
        notifyError("Push Failed", result.error, toast)
      }
      return result
    }
    catch (error) {
      notifyError("Push Failed", error, toast)
      return { status: "error", error: String(error) }
    }
    finally {
      if (branch) {
        branch.isPushing = false
      }
    }
  }

  return { pushBranch }
}
