import { toast } from "@/components/ui/toast"
import type { LandingFix, LandingRefusal, RewriteIntent } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { formatCommitCount } from "@/lib/commit"
import { logError } from "@/lib/errors"
import { FROM_MENU } from "@/lib/menu"
import { countMoved } from "@/lib/move-summary"
import type { RewriteRunSpec } from "@/lib/rewrite-run"
import { runRewriteIntent, undoRewrite, witnessed } from "@/lib/rewrite-run"
import { formatMoveUnstackAction } from "@/lib/stack"
import { joinNames } from "@/lib/text"
import { showUndoToast } from "@/lib/undo-toast"
import { branchSyncStore, wholeStackRootIn } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { getSelectedProject, getSelectedRepositoryPath } from "@/stores/repository"
import { forgetCollapsedStack, renameCollapsedStack } from "@/stores/stack-collapse"

export interface StackOperationOptions {
  /** The surface that started it and shows its spinner (`SelectContext.surface`) */
  surface?: string | null
  /** The busy state, when the form that starts it says more ("Naming the branch auth") */
  label?: string
}

/** A restack or an unstack that fixes what a start needs (`RewriteIntent.fixes`) */
export interface LandingFixOptions extends StackOperationOptions {
  /** The refusal of the landing gate it fixes: the engine runs it on fresh landing needs, while they still refuse with it */
  fixes?: LandingRefusal
  /** The engine refused it because the facts changed (`plan_changed`: the fresh needs no longer refuse with it) */
  onPlanChanged?: () => void
  /** Shows the row of the branch after it (the default); false keeps the view the user pressed it in (the Plan of Landing) */
  reveal?: boolean
  /**
   * Land Together of a stack that forks (`EdgeAction.landTogether.forks`): its toast offers the Safe Push of the whole
   * stack it made, as Rust offers it (`safe-push-whole-stack`: enabled once the sync built its stack branch)
   */
  offerWholeStack?: boolean
}

function stackSuccessTitle(title: string, suffix: string): string {
  return `${title}${suffix}`
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
  /** The branch whose row it shows after it; null keeps the view the user pressed it in */
  revealBranch: string | null,
  /** Runs with the id of its toast: an action the toast offers once it is known */
  withToast?: (toastId: string) => void,
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
  if (revealBranch !== null) {
    requestReveal({ branchName: revealBranch, commitIds: [] })
  }
  // a change of the stack parents alone moves no commit
  const moved = countMoved(done.result.answer)
  const toastId = showUndoToast(
    repositoryPath,
    successTitle(moved === 0 ? "" : ` (${formatCommitCount(moved)})`),
    () => undoRegroup(repositoryPath, undo.token, "Cannot undo the change", "Undoing the change"),
    undefined,
    done.outcome,
  )
  withToast?.(toastId)
  return true
}

/**
 * Runs the fix of what a start needs (`LandingFix`): the restack of the root of what it lands onto the branch it needs,
 * or the unstack of a branch from the one whose declared need it inherits, with what it fixes (`options.fixes`: the
 * engine runs it on fresh landing needs while they still refuse with it); false for a fix no rewrite runs (the group of
 * an unassigned commit, the start of a cycle), which the Safe Push popover says
 */
export function runLandingFix(fix: LandingFix, options: LandingFixOptions = {}): boolean {
  switch (fix.kind) {
    case "restack":
      void restackBranch(fix.branch, fix.onto, options)
      return true
    case "unstack":
      void restackBranch(fix.branch, null, options)
      return true
    case "moveCommit":
    case "start":
      return false
  }
}

/**
 * Stacks `branchName` on `parent`, or makes it a root (`null`), in the stack parents; only the commits whose prefix
 * declares another parent are rewritten. Resolves after the sync, so that the caller sees the branches as the
 * operation left them.
 */
export async function restackBranch(branchName: string, parent: string | null, options: LandingFixOptions = {}): Promise<boolean> {
  const fixes = options.fixes === undefined ? {} : { fixes: options.fixes }
  const done = await runStackOperation(
    { surface: options.surface, ...(options.onPlanChanged !== undefined && { onPlanChanged: options.onPlanChanged }) },
    repositoryPath =>
      parent === null ? witnessed(repositoryPath, { rewrite: "unstack", branchName, ...fixes }) : witnessed(repositoryPath, { rewrite: "restack", branchName, parent, ...fixes }),
    suffix => stackSuccessTitle(parent === null ? `Unstacked ${branchName}` : `Stacked ${branchName} on ${parent}`, suffix),
    options.reveal === false ? null : branchName,
    parent !== null && options.offerWholeStack === true ? toastId => void offerWholeStackStart(toastId, parent) : undefined,
  )
  // a collapsed root stacked on another branch heads no stack of its own anymore
  if (done && parent !== null) {
    forgetCollapsedStack(branchName)
  }
  return done
}

/**
 * The toast of a Land Together that made a stack that forks offers the start that lands it, "Safe Push Whole Stack" of
 * the whole stack of `branch` (now one stack with it): as Rust offers that action (`safe-push-whole-stack`) and
 * disables it with its reason (its stack branch not built yet, `StackNotBuilt`; the readiness still to come), followed
 * while the toast is open; Undo stays the toast's own action. One toast for the event. The builder of the actions of the
 * whole stack is imported when asked: its module reaches this one through the start of a Safe Push
 */
async function offerWholeStackStart(toastId: string, branch: string): Promise<void> {
  const { getWholeStackStartItems } = await import("@/hooks/useWholeStackActions")
  const offer = () => {
    const root = wholeStackRootIn(branchSyncStore.getState(), branch)
    const item = root === null ? undefined : getWholeStackStartItems(root).find(it => it.id === "safe-push-whole-stack")
    toast.update(toastId, {
      data: {
        secondaryAction:
          item === undefined
            ? undefined
            : {
                label: item.label,
                disabled: item.disabled === true,
                onClick: () => {
                  toast.close(toastId)
                  // the start opens at the row of the root, in the branches: shown first, whichever view pressed it
                  requestReveal({ branchName: root ?? branch, commitIds: [] })
                  item.onSelect(FROM_MENU)
                },
              },
      },
    })
  }
  offer()
  const unsubscribe = branchSyncStore.subscribe((state, previous) => {
    if (state.nextSteps !== previous.nextSteps || state.actions !== previous.actions || state.stackBranches !== previous.stackBranches) {
      offer()
    }
  })
  toast.update(toastId, { onClose: unsubscribe })
}

/** Stacks all selected branches oldest-first as one rewrite and forgets the collapsed state of its new children */
export async function stackBranches(names: readonly string[], options: StackOperationOptions = {}): Promise<boolean> {
  const [revealBranch] = names
  if (revealBranch === undefined) {
    return false
  }
  const done = await runStackOperation(
    { label: options.label, surface: options.surface },
    repositoryPath => witnessed(repositoryPath, { rewrite: "stackTogether", branchNames: [...names] }),
    suffix => stackSuccessTitle(`Stacked ${joinNames(names)}`, suffix),
    revealBranch,
  )
  if (!done) {
    return false
  }
  const branches = branchSyncStore.getState().branches
  const lower = names.find(name => {
    const parent = branches[name]?.stack?.declaredParent
    return parent == null || !names.includes(parent)
  })
  if (lower !== undefined) {
    for (const name of names) {
      if (name !== lower) {
        forgetCollapsedStack(name)
      }
    }
  }
  return true
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
    const result = await commands.getStackIndependence({ repositoryPath, branchName })
    if (result.status === "error") {
      // no offer of the unstack, and the log says why the check could not answer; an answer that the unstack is no valid
      // plan (`invalid_plan`: no stacked branch any more, fixups it would strand, a plan refused as asked) is no failure
      if (result.error.code !== "invalid_plan") {
        logError(`Cannot check whether ${branchName} applies alone`, result.error)
      }
      continue
    }
    if (result.data.kind === "builds") {
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
