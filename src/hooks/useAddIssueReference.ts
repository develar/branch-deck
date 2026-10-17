import { toast } from "@/components/ui/toast"
import { commands } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"
import type { Branch } from "@/stores/branch-sync"
import { syncBranches } from "@/stores/branch-sync"
import { withRowProcessing } from "@/stores/inline-row-action"
import { getSelectedProject } from "@/stores/repository"

/**
 * Prefixes the branch commits with the issue reference while the inline form shows its progress, then re-syncs. A
 * failure keeps the form with the typed reference.
 */
export async function addIssueReference(issueReference: string, branch: Branch): Promise<void> {
  const commits = branch.commits.map(commit => ({ hash: commit.originalHash, message: commit.message }))
  if (commits.length === 0) {
    // nothing to do: not an error
    toast.add({ type: "info", title: `No commits to add ${issueReference} to`, description: `${branch.name} has no commits.` })
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
      processingMessage: `Adding ${issueReference} to ${branch.name}…`,
      success: ({ updatedCount, skippedCount }) => {
        if (updatedCount === 0) {
          return { type: "info", title: `Nothing to add to ${branch.name}`, description: "Every commit has an issue reference already." }
        }
        const skipped = skippedCount === 0 ? "" : ` · ${skippedCount} had an issue reference already`
        return { title: `Added ${issueReference} to ${branch.name}`, description: `${formatCommitCount(updatedCount)}${skipped}` }
      },
      errorTitle: () => `Cannot add ${issueReference} to ${branch.name}`,
      keepFormOnError: true,
    },
  )

  if (data) {
    // refresh without auto-expand or auto-scroll
    void syncBranches({ autoScroll: false, autoExpand: false })
  }
}
