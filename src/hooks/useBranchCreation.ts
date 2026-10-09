import { toast } from "@/components/ui/toast"
import { commands } from "@/lib/bindings"

export interface CreateBranchParams {
  repositoryPath: string
  branchName: string
  commitIds: string[]
}

/**
 * Creates a branch from the given commits and reports the outcome with a toast.
 * Resolves to `true` on success. The caller refreshes the branch list (sync) afterwards.
 */
export async function createBranch(params: CreateBranchParams): Promise<boolean> {
  // TODO(Phase 5): run through `withRowProcessing("branch-creation", ...)` of the inline-row-action store,
  //  which also closes the active inline form; the toast handling below mirrors what it does
  try {
    const result = await commands.createBranchFromCommits({
      repositoryPath: params.repositoryPath,
      branchName: params.branchName,
      commitIds: params.commitIds,
    })
    if (result.status !== "ok") {
      throw new Error(result.error.message || `Unable to create branch "${params.branchName}" with selected commits`)
    }

    toast.add({
      type: "success",
      title: "Success",
      description: `Branch "${params.branchName}" created successfully`,
      timeout: 5000,
    })
    return true
  }
  catch (error) {
    toast.add({
      type: "error",
      title: "Branch Creation Failed",
      description: error instanceof Error ? error.message : `Failed to create branch "${params.branchName}"`,
    })
    return false
  }
}

const branchCreation = { createBranch }

export function useBranchCreation(): typeof branchCreation {
  return branchCreation
}
