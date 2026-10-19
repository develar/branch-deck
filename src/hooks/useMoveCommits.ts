import { toast } from "@/components/ui/toast"
import { offerUnstackAfterMove, undoRegroup } from "@/hooks/useStackActions"
import type { Commit, MoveCommitsResult } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { showUndoToast } from "@/lib/command"
import { formatCommitCount } from "@/lib/commit"
import { capitalize, countMoved, describeMove } from "@/lib/move-summary"
import { runRewrite } from "@/lib/rewrite"
import { branchSyncStore, getBranchesOfCommits } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { branchSubject } from "@/stores/operations"
import { getSelectedProject } from "@/stores/repository"

export interface MoveOptions {
  /** The action, as the id of its item: "move-to" by default, "ungroup", "remove-prefix" */
  id?: string
  /** The busy state, "Moving 3 commits to auth" by default */
  label?: string
  /** The surface that started it and shows its spinner (`SelectContext.surface`) */
  surface?: string | null
}

/**
 * Moves commits to `targetBranch` (an existing or a new branch), or back (`null`: out of their prefix branch or out of
 * the issue group of their key), with the smallest change (`move_commits_to_branch`): a whole issue goes by the issue
 * groups, without a rewrite, the other commits by their `(branch)` prefix. One rewrite (`runRewrite`): the move, the
 * sync, then the toast with an undo of the whole move and the moved commits revealed in the target branch; when a
 * stacked branch of the move builds alone afterwards, the toast also offers its unstack. Resolves after the sync, to
 * false, after an error toast, if the commits could not be moved.
 */
export async function moveCommits(commits: readonly Commit[], targetBranch: string | null, options: MoveOptions = {}): Promise<boolean> {
  const sources = getBranchesOfCommits(commits.map(commit => commit.originalHash))
  const stacked = stackedBranchesOfMove(sources, targetBranch)
  const objects = [...new Set([...sources, ...(targetBranch === null ? [] : [targetBranch])])]
  const commitCount = formatCommitCount(commits.length)
  const result = await runRewrite(
    {
      id: options.id ?? "move-to",
      subject: branchSubject(sources[0] ?? targetBranch ?? ""),
      objects,
      label: options.label ?? (targetBranch === null ? `Moving ${commitCount} back` : `Moving ${commitCount} to ${targetBranch}`),
      surface: options.surface,
    },
    targetBranch === null ? "Cannot move the commits back" : `Cannot move the commits to ${targetBranch}`,
    () =>
      commands.moveCommitsToBranch({
        repositoryPath: getSelectedProject()?.path ?? "",
        commitIds: commits.map(commit => commit.originalHash),
        targetBranch,
      }),
    moved => {
      const count = countMoved(moved)
      if (count === 0) {
        return { sync: false, after: () => toast.add({ type: "info", title: targetBranch === null ? "Nothing to move back" : `Already in ${moved.target ?? targetBranch}` }) }
      }
      const target = moved.target ?? null
      requestReveal(target === null ? null : { branchName: target, commitIds: movedIds(moved) })
      const description = describeMove(moved, target, true)
      return {
        sync: true,
        after: () => {
          const toastId = showUndoToast(
            moveTitle(moved, target, count),
            () => undoRegroup(moved.undoToken, "Cannot undo the move", "Undoing the move"),
            description === undefined ? undefined : capitalize(description),
          )
          void offerUnstackAfterMove(toastId, stacked, target)
        },
      }
    },
  )
  return result !== null
}

/** The target and the source branches of a move that are stacked, as the last sync knows them: the target first */
function stackedBranchesOfMove(sources: readonly string[], targetBranch: string | null): string[] {
  const { branches } = branchSyncStore.getState()
  const names = [...new Set([...(targetBranch === null ? [] : [targetBranch]), ...sources])]
  return names.filter(name => (branches[name]?.stack?.declaredParent ?? null) !== null)
}

function moveTitle(result: MoveCommitsResult, target: string | null, count: number): string {
  if (target !== null) {
    return `Moved ${formatCommitCount(count)} to ${target}`
  }
  const onlyPrefixes = result.commits.every(commit => commit.change === "PrefixRemoved" || commit.change === "Unchanged")
  return onlyPrefixes ? `Removed the prefix of ${formatCommitCount(count)}` : `Moved ${formatCommitCount(count)} back`
}

/** The ids of the commits a move changed, after it */
export function movedIds(result: MoveCommitsResult): string[] {
  return result.commits.filter(commit => commit.change !== "Unchanged").map(commit => commit.newId)
}
