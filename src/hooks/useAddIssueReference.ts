import { toast } from "@/components/ui/toast"
import { commands } from "@/lib/bindings"
import type { Branch } from "@/stores/branch-sync"
import { syncBranches } from "@/stores/branch-sync"
import { withPostSubmit, withRowProcessing } from "@/stores/inline-row-action"
import { getSelectedProject } from "@/stores/repository"

async function runAddIssueReference(issueReference: string, branch: Branch): Promise<void> {
  const commits = branch.commits.map(commit => ({ hash: commit.originalHash, message: commit.message }))
  if (commits.length === 0) {
    toast.add({
      type: "error",
      title: `${branch.name}: No Commits Found`,
      description: "Branch has no commits to add issue reference to",
    })
    return
  }

  const data = await withRowProcessing(
    branch.name,
    async () => {
      const result = await commands.addIssueReferenceToCommits({
        repositoryPath: getSelectedProject()?.path ?? "",
        branchName: branch.name,
        commits,
        issueReference,
      })
      if (result.status !== "ok") {
        throw new Error(result.error.message)
      }
      return result.data
    },
    {
      processingMessage: `Adding ${issueReference} to ${branch.name}...`,
      success: ({ updatedCount, skippedCount }) => {
        if (updatedCount === 0) {
          return { title: "No Changes Made", description: `All ${skippedCount} commits already have issue references`, duration: 5000 }
        }
        let description = `Updated ${updatedCount} commit${updatedCount === 1 ? "" : "s"}`
        if (skippedCount > 0) {
          description += ` (${skippedCount} already had references)`
        }
        return { title: `${branch.name}: Added ${issueReference}`, description, duration: 5000 }
      },
      error: error => ({
        title: `${branch.name}: Failed to Add Issue Reference`,
        description: error instanceof Error ? error.message : "Failed to add issue reference",
      }),
    },
  )

  if (data) {
    // refresh without auto-expand or auto-scroll
    void syncBranches({ autoScroll: false, autoExpand: false })
  }
}

/** Closes the inline form immediately, then prefixes the branch commits with the issue reference and re-syncs. */
export function addIssueReference(issueReference: string, branch: Branch): void {
  withPostSubmit(() => runAddIssueReference(issueReference, branch))
}

const addIssueReferenceActions = { addIssueReference }

export function useAddIssueReference(): typeof addIssueReferenceActions {
  return addIssueReferenceActions
}
