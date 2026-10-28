import { toast } from "@/components/ui/toast"
import { formatCommitCount } from "@/lib/commit"
import { runRewriteIntent, witnessed } from "@/lib/rewrite-run"
import { showToast } from "@/lib/toast-message"
import type { Branch } from "@/stores/branch-sync"
import { inlineSurface, withRowProgress } from "@/stores/inline-row-action"
import { getSelectedRepositoryPath } from "@/stores/repository"

/**
 * Prefixes the branch commits with the issue reference, as the rewrite `addIssueReference` of the engine
 * (`runRewriteIntent`) in the repository of the press, then the sync that shows it, while the inline form shows its
 * progress. A failure keeps the form with the typed reference. A start that waits for another rewrite in the queue of
 * the session runs in the repository of its press, on the commits as that rewrite left them (the session follows their
 * ids).
 */
export async function addIssueReference(issueReference: string, branch: Branch): Promise<void> {
  if (branch.commits.length === 0) {
    // nothing to do: not an error
    toast.add({ type: "info", title: `No commits to add ${issueReference} to`, description: `${branch.name} has no commits.` })
    return
  }

  // the repository of the press: a start that waits in the queue runs there, whichever the main window shows by then
  const repositoryPath = getSelectedRepositoryPath()
  const commits = branch.commits.map(commit => ({ hash: commit.originalHash, message: commit.message }))
  await withRowProgress(
    branch.name,
    async () => {
      const done = await runRewriteIntent(
        {
          surface: inlineSurface(branch.name),
          repositoryPath,
        },
        witnessed(repositoryPath, { rewrite: "addIssueReference", branchName: branch.name, commits, issueReference }),
      )
      if (done === null || done.result.rewrite !== "issueReferenceAdded") {
        return false
      }
      const { updatedCount, skippedCount } = done.result.answer
      if (updatedCount === 0) {
        showToast({ type: "info", title: `Nothing to add to ${branch.name}`, description: "Every commit has an issue reference already." })
      } else {
        const skipped = skippedCount === 0 ? "" : ` · ${skippedCount} had an issue reference already`
        showToast({ title: `Added ${issueReference} to ${branch.name}`, description: `${formatCommitCount(updatedCount)}${skipped}` })
      }
      return true
    },
    { keepFormOnFailure: true },
  )
}
