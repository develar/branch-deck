import { commands } from "~/utils/bindings"

export function useBranchCreation() {
  const isCreating = ref(false)
  const toast = useToast()

  async function createBranch(params: {
    repositoryPath: string
    branchName: string
    commitIds: string[]
  }) {
    if (isCreating.value) return false

    isCreating.value = true
    try {
      const result = await commands.createBranchFromCommits({
        repositoryPath: params.repositoryPath,
        branchName: params.branchName,
        commitIds: params.commitIds,
      })

      if (result.status === "ok") {
        return true
      }
      else {
        toast.add({
          title: "Branch Creation Failed",
          description: result.error || `Unable to create branch "${params.branchName}" with selected commits`,
          color: "error",
        })
        return false
      }
    }
    catch (error) {
      toast.add({
        title: "Unexpected Error",
        description: error instanceof Error ? error.message : `An unexpected error occurred while creating branch "${params.branchName}"`,
        color: "error",
      })
      return false
    }
    finally {
      isCreating.value = false
    }
  }

  return {
    isCreating: readonly(isCreating),
    createBranch,
  }
}