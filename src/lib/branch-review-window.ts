import { emit } from "@tauri-apps/api/event"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { canPushNow } from "@/hooks/usePushButton"
import type { PatronusReviewRequirement } from "@/lib/bindings"
import { BRANCH_REVIEW_ACTIONS_CHANGED } from "@/lib/branch-review"
import type { BranchActionRequest, BranchReviewActionsChanged } from "@/lib/branch-review"
import { logError, notifyError } from "@/lib/errors"
import { pushOrConfirm } from "@/lib/git/push"
import { canStartRobot } from "@/lib/patronus"
import { formatBlockedReason, stackAncestors } from "@/lib/stack"
import { openSubWindow } from "@/lib/window-management"
import type { Branch } from "@/stores/branch-sync"
import { branchSyncStore, getBranch, isBranchPushing, isStackPushing } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { isProcessing, openInline } from "@/stores/inline-row-action"
import { getBranchRobot, getLandingAncestor, getLandingSpan, isPatronusAvailable } from "@/stores/patronus"
import { getPullRequest } from "@/stores/pull-requests"
import { getEffectiveBranchPrefix, getSelectedProject } from "@/stores/repository"
import type { BranchReviewActions, BranchReviewGroup, BranchReviewWindowData } from "@/types/sub-window"

/** The label of the review window: one window, reused for the next branch */
export const BRANCH_REVIEW_WINDOW = "branch-review"

/** The groups of the last code-owner check of a branch by name, for a review window opened from elsewhere */
const lastReviewGroups = new Map<string, { head: string | undefined, groups: BranchReviewGroup[] }>()

export function toReviewGroups(requirement: PatronusReviewRequirement): BranchReviewGroup[] {
  return requirement.groups.map(group => ({ name: group.name, paths: group.paths, branchWide: group.branchWide }))
}

/** Remembers what the code owners of the branch at this head review (the Safe Push form checked it) */
export function rememberReviewGroups(branchName: string, requirement: PatronusReviewRequirement): void {
  lastReviewGroups.set(branchName, { head: getBranch(branchName)?.commits[0]?.hash, groups: toReviewGroups(requirement) })
}

function reviewGroupsOf(branchName: string): BranchReviewGroup[] {
  const last = lastReviewGroups.get(branchName)
  return last !== undefined && last.head === getBranch(branchName)?.commits[0]?.hash ? last.groups : []
}

export interface BranchReviewFocus {
  range?: "own" | "all"
  /** The reviewer group to show the files of */
  group?: string | null
  path?: string | null
  /** The groups of a code-owner check the caller holds; the remembered ones otherwise */
  groups?: BranchReviewGroup[]
}

/** The review window of a branch: its stack from the root, the reviewer groups the code owners check asked for */
export async function openBranchReview(branchName: string, focus: BranchReviewFocus = {}): Promise<void> {
  const repositoryPath = getSelectedProject()?.path
  const baselineBranch = branchSyncStore.getState().baselineBranch
  if (repositoryPath === undefined || baselineBranch === null) {
    return
  }
  const groups = focus.groups ?? reviewGroupsOf(branchName)
  const chain = [...stackAncestors(branchName, getBranch).map(branch => branch.name), branchName]
  const actions = getBranchReviewActions(chain)
  // the window is one, reused: the main window keeps the actions of the last opened chain up to date
  shownReview = { repositoryPath, chain, sent: JSON.stringify(actions) }
  const data: BranchReviewWindowData = {
    repositoryPath,
    branchPrefix: getEffectiveBranchPrefix(),
    baselineBranch,
    chain,
    range: focus.range ?? "own",
    groups,
    group: focus.group ?? null,
    path: focus.path ?? null,
    openedAt: Date.now(),
    actions,
  }
  try {
    await openSubWindow({ windowId: BRANCH_REVIEW_WINDOW, url: "/branch-review", title: `Review ${branchName}`, data })
  }
  catch (error) {
    notifyError(`Cannot open the review of ${branchName}`, error)
  }
}

/** The repository and the chain of the last opened review window, with the actions last sent to it */
let shownReview: { repositoryPath: string, chain: readonly string[], sent: string } | null = null

/** Why the push button of the row can't push the branch now (`canPushNow`); null when it can */
function pushBlocker(branch: Branch): string | null {
  const name = branch.name
  const isSyncing = branchSyncStore.getState().isSyncing
  const isPushing = isBranchPushing(name) || isStackPushing(name)
  const processing = isProcessing(name)
  if (canPushNow(branch, { isSyncing, isPushing, processing })) {
    return null
  }
  if (branch.status === "Blocked") {
    return formatBlockedReason(branch)
  }
  if (isSyncing || branch.status === "Syncing") {
    return "Wait until the sync ends"
  }
  if (isPushing) {
    return `${name} is being pushed`
  }
  return processing ? `Wait until the action on ${name} ends` : "Nothing to push: the remote branch is up to date"
}

/** Why the Safe Push form of the row can't start a robot now (`canStartRobot`, as S on the row); null when it can */
function safePushBlocker(branch: Branch): string | null {
  const name = branch.name
  if (isProcessing(name)) {
    return `Wait until the action on ${name} ends`
  }
  const start = canStartRobot(branch, getBranchRobot(name), getLandingAncestor(name), getLandingSpan(name))
  return start.ok ? null : start.reason
}

/**
 * What the review window offers for each branch of `chain` in the selected repository, as the branch preview does: the
 * pull request, "Safe Push…" where Patronus is and the branch has work, "Push" for a branch with work
 */
export function getBranchReviewActions(chain: readonly string[]): Record<string, BranchReviewActions> {
  const actions: Record<string, BranchReviewActions> = {}
  for (const name of chain) {
    const branch = getBranch(name)
    if (branch === undefined) {
      continue
    }
    const pullRequest = getPullRequest(name)
    // as the push button of the row: only a branch that synced without an error has something to push
    const hasPush = !branch.hasError && branch.commitCount > 0
    actions[name] = {
      pullRequest: pullRequest === undefined ? null : { number: pullRequest.number, url: pullRequest.url },
      safePush: hasPush && isPatronusAvailable() ? { reason: safePushBlocker(branch) } : null,
      push: hasPush ? { reason: pushBlocker(branch) } : null,
    }
  }
  return actions
}

/**
 * Tells the review window what it can do now (`branch-review-actions-changed`), when that changed since it was told: a
 * broadcast, as the main window doesn't track whether the window is still open. While the main window shows another
 * repository, the window gets no actions. Main window only.
 */
export function broadcastBranchReviewActions(): void {
  const shown = shownReview
  if (shown === null) {
    return
  }
  const branches = getSelectedProject()?.path === shown.repositoryPath ? getBranchReviewActions(shown.chain) : {}
  const sent = JSON.stringify(branches)
  if (sent === shown.sent) {
    return
  }
  shown.sent = sent
  const payload: BranchReviewActionsChanged = { repositoryPath: shown.repositoryPath, branches }
  emit(BRANCH_REVIEW_ACTIONS_CHANGED, payload).catch((error: unknown) => logError("Cannot tell the review window about the branch actions", error))
}

/**
 * Runs an action the review window asks for (`branch-action`) in the main window, which owns the branches: brings the
 * window to the front, shows the row of the branch and opens its Safe Push form or pushes it as the push button of the
 * row does (`pushOrConfirm`, a push that drops remote commits asks in the row). A request for another repository than
 * the selected one is ignored; a branch that can't act says why.
 */
export function runBranchAction({ repositoryPath, branchName, action }: BranchActionRequest): void {
  if (getSelectedProject()?.path !== repositoryPath) {
    return
  }
  getCurrentWindow().setFocus().catch((error: unknown) => logError("Cannot focus the main window", error))
  const failure = action === "push" ? `Cannot push ${branchName}` : `Cannot start the Safe Push of ${branchName}`
  const branch = getBranch(branchName)
  if (branch === undefined) {
    notifyError(failure, "The branch is not in the table: sync and try again.")
    return
  }
  requestReveal({ branchName, commitIds: [] })
  const actions = getBranchReviewActions([branchName])[branchName]!
  const option = action === "push" ? actions.push : actions.safePush
  const reason = option === null
    ? (action === "push" || isPatronusAvailable() ? `${branchName} has nothing to push` : "Patronus is not available for the repository")
    : option.reason
  if (reason !== null) {
    notifyError(failure, reason)
  }
  else if (action === "push") {
    pushOrConfirm(branchName, "row")
  }
  else {
    openInline("safe-push", branchName)
  }
}
