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
 * Runs a regroup of a stack operation, then offers an undo (of the messages and of the issue groups, in one
 * transaction), syncs and reveals `revealBranch`
 */
async function runStackOperation(
  operation: () => Promise<CommandResult<MoveCommitsResult>>,
  failureTitle: string,
  successTitle: (count: string) => string,
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
  showUndoToast(successTitle(formatCommitCount(countMoved(result.data))), () => undoRegroup(undoToken, "Cannot undo the change"))
  void syncBranches({ autoScroll: false, autoExpand: false })
  return true
}

/** Stacks `branchName` on `parent`, or makes it a root (`null`), by rewriting the prefixes of its commits */
export async function restackBranch(branchName: string, parent: string | null): Promise<boolean> {
  const done = await runStackOperation(
    () => commands.restackBranch({ repositoryPath: getSelectedProject()?.path ?? "", branchName, parent }),
    parent === null ? `Cannot unstack ${branchName}` : `Cannot stack ${branchName} on ${parent}`,
    count => parent === null ? `Unstacked ${branchName} (${count})` : `Stacked ${branchName} on ${parent} (${count})`,
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
    count => `Renamed ${branchName} to ${newName} (${count})`,
    newName,
  )
  // a collapsed stack stays collapsed under the new name of its root
  if (done) {
    renameCollapsedStack(branchName, newName)
  }
  return done
}
