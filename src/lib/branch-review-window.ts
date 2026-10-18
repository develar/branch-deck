import { getCurrentWindow } from "@tauri-apps/api/window"
import { getBranchContextMenuItems, getBranchReviewItems, getSafePushWithoutReviewItem } from "@/hooks/useBranchContextActions"
import type { BranchActionItem } from "@/hooks/useBranchContextActions"
import type { BranchReviewSource } from "@/hooks/useBranchReview"
import { getCreatePullRequestContext } from "@/hooks/useCreatePullRequestContext"
import type { BranchReviewPart, PatronusReviewRequirement } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { BRANCH_REVIEW_WINDOW, branchGoneReason, requestAction } from "@/lib/branch-review"
import type { BranchReviewFocus } from "@/lib/branch-review"
import { continueOnGitHub, submitPullRequest } from "@/lib/create-pull-request-context"
import { logError, notifyError, recordError } from "@/lib/errors"
import type { MenuItem } from "@/lib/menu"
import { showRepository } from "@/lib/show-repository"
import { stackAncestors } from "@/lib/stack"
import { createWindowLink } from "@/lib/sub-window-link"
import { formatWholeStack } from "@/lib/whole-stack"
import { branchSyncStore, getBranch, getStackBranch } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { getEffectiveBranchPrefix, getSelectedProject } from "@/stores/repository"
import { demandReviewReadiness, showReviewReadiness } from "@/stores/review-readiness"
import type {
  BranchReviewFeedback,
  BranchReviewGroup,
  BranchReviewItem,
  BranchReviewMenuItem,
  BranchReviewRequest,
  BranchReviewWindowData,
  BranchReviewWindowState,
} from "@/types/sub-window"

/** The groups of the last code-owner check of a branch by name, for a review window opened from elsewhere */
const lastReviewGroups = new Map<string, { head: string | undefined; groups: BranchReviewGroup[] }>()

export function toReviewGroups(requirement: PatronusReviewRequirement): BranchReviewGroup[] {
  return requirement.groups.map(group => ({ name: group.name, paths: group.paths, branchWide: group.branchWide }))
}

/** Remembers what the code owners of the branch at this head review (the Safe Push popover checked it) */
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

/** Ends the readiness of the branches the window shows the actions of (`showReviewReadiness`), its chain; null while closed */
let hideChain: (() => void) | null = null

function showChain(chain: readonly string[]): void {
  releaseChain()
  const hides = chain.map(name => showReviewReadiness(name))
  hideChain = () => hides.forEach(hide => hide())
}

function releaseChain(): void {
  hideChain?.()
  hideChain = null
}

/** The open review window: the repository and the chain it shows, or the whole stack of its root */
const link = createWindowLink<{ repositoryPath: string; chain: readonly string[]; wholeStack: boolean }, BranchReviewWindowState>({
  label: BRANCH_REVIEW_WINDOW,
  buildState: shown => (shown.wholeStack ? buildStackState(shown.repositoryPath, shown.chain[0] ?? "") : buildState(shown.repositoryPath, shown.chain)),
  onClose: () => {
    closeDialog()
    releaseChain()
  },
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
  const shown = { repositoryPath, chain, wholeStack: false }
  // the window starts over: no dialog is open in it
  closeDialog()
  link.setShown(shown)
  // its Safe Push of each branch reads the next step, as a row does
  showChain(chain)
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
      // the link forgets a window that failed to open: nothing shows its chain
      if (link.shown() === null) {
        releaseChain()
      }
      notifyError(`Cannot open the review of ${branchName}`, error)
      return false
    },
  )
}

/**
 * Reviews the whole stack of `root` in the review window: its stack branch `{prefix}/stack/<root>`, every commit of the
 * stack on the baseline as one part ("Review Whole Stack"). The window shows no actions of a branch: the stack's are in
 * the menus of the main window. Resolves to whether the window opened; a failure is told.
 */
export async function openWholeStackReviewWindow(root: string): Promise<boolean> {
  const source = getBranchReviewSource(root, {})
  if (source === null) {
    return false
  }
  const { repositoryPath } = source
  const chain = [root]
  closeDialog()
  releaseChain()
  const shown = { repositoryPath, chain, wholeStack: true }
  link.setShown(shown)
  const data: BranchReviewWindowData = {
    ...source,
    chain,
    groups: [],
    wholeStack: true,
    focus: { range: "own", filter: { kind: "all" }, commitId: null, path: null },
    openedAt: Date.now(),
    state: link.takeState()!,
  }
  const subject = formatWholeStack(root)
  return link.open(shown, { url: "/branch-review", title: `Review ${subject}`, data }).then(
    () => true,
    (error: unknown) => {
      notifyError(`Cannot open the review of ${subject}`, error)
      return false
    },
  )
}

/** The tip of the stack branch the last sync left, which a review of the whole stack reads again when it moves */
function buildStackState(repositoryPath: string, root: string): BranchReviewWindowState {
  const tip = getSelectedProject()?.path === repositoryPath ? getStackBranch(root)?.tip : undefined
  return { repositoryPath, selected: tip !== undefined, tips: tip == null ? {} : { [root]: tip }, items: {}, menus: {}, createPullRequest: null }
}

/** The tips of the chain the last sync built, and the actions of each branch; nothing for a repository not shown */
function buildState(repositoryPath: string, chain: readonly string[]): BranchReviewWindowState {
  if (getSelectedProject()?.path !== repositoryPath) {
    return { repositoryPath, selected: false, tips: {}, items: {}, menus: {}, createPullRequest: null }
  }
  const tips: Record<string, string> = {}
  const items: Record<string, BranchReviewItem[]> = {}
  const menus: Record<string, BranchReviewMenuItem[][]> = {}
  for (const name of chain) {
    const tip = syncedTips?.repositoryPath === repositoryPath ? syncedTips.tips[name] : getBranch(name)?.commits[0]?.hash
    if (tip !== undefined) {
      tips[name] = tip
    }
    const branch = getBranch(name)
    if (branch !== undefined) {
      items[name] = getBranchReviewItems(branch).map(toReviewItem)
      menus[name] = getBranchContextMenuItems(branch, revealRow)
        .map(group => group.filter(item => item.id !== undefined && item.children === undefined).map(toMenuItem))
        .filter(group => group.length > 0)
    }
  }
  const dialogBranch = dialog === null ? undefined : getBranch(dialog.branchName)
  return { repositoryPath, selected: true, tips, items, menus, createPullRequest: dialogBranch === undefined ? null : getCreatePullRequestContext(dialogBranch) }
}

function toMenuItem(item: MenuItem): BranchReviewMenuItem {
  return {
    id: item.id!,
    label: item.label,
    ...(item.disabled === true && { disabled: true }),
    ...(item.description !== undefined && { description: item.description }),
  }
}

/** The row of the branch expands for an item that works on its commits (Amend, Unapply), as the reveal of a move does */
function revealRow(name: string, expanded: boolean): void {
  if (expanded) {
    requestReveal({ branchName: name, commitIds: [] })
  }
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
 * front, shows the row of the branch and asks its Safe Push (`requestSafePush`) or pushes it as the push button of the row does
 * (`pushOrConfirm`, a push that drops remote commits asks in the row). The window's "Create pull request" dialog stays
 * in the window: the main window sends its context while it is open, creates the pull request it asks for (the toast
 * goes to the window) and opens the popover of the row's Safe Push without review. One the main window can't run now
 * (the branch left, the action is disabled) is refused with the reason, shown in the window; a request of another
 * repository switches the main window to it first (`showRepository`).
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
  const menuItem = "menuItemId" in request ? findMenuItem(branchName, request.menuItemId) : undefined
  const action =
    "menuItemId" in request
      ? requestAction({ menuLabel: menuItem?.label ?? request.menuItemId }, branchName)
      : "itemId" in request
        ? requestAction(request.itemId, branchName)
        : requestAction(request.dialog === "create" ? "create-pull-request" : request.dialog === "continueOnGitHub" ? "continue-on-github" : "safe-push-without-review", branchName)
  const title = `Cannot ${action}`
  // a reason, or what GitHub said
  const refuse = (reason: unknown) => {
    send({ kind: "error", title, message: recordError(title, reason) })
    return false
  }
  // the window of another repository: the main window switches to it, then runs the action on its branches
  const notShown = await showRepository(repositoryPath)
  if (notShown !== null) {
    return refuse(notShown)
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
  if ("dialog" in request && request.dialog === "continueOnGitHub") {
    return continueOnGitHub(getCreatePullRequestContext(branch), request.fields).then(
      () => true,
      (error: unknown) => refuse(error),
    )
  }
  if ("menuItemId" in request && menuItem === undefined) {
    return refuse(`The menu of ${branchName} has no such action now`)
  }
  const item =
    "menuItemId" in request
      ? menuItem
      : "itemId" in request
        ? getBranchReviewItems(branch).find(it => it.id === request.itemId)
        : (getSafePushWithoutReviewItem(branch) ?? undefined)
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

/** The item of the branch's menu the review window asks to run, as the row's menu has it now */
function findMenuItem(branchName: string, id: string): MenuItem | undefined {
  const branch = getBranch(branchName)
  return branch === undefined
    ? undefined
    : getBranchContextMenuItems(branch, revealRow)
        .flat()
        .find(item => item.id === id && item.children === undefined)
}

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
