import { toast } from "@/components/ui/toast"
import { commands } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"
import type { Branch } from "@/stores/branch-sync"
import { syncBranches } from "@/stores/branch-sync"
import { withRowProcessing } from "@/stores/inline-row-action"
import { getSelectedProject } from "@/stores/repository"

/**
 * Prefixes the branch commits with the issue reference, then re-syncs, while the inline form shows its progress: one
 * operation. A failure keeps the form with the typed reference.
 */
export async function addIssueReference(issueReference: string, branch: Branch): Promise<void> {
  const commits = branch.commits.map(commit => ({ hash: commit.originalHash, message: commit.message }))
  if (commits.length === 0) {
    // nothing to do: not an error
    toast.add({ type: "info", title: `No commits to add ${issueReference} to`, description: `${branch.name} has no commits.` })
    return
  }

  await withRowProcessing(
    branch.name,
    { id: "add-issue-reference", kind: "rewrite", label: `Adding ${issueReference} to ${branch.name}` },
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
      // refresh without auto-expand or auto-scroll
      await syncBranches({ autoScroll: false, autoExpand: false })
      return result.data
    },
    {
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
}
