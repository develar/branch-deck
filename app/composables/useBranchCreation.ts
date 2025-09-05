import { commands } from "~/utils/bindings"

export function useBranchCreation() {
  const { withRowProcessing } = useInlineRowAction()

  async function createBranch(params: {
    repositoryPath: string
    branchName: string
    commitIds: string[]
  }) {
    const key = "branch-creation" // Use a consistent key for the processing state

    const result = await withRowProcessing(
      key,
      async () => {
        const result = await commands.createBranchFromCommits({
          repositoryPath: params.repositoryPath,
          branchName: params.branchName,
          commitIds: params.commitIds,
        })

        if (result.status !== "ok") {
          throw new Error(result.error || `Unable to create branch "${params.branchName}" with selected commits`)
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

  return {
    createBranch,
  }
}
