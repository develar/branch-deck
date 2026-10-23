import type { Commit } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { showUndoToast } from "@/lib/command"
import { formatCommitCount } from "@/lib/commit"
import { runRewrite, runUndo } from "@/lib/rewrite"
import { getBranchesOfCommits, resolveCommitsOf } from "@/stores/branch-sync"
import { branchSubject } from "@/stores/operations"
import { getSelectedRepositoryPath } from "@/stores/repository"

/**
 * Squashes the commits into the oldest of them with `message`, as one rewrite (`runRewrite`): the squash, the sync, then
 * the toast with an undo. A squash that waited for another rewrite in the queue squashes the commits as that rewrite
 * left them. Resolves after the sync, to false, after an error toast, if the commits could not be squashed; nothing
 * changed then.
 */
export async function squashCommits(commits: readonly Commit[], message: string, surface: string | null = null): Promise<boolean> {
  const repositoryPath = getSelectedRepositoryPath()
  const branchName = getBranchesOfCommits(commits.map(commit => commit.originalHash))[0] ?? null
  const result = await runRewrite(
    {
      id: "squash",
      subject: branchSubject(branchName ?? ""),
      // a squash of other commits of the branch, or with another message, is a squash of its own
      parameters: JSON.stringify([commits.map(commit => commit.originalHash), message]),
      objects: branchName === null ? [] : [branchName],
      label: `Squashing ${formatCommitCount(commits.length)}`,
      surface,
    },
    "Cannot squash the commits",
    // the ids as the rewrite before it in the queue left them (`resolveCommitsOf`)
    wait => commands.squashCommits({ repositoryPath, commitIds: resolveCommitsOf(repositoryPath, commits).map(commit => commit.originalHash), message }, wait),
    ({ newHead, previousHead, squashedCount }) => ({
      sync: true,
      after: () => showUndoToast(`Squashed ${formatCommitCount(squashedCount)}`, () => undoSquash(repositoryPath, newHead, previousHead, "Cannot undo the squash")),
    }),
  )
  return result !== null
}

/**
 * Moves the branch back to the commit before the squash: the squash kept the files, so nothing else changes. A rewrite
 * of its own, then the sync.
 */
export async function undoSquash(repositoryPath: string, expectedHead: string, previousHead: string, failureTitle: string): Promise<boolean> {
  return await runUndo({ id: "undo", subject: `undo:${expectedHead}`, objects: [], label: "Undoing the squash", repositoryPath }, failureTitle, wait =>
    commands.restoreBranchHead({ repositoryPath, expectedHead, previousHead }, wait),
  )
}
