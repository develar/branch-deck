import { toast } from "@/components/ui/toast"
import type { RewriteIntent } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand } from "@/lib/command"
import { formatCommitCount } from "@/lib/commit"
import { countMoved } from "@/lib/move-summary"
import type { RewriteRunSpec } from "@/lib/rewrite-run"
import { runRewriteIntent, undoRewrite, witnessed } from "@/lib/rewrite-run"
import { formatMoveUnstackAction } from "@/lib/stack"
import { showUndoToast } from "@/lib/undo-toast"
import { requestReveal } from "@/stores/commit-move"
import { getSelectedProject, getSelectedRepositoryPath } from "@/stores/repository"
import { forgetCollapsedStack, renameCollapsedStack } from "@/stores/stack-collapse"

export interface StackOperationOptions {
  /** The surface that started it and shows its spinner (`SelectContext.surface`) */
  surface?: string | null
  /** The busy state, when the form that starts it says more ("Naming the branch auth") */
  label?: string
}

/**
 * Runs a regroup of a stack operation as a rewrite of the engine (`runRewriteIntent`) in the repository of the click:
 * the rewrite, the sync, then the toast with its undo (of the messages, the issue groups and the stack parents, in one
 * transaction); reveals `revealBranch`
 */
async function runStackOperation(
  spec: Omit<RewriteRunSpec, "repositoryPath">,
  rewrite: (repositoryPath: string) => RewriteIntent,
  /** `suffix` is " (3 commits)", or empty when no commit moved */
  successTitle: (suffix: string) => string,
  revealBranch: string,
): Promise<boolean> {
  const repositoryPath = getSelectedRepositoryPath()
  const done = await runRewriteIntent({ ...spec, repositoryPath }, rewrite(repositoryPath))
  if (done === null || done.result.rewrite !== "moved") {
    return false
  }
  const { undo } = done
  if (undo === null) {
    toast.add({ type: "info", title: "Nothing to change" })
    return true
  }
  requestReveal({ branchName: revealBranch, commitIds: [] })
  // a change of the stack parents alone moves no commit
  const moved = countMoved(done.result.answer)
  showUndoToast(
    repositoryPath,
    successTitle(moved === 0 ? "" : ` (${formatCommitCount(moved)})`),
    () => undoRegroup(repositoryPath, undo.token, "Cannot undo the change", "Undoing the change"),
    undefined,
    done.outcome,
  )
  return true
}

/**
 * Stacks `branchName` on `parent`, or makes it a root (`null`), in the stack parents; only the commits whose prefix
 * declares another parent are rewritten. Resolves after the sync, so that the caller sees the branches as the
 * operation left them.
 */
export async function restackBranch(branchName: string, parent: string | null, options: StackOperationOptions = {}): Promise<boolean> {
  const done = await runStackOperation(
    { surface: options.surface },
    repositoryPath => (parent === null ? witnessed(repositoryPath, { rewrite: "unstack", branchName }) : witnessed(repositoryPath, { rewrite: "restack", branchName, parent })),
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
    { label: options.label, surface: options.surface },
    repositoryPath => witnessed(repositoryPath, { rewrite: "rename", branchName, newName }),
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
 * The Undo of a move, a rename, a restack, an unstack, a reword or a fix in `repositoryPath` (the repository it was
 * made in, whichever the main window shows at the click): the rewrite `undo` of its regroup token, HEAD and the stores
 * back in one transaction, then the sync; queued behind a rewrite that runs (`label`: "Undoing the move"). `applied`
 * runs once the undo is written, before the sync shows it (a preview follows its commit back while it is there)
 */
export async function undoRegroup(repositoryPath: string, token: string | null | undefined, failureTitle: string, label: string, applied?: () => void): Promise<boolean> {
  if (token == null) {
    return false
  }
  return await undoRewrite(repositoryPath, { kind: "regroup", token }, label, failureTitle, () => {
    requestReveal(null)
    applied?.()
  })
}

/**
 * After a move: the first of `branches` (stacked, the target first) that builds alone now gets its unstack on the move's
 * toast, "Unstack from ‹parent›" for the target, else "Unstack ‹branch› from ‹parent›" (`formatMoveUnstackAction`). Never an unstack on its own: the stack parents don't say why a branch is stacked, and Stack On
 * writes the same line as a suggestion. One trial build per branch, without the repository lock; a failure offers
 * nothing.
 */
export async function offerUnstackAfterMove(toastId: string, branches: readonly string[], moveTarget: string | null): Promise<void> {
  const repositoryPath = getSelectedProject()?.path ?? ""
  for (const branchName of branches) {
    const result = await invokeCommand(() => commands.getStackIndependence({ repositoryPath, branchName }))
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
