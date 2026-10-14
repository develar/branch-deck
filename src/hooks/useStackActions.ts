import { toast } from "@/components/ui/toast"
import type { MoveCommitsResult } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand, showUndoToast } from "@/lib/command"
import type { CommandResult } from "@/lib/command"
import { formatCommitCount } from "@/lib/commit"
import { notifyError } from "@/lib/errors"
import { undoMove } from "@/hooks/useMoveCommits"
import { syncBranches } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { getSelectedProject } from "@/stores/repository"

/** Runs a prefix rewrite of a stack operation, then offers an undo, syncs and reveals `revealBranch` */
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
  const { moved } = result.data
  if (moved.length === 0) {
    toast.add({ type: "info", title: "Nothing to change" })
    return true
  }
  requestReveal({ branchName: revealBranch, commitIds: [] })
  showUndoToast(successTitle(formatCommitCount(moved.length)), () => undoMove(moved))
  void syncBranches({ autoScroll: false, autoExpand: false })
  return true
}

/** Stacks `branchName` on `parent`, or makes it a root (`null`), by rewriting the prefixes of its commits */
export function restackBranch(branchName: string, parent: string | null): Promise<boolean> {
  return runStackOperation(
    () => commands.restackBranch({ repositoryPath: getSelectedProject()?.path ?? "", branchName, parent }),
    parent === null ? `Failed to Unstack ${branchName}` : `Failed to Stack ${branchName} on ${parent}`,
    count => parent === null ? `Unstacked ${branchName} (${count})` : `Stacked ${branchName} on ${parent} (${count})`,
    branchName,
  )
}

/** Renames a branch together with its name in the prefixes of the branches stacked on it */
export function renameBranch(branchName: string, newName: string): Promise<boolean> {
  return runStackOperation(
    () => commands.renameBranch({ repositoryPath: getSelectedProject()?.path ?? "", branchName, newName }),
    `Failed to Rename ${branchName}`,
    count => `Renamed ${branchName} to ${newName} (${count})`,
    newName,
  )
}
