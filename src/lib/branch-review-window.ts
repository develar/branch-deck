import { getCurrentWindow } from "@tauri-apps/api/window"
import { getBranchReviewItems } from "@/hooks/useBranchContextActions"
import type { BranchActionItem } from "@/hooks/useBranchContextActions"
import type { BranchReviewSource } from "@/hooks/useBranchReview"
import type { PatronusReviewRequirement } from "@/lib/bindings"
import { BRANCH_REVIEW_WINDOW, branchGoneReason, requestAction } from "@/lib/branch-review"
import type { BranchReviewFocus } from "@/lib/branch-review"
import { logError, notifyError, recordError } from "@/lib/errors"
import { stackAncestors } from "@/lib/stack"
import { createWindowLink } from "@/lib/sub-window-link"
import { openBranchReviewDialog } from "@/stores/branch-preview"
import { branchSyncStore, getBranch } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { getEffectiveBranchPrefix, getSelectedProject } from "@/stores/repository"
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

/** The open review window: the repository and the chain it shows */
const link = createWindowLink<{ repositoryPath: string; chain: readonly string[] }, BranchReviewWindowState>({
  label: BRANCH_REVIEW_WINDOW,
  buildState: shown => buildState(shown.repositoryPath, shown.chain),
})

/**
 * Reviews a branch: in the dialog of the branch preview, or in the review window when `inWindow` or while the window is
 * open for the repository (it takes the reviews of its kind, as the review window of a pull request does)
 */
export function openBranchReview(branchName: string, focus: BranchReviewFocus = {}, { inWindow = false }: { inWindow?: boolean } = {}): void {
  const repositoryPath = getSelectedProject()?.path
  if (repositoryPath === undefined) {
    return
  }
  if (inWindow || link.isOpenFor(repositoryPath)) {
    void openBranchReviewWindow(branchName, focus)
  } else {
    openBranchReviewDialog(branchName, focus, false)
  }
}

/** What a review of a branch reads: its chain from the root of its stack, the reviewer groups of the code-owner check */
export function getBranchReviewSource(branchName: string, focus: BranchReviewFocus): BranchReviewSource | null {
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

/** The review window of a branch: its stack from the root, the reviewer groups the code owners check asked for */
export async function openBranchReviewWindow(branchName: string, focus: BranchReviewFocus = {}): Promise<void> {
  const source = getBranchReviewSource(branchName, focus)
  if (source === null) {
    return
  }
  const { repositoryPath, chain } = source
  // the window is one, reused: the main window keeps the state of the last opened chain up to date
  const shown = { repositoryPath, chain }
  link.setShown(shown)
  const data: BranchReviewWindowData = {
    ...source,
    chain: [...chain],
    groups: [...source.groups],
    focus: { range: focus.range ?? "own", filter: focus.filter ?? { kind: "all" }, commitId: focus.commitId ?? null, path: focus.path ?? null },
    openedAt: Date.now(),
    state: link.takeState()!,
  }
  try {
    await link.open(shown, { url: "/branch-review", title: `Review ${branchName}`, data })
  } catch (error) {
    notifyError(`Cannot open the review of ${branchName}`, error)
  }
}

/** The tips of the chain the last sync built, and the actions of each branch; nothing for a repository not shown */
function buildState(repositoryPath: string, chain: readonly string[]): BranchReviewWindowState {
  if (getSelectedProject()?.path !== repositoryPath) {
    return { repositoryPath, selected: false, tips: {}, items: {} }
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
      items[name] = getBranchReviewItems(branch, "review").map(toReviewItem)
    }
  }
  return { repositoryPath, selected: true, tips, items }
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
 * (`pushOrConfirm`, a push that drops remote commits asks in the row). One the main window can't run now (another
 * repository is shown, the branch left, the action is disabled) is refused with the reason, shown in the window.
 */
export const runBranchReviewRequest = link.serve<BranchReviewRequest, BranchReviewFeedback>(({ repositoryPath, branchName, itemId }, send) => {
  const action = requestAction(itemId, branchName)
  const title = `Cannot ${action}`
  const refuse = (reason: string) => {
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
  const item = getBranchReviewItems(branch, "review").find(it => it.id === itemId)
  if (item === undefined) {
    return refuse(unavailableReason(itemId, branchName))
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
function unavailableReason(itemId: BranchReviewRequest["itemId"], branchName: string): string {
  switch (itemId) {
    case "push":
      return `${branchName} has nothing to push`
    case "safe-push":
      return "Patronus is not available for the repository"
    case "create-pull-request":
      return `${branchName} has a pull request, isn't on the remote, or its repository isn't on GitHub`
  }
}

/** Forgets the window (tests) */
export function resetBranchReviewWindow(): void {
  link.reset()
  syncedTips = null
}
