import { getCurrentWindow } from "@tauri-apps/api/window"
import { getBranchReviewItems, getSafePushWithoutReviewItem } from "@/hooks/useBranchContextActions"
import type { BranchActionItem } from "@/hooks/useBranchContextActions"
import type { BranchReviewSource } from "@/hooks/useBranchReview"
import { getCreatePullRequestContext } from "@/hooks/useCreatePullRequestContext"
import type { BranchReviewPart, PatronusReviewRequirement } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { BRANCH_REVIEW_WINDOW, branchGoneReason, requestAction } from "@/lib/branch-review"
import type { BranchReviewFocus } from "@/lib/branch-review"
import { submitPullRequest } from "@/lib/create-pull-request-context"
import { logError, notifyError, recordError } from "@/lib/errors"
import { stackAncestors } from "@/lib/stack"
import { createWindowLink } from "@/lib/sub-window-link"
import { branchSyncStore, getBranch } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { getEffectiveBranchPrefix, getSelectedProject } from "@/stores/repository"
import { demandReviewReadiness } from "@/stores/review-readiness"
import { otherRepositoryReason } from "@/stores/review-requests"
import type { BranchReviewFeedback, BranchReviewGroup, BranchReviewItem, BranchReviewRequest, BranchReviewWindowData, BranchReviewWindowState } from "@/types/sub-window"

/** The groups of the last code-owner check of a branch by name, for a review window opened from elsewhere */
const lastReviewGroups = new Map<string, { head: string | undefined; groups: BranchReviewGroup[] }>()

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

/** The heads of the branches the last sync of a repository built: what the review window compares its stack with */
let syncedTips: { repositoryPath: string; tips: Readonly<Record<string, string>> } | null = null

/**
 * The branch whose "Create pull request" dialog the window has open: its context goes with the state, and its
 * readiness (the code owners) is asked for meanwhile
 */
let dialog: { branchName: string; release: () => void } | null = null

function closeDialog(): void {
  dialog?.release()
  dialog = null
}

/** The open review window: the repository and the chain it shows */
const link = createWindowLink<{ repositoryPath: string; chain: readonly string[] }, BranchReviewWindowState>({
  label: BRANCH_REVIEW_WINDOW,
  buildState: shown => buildState(shown.repositoryPath, shown.chain),
  onClose: closeDialog,
})

/** What a review of a branch reads: its chain from the root of its stack, the reviewer groups of the code-owner check */
function getBranchReviewSource(branchName: string, focus: BranchReviewFocus): BranchReviewSource | null {
  const repositoryPath = getSelectedProject()?.path
  const baselineBranch = branchSyncStore.getState().baselineBranch
  if (repositoryPath === undefined || baselineBranch === null) {
    return null
  }
  return {
    repositoryPath,
    branchPrefix: getEffectiveBranchPrefix(),
    baselineBranch,
    chain: [...stackAncestors(branchName, getBranch).map(branch => branch.name), branchName],
    groups: focus.groups ?? reviewGroupsOf(branchName),
  }
}

/** The part a review of a range shows first, when the backend can read it ahead: the own part or the whole stack */
function prefetchedPart(range: BranchReviewFocus["range"]): BranchReviewPart | null {
  return range === undefined || range === "own" ? "own" : range === "all" ? "all" : null
}

/** What was read ahead, by the repository, the chain, its heads and the part: once per state of the branch */
const prefetched = new Set<string>()

/**
 * Reads ahead what the review window of a branch asks for first (`prefetch_branch_review`: its parts, the files and
 * the patch of the part it shows), once per state of the branch: on the intent to review (the pointer rests on its
 * row, its row has the focus) and on the open, beside the start of the window
 */
export function prefetchBranchReview(branchName: string, focus: BranchReviewFocus = {}): void {
  const source = getBranchReviewSource(branchName, focus)
  const part = prefetchedPart(focus.range)
  if (source === null || part === null) {
    return
  }
  const { repositoryPath, branchPrefix, baselineBranch, chain } = source
  const key = [repositoryPath, part, ...chain.map(name => `${name}@${getBranch(name)?.commits[0]?.hash ?? ""}`)].join("\n")
  if (prefetched.has(key)) {
    return
  }
  prefetched.add(key)
  void commands.prefetchBranchReview({ review: { repositoryPath, branchPrefix, baselineBranch, chain: [...chain] }, part }).then(result => {
    if (result.status === "error") {
      prefetched.delete(key)
      logError("Cannot read the review of a branch ahead", result.error)
    }
  })
}

/**
 * Reviews a branch in its window, the only review of a branch: its stack from the root, the reviewer groups the code
 * owners check asked for, from `focus` (the part, the filter, the commit, the file). Resolves to whether the window
 * opened; a failure is told.
 */
export async function openBranchReviewWindow(branchName: string, focus: BranchReviewFocus = {}): Promise<boolean> {
  const source = getBranchReviewSource(branchName, focus)
  if (source === null) {
    return false
  }
  const { repositoryPath, chain } = source
  // git reads while the window starts
  prefetchBranchReview(branchName, focus)
  // the window is one, reused: the main window keeps the state of the last opened chain up to date
  const shown = { repositoryPath, chain }
  // the window starts over: no dialog is open in it
  closeDialog()
  link.setShown(shown)
  const data: BranchReviewWindowData = {
    ...source,
    chain: [...chain],
    groups: [...source.groups],
    focus: { range: focus.range ?? "own", filter: focus.filter ?? { kind: "all" }, commitId: focus.commitId ?? null, path: focus.path ?? null },
    openedAt: Date.now(),
    state: link.takeState()!,
  }
  return link.open(shown, { url: "/branch-review", title: `Review ${branchName}`, data }).then(
    () => true,
    (error: unknown) => {
      notifyError(`Cannot open the review of ${branchName}`, error)
      return false
    },
  )
}

/** The tips of the chain the last sync built, and the actions of each branch; nothing for a repository not shown */
function buildState(repositoryPath: string, chain: readonly string[]): BranchReviewWindowState {
  if (getSelectedProject()?.path !== repositoryPath) {
    return { repositoryPath, selected: false, tips: {}, items: {}, createPullRequest: null }
  }
  const tips: Record<string, string> = {}
  const items: Record<string, BranchReviewItem[]> = {}
  for (const name of chain) {
    const tip = syncedTips?.repositoryPath === repositoryPath ? syncedTips.tips[name] : getBranch(name)?.commits[0]?.hash
    if (tip !== undefined) {
      tips[name] = tip
    }
    const branch = getBranch(name)
    if (branch !== undefined) {
      items[name] = getBranchReviewItems(branch).map(toReviewItem)
    }
  }
  const dialogBranch = dialog === null ? undefined : getBranch(dialog.branchName)
  return { repositoryPath, selected: true, tips, items, createPullRequest: dialogBranch === undefined ? null : getCreatePullRequestContext(dialogBranch) }
}

function toReviewItem(item: BranchActionItem): BranchReviewItem {
  return {
    id: item.id as BranchReviewItem["id"],
    label: item.label,
    ...(item.kbds !== undefined && { kbds: item.kbds }),
    ...(item.disabled === true && { disabled: true }),
    ...(item.description !== undefined && { description: item.description }),
    ...(item.url !== undefined && { url: item.url }),
  }
}

/** A sync of `repositoryPath` built these heads: the review window reads its stack again when a head of it moved */
export function noteSyncedTips(repositoryPath: string, tips: Readonly<Record<string, string>>): void {
  syncedTips = { repositoryPath, tips }
  link.broadcast()
}

/**
 * Tells the review window what changed for its chain (the tips of a sync, the actions of its branches), when it
 * changed since it was told; nothing while it is closed. Main window only.
 */
export function broadcastBranchReviewState(): void {
  link.broadcast()
}

/**
 * Runs an action the review window asks for, in the main window, which owns the branches: brings the window to the
 * front, shows the row of the branch and opens its Safe Push form or pushes it as the push button of the row does
 * (`pushOrConfirm`, a push that drops remote commits asks in the row). The window's "Create pull request" dialog stays
 * in the window: the main window sends its context while it is open, creates the pull request it asks for (the toast
 * goes to the window) and opens the Safe Push form without review of the row. One the main window can't run now
 * (another repository is shown, the branch left, the action is disabled) is refused with the reason, shown in the window.
 */
export const runBranchReviewRequest = link.serve<BranchReviewRequest, BranchReviewFeedback>(async (request, send) => {
  const { repositoryPath, branchName } = request
  if ("dialog" in request && (request.dialog === "open" || request.dialog === "close")) {
    closeDialog()
    if (request.dialog === "open") {
      dialog = { branchName, release: demandReviewReadiness(branchName) }
    }
    link.broadcast()
    return true
  }
  const action =
    "itemId" in request ? requestAction(request.itemId, branchName) : requestAction(request.dialog === "create" ? "create-pull-request" : "safe-push-without-review", branchName)
  const title = `Cannot ${action}`
  // a reason, or what GitHub said
  const refuse = (reason: unknown) => {
    send({ kind: "error", title, message: recordError(title, reason) })
    return false
  }
  if (getSelectedProject()?.path !== repositoryPath) {
    return refuse(otherRepositoryReason(repositoryPath, action))
  }
  const branch = getBranch(branchName)
  if (branch === undefined) {
    return refuse(branchGoneReason(branchName))
  }
  if ("dialog" in request && request.dialog === "create") {
    const context = getCreatePullRequestContext(branch)
    if (context.newPullRequestBlocker !== null) {
      return refuse(context.newPullRequestBlocker)
    }
    return submitPullRequest(context, request.input).then(
      ({ toast, pullRequest }) => {
        send({ kind: "toast", toast, pullRequestUrl: pullRequest.url })
        return true
      },
      (error: unknown) => refuse(error),
    )
  }
  const item = "itemId" in request ? getBranchReviewItems(branch).find(it => it.id === request.itemId) : (getSafePushWithoutReviewItem(branch) ?? undefined)
  if (item === undefined) {
    return refuse(unavailableReason("itemId" in request ? request.itemId : "safe-push-without-review", branchName))
  }
  if (item.disabled === true) {
    return refuse(item.description ?? `${item.label} is not available now`)
  }
  getCurrentWindow()
    .setFocus()
    .catch((error: unknown) => logError("Cannot focus the main window", error))
  requestReveal({ branchName, commitIds: [] })
  item.onSelect()
  return true
})

/** Why the main window doesn't offer the action of a request on the branch */
function unavailableReason(itemId: "safe-push" | "push" | "safe-push-without-review", branchName: string): string {
  switch (itemId) {
    case "push":
      return `${branchName} has nothing to push`
    case "safe-push":
      return "Patronus is not available for the repository"
    case "safe-push-without-review":
      return `The approvals of ${branchName} don't wait for a pull request now`
  }
}

/** Forgets the window (tests) */
export function resetBranchReviewWindow(): void {
  closeDialog()
  link.reset()
  syncedTips = null
}
