import type { VcsRequestFactory } from "./vcsRequest"
import type { ReactiveBranch } from "~/composables/branchSyncProvider"
import { commands } from "~/utils/bindings"
// notifyError is auto-imported from shared-ui layer

export function usePush(vcsRequestFactory: VcsRequestFactory, branches: Ref<ReactiveBranch[]>) {
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
      const result = await commands.pushBranch({ repositoryPath: request.repositoryPath, branchPrefix: request.branchPrefix, branchName })
      if (result.status === "ok") {
        toast.add({
          title: "Success",
          description: result.data,
          color: "success",
        })
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
