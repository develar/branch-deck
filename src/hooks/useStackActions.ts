import { toast } from "@/components/ui/toast"
import type { MoveCommitsResult } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand, showUndoToast } from "@/lib/command"
import type { CommandResult } from "@/lib/command"
import { formatCommitCount } from "@/lib/commit"
import { notifyError } from "@/lib/errors"
import { countMoved } from "@/lib/move-summary"
import { formatMoveUnstackAction } from "@/lib/stack"
import { syncBranches } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { getEffectiveBranchPrefix, getSelectedProject } from "@/stores/repository"
import { forgetCollapsedStack, renameCollapsedStack } from "@/stores/stack-collapse"

export interface StackOperationOptions {
  /** Resolve after the sync, so that the caller sees the branches as the operation left them; else before it */
  awaitSync?: boolean
}

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
  options: StackOperationOptions = {},
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
  const sync = syncBranches({ autoScroll: false, autoExpand: false })
  if (options.awaitSync === true) {
    await sync
  }
  return true
}

/**
 * Stacks `branchName` on `parent`, or makes it a root (`null`), in the stack parents; only the commits whose prefix
 * declares another parent are rewritten
 */
export async function restackBranch(branchName: string, parent: string | null, options: StackOperationOptions = {}): Promise<boolean> {
  const done = await runStackOperation(
    () => commands.restackBranch({ repositoryPath: getSelectedProject()?.path ?? "", branchName, parent }),
    parent === null ? `Cannot unstack ${branchName}` : `Cannot stack ${branchName} on ${parent}`,
    suffix => (parent === null ? `Unstacked ${branchName}${suffix}` : `Stacked ${branchName} on ${parent}${suffix}`),
    branchName,
    options,
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

/** Moves back what a move, a rename or a restack wrote, HEAD and the issue groups in one transaction, then syncs */
export async function undoRegroup(token: string | null | undefined, failureTitle: string): Promise<void> {
  if (token == null) {
    return
  }
  const result = await invokeCommand(() => commands.undoRegroup({ repositoryPath: getSelectedProject()?.path ?? "", token }))
  if (result.status !== "ok") {
    notifyError(failureTitle, result.error)
    return
  }
  requestReveal(null)
  await syncBranches({ autoScroll: false, autoExpand: false })
}

/**
 * After a move: the first of `branches` (stacked, the target first) that builds alone now gets its unstack on the move's
 * toast, "Unstack from ‹parent›" for the target, else "Unstack ‹branch› from ‹parent›" (`formatMoveUnstackAction`). Never an unstack on its own: the stack parents don't say why a branch is stacked, and Stack On
 * writes the same line as a suggestion. One trial build per branch, without the repository lock; a failure offers
 * nothing.
 */
export async function offerUnstackAfterMove(toastId: string, branches: readonly string[], moveTarget: string | null): Promise<void> {
  const repositoryPath = getSelectedProject()?.path ?? ""
  const branchPrefix = getEffectiveBranchPrefix()
  for (const branchName of branches) {
    const result = await invokeCommand(() => commands.getStackIndependence({ repositoryPath, branchPrefix, branchName }))
    if (result.status === "ok" && result.data.kind === "builds") {
      const parent = result.data.parent
      toast.update(toastId, {
        data: {
          secondaryAction: {
            label: formatMoveUnstackAction(branchName, parent, moveTarget),
            onClick: () => {
              toast.close(toastId)
              void restackBranch(branchName, null)
            },
          },
        },
      })
      return
    }
  }
}
