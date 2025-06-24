import { ref } from "vue"
import { PushBranch } from "../../wailsjs/go/main/App"
import { useToast } from "@nuxt/ui/composables/useToast"
import {VcsRequestFactory} from "./vcsRequest"

export function usePush(vcsRequestFactory: VcsRequestFactory) {
  const pushingBranches = ref(new Set<string>())

  const toast = useToast()

  const pushBranch = async (branchName: string) => {
    pushingBranches.value.add(branchName)
    try {
      const result = await PushBranch(vcsRequestFactory.createRequest(), branchName)
      if (result.success) {
        toast.add({
          title: "Success",
          description: result.message,
          color: "success",
        })
      } else {
        toast.add({
          title: "Push Failed",
          description: result.message || "Unknown error occurred",
          color: "error",
        })
      }

      return result
    } catch (error) {
      const errorMessage = error.message || "Failed to push branch"
      if (toast) {
        toast.add({
          title: "Push Failed",
          description: errorMessage,
          color: "error",
        })
      }
      return { success: false, error: errorMessage }
    } finally {
      pushingBranches.value.delete(branchName)
    }
  }

  const isPushing = (branchName: string) => {
    return pushingBranches.value.has(branchName)
  }

  return {
    pushBranch,
    isPushing,
  }
}
