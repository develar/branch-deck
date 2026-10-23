import { toast } from "@/components/ui/toast"
import type { MoveCommitsResult } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand, showUndoToast } from "@/lib/command"
import type { CommandResult } from "@/lib/command"
import { formatCommitCount } from "@/lib/commit"
import { countMoved } from "@/lib/move-summary"
import { runRewrite } from "@/lib/rewrite"
import type { RewriteSpec } from "@/lib/rewrite"
import { formatMoveUnstackAction } from "@/lib/stack"
import { requestReveal } from "@/stores/commit-move"
import { branchSubject } from "@/stores/operations"
import { getEffectiveBranchPrefix, getSelectedProject, getSelectedRepositoryPath } from "@/stores/repository"
import { forgetCollapsedStack, renameCollapsedStack } from "@/stores/stack-collapse"

export interface StackOperationOptions {
  /** The surface that started it and shows its spinner (`SelectContext.surface`) */
  surface?: string | null
  /** The busy state, when the form that starts it says more ("Naming the branch auth") */
  label?: string
}

/**
 * Runs a regroup of a stack operation as one rewrite (`runRewrite`): the command, the sync, then the toast with an undo
 * (of the messages, the issue groups and the stack parents, in one transaction); reveals `revealBranch`
 */
async function runStackOperation(
  spec: RewriteSpec,
  /** The command in the repository of the click (`repositoryPath`), whichever the main window shows when it runs */
  operation: (repositoryPath: string) => Promise<CommandResult<MoveCommitsResult>>,
  failureTitle: string,
  /** `suffix` is " (3 commits)", or empty when no commit moved */
  successTitle: (suffix: string) => string,
  revealBranch: string,
): Promise<boolean> {
  const repositoryPath = getSelectedRepositoryPath()
  const result = await runRewrite(
    { ...spec, repositoryPath },
    failureTitle,
    () => operation(repositoryPath),
    data => {
      const { undoToken } = data
      if (undoToken == null) {
        return { sync: false, after: () => toast.add({ type: "info", title: "Nothing to change" }) }
      }
      requestReveal({ branchName: revealBranch, commitIds: [] })
      // a change of the stack parents alone moves no commit
      const moved = countMoved(data)
      return {
        sync: true,
        after: () =>
          showUndoToast(successTitle(moved === 0 ? "" : ` (${formatCommitCount(moved)})`), () =>
            undoRegroup(repositoryPath, undoToken, "Cannot undo the change", "Undoing the change"),
          ),
      }
    },
  )
  return result !== null
}

/**
 * Stacks `branchName` on `parent`, or makes it a root (`null`), in the stack parents; only the commits whose prefix
 * declares another parent are rewritten. Resolves after the sync, so that the caller sees the branches as the
 * operation left them.
 */
export async function restackBranch(branchName: string, parent: string | null, options: StackOperationOptions = {}): Promise<boolean> {
  const done = await runStackOperation(
    {
      id: parent === null ? "unstack" : "stack-on",
      subject: branchSubject(branchName),
      objects: [branchName],
      label: parent === null ? `Unstacking ${branchName}` : `Stacking ${branchName} on ${parent}`,
      surface: options.surface,
    },
    repositoryPath => commands.restackBranch({ repositoryPath, branchName, parent }),
    parent === null ? `Cannot unstack ${branchName}` : `Cannot stack ${branchName} on ${parent}`,
    suffix => (parent === null ? `Unstacked ${branchName}${suffix}` : `Stacked ${branchName} on ${parent}${suffix}`),
    branchName,
  )
  // a collapsed root stacked on another branch heads no stack of its own anymore
  if (done && parent !== null) {
    forgetCollapsedStack(branchName)
  }
  return done
}

/** Renames a branch together with its name in the prefixes of the branches stacked on it */
export async function renameBranch(branchName: string, newName: string, options: StackOperationOptions = {}): Promise<boolean> {
  const done = await runStackOperation(
    { id: "rename", subject: branchSubject(branchName), objects: [branchName], label: options.label ?? `Renaming ${branchName} to ${newName}`, surface: options.surface },
    repositoryPath => commands.renameBranch({ repositoryPath, branchName, newName }),
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

/**
 * Moves back what a move, a rename, a restack or a reword wrote in `repositoryPath` (the repository it was made in,
 * whichever the main window shows at the click), HEAD and the issue groups in one transaction, then syncs; a rewrite of
 * its own (`label`: "Undoing the move"). `applied` runs once the undo is written, before the sync (a preview follows
 * its commit back)
 */
export async function undoRegroup(repositoryPath: string, token: string | null | undefined, failureTitle: string, label: string, applied?: () => void): Promise<void> {
  if (token == null) {
    return
  }
  await runRewrite(
    { id: "undo", subject: `undo:${token}`, objects: [], label, repositoryPath },
    failureTitle,
    () => commands.undoRegroup({ repositoryPath, token }),
    () => {
      requestReveal(null)
      applied?.()
      return { sync: true }
    },
  )
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
