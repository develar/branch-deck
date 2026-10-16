import { toast } from "@/components/ui/toast"
import type { MoveCommitsResult } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand, showUndoToast } from "@/lib/command"
import type { CommandResult } from "@/lib/command"
import { formatCommitCount } from "@/lib/commit"
import { notifyError } from "@/lib/errors"
import { countMoved } from "@/lib/move-summary"
import { undoRegroup } from "@/hooks/useMoveCommits"
import { syncBranches } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { getSelectedProject } from "@/stores/repository"
import { forgetCollapsedStack, renameCollapsedStack } from "@/stores/stack-collapse"

/**
 * Runs a regroup of a stack operation, then offers an undo (of the messages, the issue groups and the stack
 * parents, in one transaction), syncs and reveals `revealBranch`
 */
async function runStackOperation(
  operation: () => Promise<CommandResult<MoveCommitsResult>>,
  failureTitle: string,
  /** `suffix` is " (3 commits)", or empty when no commit moved */
  successTitle: (suffix: string) => string,
  revealBranch: string,
): Promise<boolean> {
  const result = await invokeCommand(operation)
  if (result.status !== "ok") {
    notifyError(failureTitle, result.error)
    return false
  }
  const { undoToken } = result.data
  if (undoToken == null) {
    toast.add({ type: "info", title: "Nothing to change" })
    return true
  }
  requestReveal({ branchName: revealBranch, commitIds: [] })
  // a change of the stack parents alone moves no commit
  const moved = countMoved(result.data)
  showUndoToast(successTitle(moved === 0 ? "" : ` (${formatCommitCount(moved)})`), () => undoRegroup(undoToken, "Cannot undo the change"))
  void syncBranches({ autoScroll: false, autoExpand: false })
  return true
}

/**
 * Stacks `branchName` on `parent`, or makes it a root (`null`), in the stack parents; only the commits whose prefix
 * declares another parent are rewritten
 */
export async function restackBranch(branchName: string, parent: string | null): Promise<boolean> {
  const done = await runStackOperation(
    () => commands.restackBranch({ repositoryPath: getSelectedProject()?.path ?? "", branchName, parent }),
    parent === null ? `Cannot unstack ${branchName}` : `Cannot stack ${branchName} on ${parent}`,
    suffix => parent === null ? `Unstacked ${branchName}${suffix}` : `Stacked ${branchName} on ${parent}${suffix}`,
    branchName,
  )
  // a collapsed root stacked on another branch heads no stack of its own anymore
  if (done && parent !== null) {
    forgetCollapsedStack(branchName)
  }
  return done
}

/** Renames a branch together with its name in the prefixes of the branches stacked on it */
export async function renameBranch(branchName: string, newName: string): Promise<boolean> {
  const done = await runStackOperation(
    () => commands.renameBranch({ repositoryPath: getSelectedProject()?.path ?? "", branchName, newName }),
    `Cannot rename ${branchName}`,
    suffix => `Renamed ${branchName} to ${newName}${suffix}`,
    newName,
  )
  // a collapsed stack stays collapsed under the new name of its root
  if (done) {
    renameCollapsedStack(branchName, newName)
  }
  return done
}
