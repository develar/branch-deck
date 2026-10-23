import { toast } from "@/components/ui/toast"
import { commands } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"
import type { Branch } from "@/stores/branch-sync"
import { resolveCommitsOf, syncBranches } from "@/stores/branch-sync"
import { withRowProcessing } from "@/stores/inline-row-action"
import { getSelectedRepositoryPath, isSelectedRepository } from "@/stores/repository"

/**
 * Prefixes the branch commits with the issue reference, then re-syncs, while the inline form shows its progress: one
 * operation. A failure keeps the form with the typed reference. A start that waits for another rewrite in the queue
 * runs in the repository of its start, on the commits as that rewrite left them.
 */
export async function addIssueReference(issueReference: string, branch: Branch): Promise<void> {
  if (branch.commits.length === 0) {
    // nothing to do: not an error
    toast.add({ type: "info", title: `No commits to add ${issueReference} to`, description: `${branch.name} has no commits.` })
    return
  }

  // the repository of the start: a start that waits in the queue runs there, whichever the main window shows by then
  const repositoryPath = getSelectedRepositoryPath()
  await withRowProcessing(
    branch.name,
    { id: "add-issue-reference", kind: "rewrite", label: `Adding ${issueReference} to ${branch.name}`, repositoryPath },
    async () => {
      const result = await commands.addIssueReferenceToCommits({
        repositoryPath,
        branchName: branch.name,
        // the ids as the rewrite before it in the queue left them (`resolveCommitsOf`)
        commits: resolveCommitsOf(repositoryPath, branch.commits).map(commit => ({ hash: commit.originalHash, message: commit.message })),
        issueReference,
      })
      if (result.status !== "ok") {
        throw new Error(result.error.message)
      }
      // refresh without auto-expand or auto-scroll; the table shows another repository after a switch: this one is
      // synced when it opens
      if (isSelectedRepository(repositoryPath)) {
        await syncBranches({ autoScroll: false, autoExpand: false })
      }
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
