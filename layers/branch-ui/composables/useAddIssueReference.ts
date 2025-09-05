import { commands } from "~/utils/bindings"

export function useAddIssueReference() {
  const toast = useToast()
  const { selectedProject } = useRepository()
  const { syncBranches } = useBranchSync()
  const { withRowProcessing, withPostSubmit, closeInline } = useInlineRowAction()

  const addIssueReference = async (issueReference: string, branch: ReactiveBranch) => {
    // Get commits with hash and message
    const commits = branch.commits.map(commit => ({
      hash: commit.originalHash,
      message: commit.message,
    }))

    // Validate we have commits to update
    if (commits.length === 0) {
      toast.add({
        title: `${branch.name}: No Commits Found`,
        description: "Branch has no commits to add issue reference to",
        color: "error",
      })
      return
    }

    const data = await withRowProcessing(
      branch.name,
      async () => {
        const result = await commands.addIssueReferenceToCommits({
          repositoryPath: selectedProject.value?.path || "",
          branchName: branch.name,
          commits,
          issueReference,
        })
        if (result.status !== "ok") {
          throw new Error(result.error)
        }
        return result.data
      },
      {
        processingMessage: `Adding ${issueReference} to ${branch.name}...`,
        success: ({ updatedCount, skippedCount }) => {
          if (updatedCount > 0) {
            let description = `Updated ${updatedCount} commit${updatedCount === 1 ? "" : "s"}`
            if (skippedCount > 0) {
              description += ` (${skippedCount} already had references)`
            }
            return { title: `${branch.name}: Added ${issueReference}`, description, duration: 5000 }
          }
          else {
            return { title: "No Changes Made", description: `All ${skippedCount} commits already have issue references`, duration: 5000 }
          }
        },
        error: error => ({
          title: `${branch.name}: Failed to Add Issue Reference`,
          description: error instanceof Error ? error.message : "Failed to add issue reference",
        }),
      },
    )

    if (data) {
      // Refresh branches without auto-expand or auto-scroll
      // noinspection ES6MissingAwait
      syncBranches({ autoScroll: false, autoExpand: false })
    }
  }

  const handleSubmit = (issueReference: string, branch: ReactiveBranch) => {
    withPostSubmit(() => addIssueReference(issueReference, branch))
  }

  return {
    addIssueReference: handleSubmit,
    hideInline: () => closeInline(),
  }
}
