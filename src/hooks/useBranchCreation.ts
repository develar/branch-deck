import { commands } from "@/lib/bindings"
import { withRowProcessing } from "@/stores/inline-row-action"

export interface CreateBranchParams {
  repositoryPath: string
  branchName: string
  commitIds: string[]
}

/**
 * Creates a branch from the given commits; the outcome is reported with a toast and the active inline form is closed.
 * Resolves to `true` on success. The caller refreshes the branch list (sync) afterwards.
 */
export async function createBranch(params: CreateBranchParams): Promise<boolean> {
  const result = await withRowProcessing(
    // a fixed key for the processing state
    "branch-creation",
    async () => {
      const result = await commands.createBranchFromCommits({
        repositoryPath: params.repositoryPath,
        branchName: params.branchName,
        commitIds: params.commitIds,
      })
      if (result.status !== "ok") {
        throw new Error(result.error.message || `Unable to create branch "${params.branchName}" with selected commits`)
      }
      return result.data
    },
    {
      processingMessage: `Creating branch "${params.branchName}"...`,
      success: () => ({
        title: "Success",
        description: `Branch "${params.branchName}" created successfully`,
        duration: 5000,
      }),
      error: error => ({
        title: "Branch Creation Failed",
        description: error instanceof Error ? error.message : `Failed to create branch "${params.branchName}"`,
      }),
    },
  )
  return result !== undefined
}

const branchCreation = { createBranch }

export function useBranchCreation(): typeof branchCreation {
  return branchCreation
}
