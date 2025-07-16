import { ref } from "vue"
import type { VcsRequestFactory } from "./vcsRequest"
import { commands } from "~/utils/bindings"
// notifyError is auto-imported from shared-ui layer

export function usePush(vcsRequestFactory: VcsRequestFactory) {
  const pushingBranches = ref(new Set<string>())

  const toast = useToast()

  const pushBranch = async (branchName: string) => {
    pushingBranches.value.add(branchName)
    try {
      const request = vcsRequestFactory.createRequest()
      const result = await commands.pushBranch(request.repositoryPath, request.branchPrefix, branchName)
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
      pushingBranches.value.delete(branchName)
    }
  }

  const isPushing = (branchName: string) => {
    return pushingBranches.value.has(branchName)
  }

  return { pushBranch, isPushing }
}
