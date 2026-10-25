import { getCurrentWindow } from "@tauri-apps/api/window"
import { getBranchContextMenuItems, getBranchReviewItems, getSafePushWithoutReviewItem } from "@/hooks/useBranchContextActions"
import type { BranchActionItem } from "@/hooks/useBranchContextActions"
import { getBranchNextStepOf } from "@/hooks/useBranchNextStep"
import type { BranchReviewSource } from "@/hooks/useBranchReview"
import { getCommitMenuGroups } from "@/hooks/useCommitMoveList"
import { getCreatePullRequestContext, getStackPullRequestContext } from "@/hooks/useCreatePullRequestContext"
import { moveCommits } from "@/hooks/useMoveCommits"
import { getRewordMenuItem } from "@/hooks/useRewordCommit"
import { getWholeStackRest, isForkingStack } from "@/hooks/useStackNextStep"
import { getWholeStackMenuItems } from "@/hooks/useWholeStackActions"
import type { BranchReviewPart, PatronusReviewRequirement } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { formatBranchesAndCommits } from "@/lib/branch"
import { BRANCH_REVIEW_WINDOW, branchGoneReason, branchReviewBlocker, planReviewOpen, requestAction, reviewCommitKey } from "@/lib/branch-review"
import type { BranchReviewFocus, ShownBranchReview } from "@/lib/branch-review"
import { continueOnGitHub, submitPullRequest } from "@/lib/create-pull-request-context"
import { logError, notifyError, recordError } from "@/lib/errors"
import { formatShortHash } from "@/lib/hash"
import type { MenuItem, MenuItemGroups } from "@/lib/menu"
import { findMenuItem, FROM_MENU } from "@/lib/menu"
import { showRepository } from "@/lib/show-repository"
import { stackAncestors } from "@/lib/stack"
import { createWindowLink } from "@/lib/sub-window-link"
import { showToast } from "@/lib/toast-message"
import { formatWholeStack, getStackBranchBlocker, stackReadinessKey } from "@/lib/whole-stack"
import { demandReviewReadiness, showReviewReadiness } from "@/model/demand"
import { branchSyncStore, getBranch, getStackBranch, getStackDescendants, mainLineOf } from "@/stores/branch-sync"
import type { Branch, SyncedCommit } from "@/stores/branch-sync"
import { requestCommitForm, requestReveal } from "@/stores/commit-move"
import type { CommitFormRequest, RevealRequest } from "@/stores/commit-move"
import { getOperations } from "@/stores/operations"
import { getStackPullRequest } from "@/stores/pull-requests"
import { getSelectedProject } from "@/stores/repository"
import type {
  BranchReviewFeedback,
  BranchReviewGroup,
  BranchReviewItem,
  BranchReviewMenuItem,
  BranchReviewRequest,
  BranchReviewWindowData,
  BranchReviewWindowState,
  ReviewedBranch,
  WholeStackScope,
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
  return knownReviewGroupsOf(branchName) ?? []
}

/** The groups of the last code-owner check of the branch at its head; null while none answered for it */
function knownReviewGroupsOf(branchName: string): BranchReviewGroup[] | null {
  const last = lastReviewGroups.get(branchName)
  return last !== undefined && last.head === getBranch(branchName)?.commits[0]?.hash ? last.groups : null
}

/**
 * The branch whose "Create pull request" dialog the window has open: its context goes with the state, and its
 * readiness (the code owners) is asked for meanwhile
 */
let dialog: { branchName: string; wholeStack: boolean; release: () => void } | null = null

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
const link = createWindowLink<ShownBranchReview, BranchReviewWindowState>({
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
  const { repositoryPath, baselineBranch, chain } = source
  const key = [repositoryPath, part, ...chain.map(name => `${name}@${getBranch(name)?.commits[0]?.hash ?? ""}`)].join("\n")
  if (prefetched.has(key)) {
    return
  }
  prefetched.add(key)
  void commands.prefetchBranchReview({ review: { repositoryPath, baselineBranch, chain: [...chain] }, part }).then(result => {
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
  const { repositoryPath } = source
  // the window is one, reused: a commit of the chain on screen moves there, the branch on screen comes to the front
  const open = planReviewOpen(link.shown(), { repositoryPath, branchName, chain: source.chain, focus }, Date.now())
  const restarts = open.startedAt !== link.shown()?.startedAt
  // a start over would drop what is typed into the "Create pull request" dialog of the window
  if (restarts && dialog !== null) {
    tellDialogOpen(branchName, dialog.branchName)
    return false
  }
  const chain = open.chain
  // git reads while the window starts
  prefetchBranchReview(branchName, focus)
  // the main window keeps the state of the last opened chain up to date
  const shown: ShownBranchReview = { repositoryPath, chain, wholeStack: false, startedAt: open.startedAt }
  if (restarts) {
    // the window starts over: no dialog is open in it
    closeDialog()
    // its Safe Push of each branch reads the next step, as a row does
    showChain(chain)
  }
  link.setShown(shown)
  const data: BranchReviewWindowData = {
    ...source,
    chain: [...chain],
    groups: open.move ? reviewGroupsOf(chain.at(-1)!) : [...source.groups],
    // a move falls back to the part of the commit's branch when the part on screen doesn't have it
    focus: {
      range: open.move ? { from: branchName, to: branchName } : (focus.range ?? "own"),
      filter: focus.filter ?? { kind: "all" },
      commitId: focus.commitId ?? null,
      path: focus.path ?? null,
    },
    openedAt: Date.now(),
    startedAt: open.startedAt,
    move: open.move,
    state: link.takeState()!,
  }
  const commitId = focus.commitId
  const title = commitId == null ? `Review ${branchName}` : `Review ${branchName} · ${formatShortHash(commitId)}`
  return link.open(shown, { url: "/branch-review", title, data }).then(
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
 * Reviews a commit of a branch in the review window of the branch, at the commit (and `path`): the stack, its actions
 * and the commits of the branch beside it. Null when the branch review can't show it: the branch is not built on its
 * parents (blocked, failed, in conflict) or its virtual branch doesn't have the commit yet; the review window of the
 * commit shows it then.
 */
export function reviewBranchCommit(branchName: string, commitHash: string, path: string | null): Promise<boolean> | null {
  const branch = getBranch(branchName)
  if (branch === undefined || branchReviewBlocker(branch) !== null || branch.status === "MergeConflict" || branch.status === "AnalyzingConflict") {
    return null
  }
  const copy = branch.commits.find(it => it.originalHash === commitHash)
  if (copy?.hash === undefined || copy.status === "Error" || copy.status === "Blocked") {
    return null
  }
  return openBranchReviewWindow(branchName, { commitId: copy.hash, path })
}

/** An open that would start the window over while its "Create pull request" dialog holds what the user typed */
function tellDialogOpen(subject: string, dialogBranch: string): void {
  showToast({
    type: "info",
    title: `Cannot review ${subject}`,
    description: `The review window has the pull request of ${dialogBranch} open: create it or close the dialog first`,
  })
}

/**
 * Reviews the whole stack of `root` in the review window: its stack branch `{prefix}/stack/<root>`, every commit of the
 * stack on the baseline as one part ("Review Whole Stack"), each commit named by its branch; the segments of `from`
 * (the chain the window showed, else the main line) go back to a branch; its title bar holds the actions of the whole
 * stack (`getWholeStackReviewItems`), its "⋯" all of them. `groups`: the reviewer groups of the code-owner check of the
 * whole stack (the Safe Push popover). Resolves to whether the window opened; a failure is told.
 */
export async function openWholeStackReviewWindow(root: string, options: { readonly from?: readonly string[]; readonly groups?: BranchReviewGroup[] } = {}): Promise<boolean> {
  const from = options.from ?? mainLineOf(branchSyncStore.getState(), root)
  const source = getBranchReviewSource(root, {})
  if (source === null) {
    return false
  }
  const { repositoryPath } = source
  const chain = [root]
  if (dialog !== null) {
    tellDialogOpen(formatWholeStack(root), dialog.branchName)
    return false
  }
  const startedAt = Date.now()
  const shown: ShownBranchReview = { repositoryPath, chain, wholeStack: true, startedAt }
  link.setShown(shown)
  // its Safe Push reads the next step of the whole stack, as the header of its island does
  showChain([stackReadinessKey(root)])
  const data: BranchReviewWindowData = {
    ...source,
    chain,
    groups: options.groups ?? [],
    wholeStack: true,
    from: from.map((name): ReviewedBranch => ({ name, commits: getBranch(name)?.commitCount ?? 0 })),
    focus: { range: "own", filter: { kind: "all" }, commitId: null, path: null },
    openedAt: startedAt,
    startedAt,
    move: false,
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

/** The actions of the whole stack of `root` while the main window shows its repository and the stack has a stack branch */
function buildStackState(repositoryPath: string, root: string): BranchReviewWindowState {
  const selected = getSelectedProject()?.path === repositoryPath && getStackBranch(root) !== undefined
  return {
    repositoryPath,
    selected,
    // the actions of the whole stack, under its root: the window's title bar and "⋯" render them as a branch's
    items: selected ? { [root]: getWholeStackReviewItems(root).map(toReviewItem) } : {},
    menus: selected ? { [root]: [wholeStackMenu(root).map(toMenuItem)] } : {},
    createPullRequest: selected && dialog?.wholeStack === true ? getStackPullRequestContext(root) : null,
    wholeStack: null,
    commitMenus: {},
    groups: null,
  }
}

/** The whole menu of the whole stack for the "⋯" of its review: without its review, which the window is */
function wholeStackMenu(root: string): MenuItem[] {
  return getWholeStackMenuItems(root, { withPush: true, landInPieces: true }).filter(
    item => item.id !== undefined && item.id !== "review-whole-stack" && item.children === undefined,
  )
}

/**
 * The actions of the whole stack in the title bar of its review, as a branch's: O its pull request (opened by the
 * window) or "Create Pull Request of the Stack…" (its dialog in the window), S "Safe Push Whole Stack", Push with
 * Children of the root (no key: P is the previous change)
 */
function getWholeStackReviewItems(root: string): BranchActionItem[] {
  const pullRequest = getStackPullRequest(root)
  const keys: Readonly<Record<string, string[]>> = { "open-stack-pull-request": ["o"], "safe-push-whole-stack": ["s"] }
  // in the order of a branch's: its pull request, its Safe Push, its push
  const order = ["open-stack-pull-request", "create-stack-pull-request", "safe-push-whole-stack", "push-with-children"]
  const menu = wholeStackMenu(root)
  return order
    .flatMap(id => menu.filter(item => item.id === id))
    .map(item => ({
      ...item,
      ...(keys[item.id!] !== undefined && { kbds: keys[item.id!] }),
      ...(item.id === "push-with-children" && { kbds: undefined }),
      ...(item.id === "open-stack-pull-request" && pullRequest !== undefined && { url: pullRequest.url }),
    }))
}

/**
 * "Whole Stack" after "From Root" in the stack bar of `chain`: every branch of the stack of its root, while it has
 * branches outside the chain; for a stack that forks its stack branch, for a straight chain its last branch from the
 * root, which holds them all. Disabled with the reason while that review can't open.
 */
function wholeStackScopeOf(chain: readonly string[], baselineBranch: string): WholeStackScope | null {
  const root = chain[0]
  const last = chain.at(-1)
  if (root === undefined || last === undefined) {
    return null
  }
  const stack = getStackBranch(root)
  if (isForkingStack(stack)) {
    const blocker = stack.tip === null ? (getStackBranchBlocker(root, stack) ?? "The stack branch is not built yet: sync first") : null
    if (blocker !== null) {
      return { root, disabled: true, description: blocker }
    }
    const rest = getWholeStackRest(stack)
    return {
      root,
      description: `Every branch of ${formatWholeStack(root)}, its side branches too, on ${baselineBranch}: ${formatBranchesAndCommits(rest.members.length, rest.commitCount)}, what its Safe Push lands`,
    }
  }
  if (getStackDescendants(last).length === 0) {
    return null
  }
  const mainLine = mainLineOf(branchSyncStore.getState(), root)
  const leaf = mainLine.at(-1)
  if (leaf === undefined || leaf === last) {
    return null
  }
  const leafBranch = getBranch(leaf)
  const blocker = leafBranch === undefined ? branchGoneReason(leaf) : branchReviewBlocker(leafBranch)
  const commits = mainLine.reduce((sum, name) => sum + (getBranch(name)?.commitCount ?? 0), 0)
  return {
    root,
    ...(blocker !== null && { disabled: true }),
    description: blocker ?? `${root} to ${leaf} on ${baselineBranch}: ${formatBranchesAndCommits(mainLine.length, commits)}, what the Safe Push of ${leaf} lands`,
  }
}

/** The actions of each branch of the chain; nothing for a repository not shown */
function buildState(repositoryPath: string, chain: readonly string[]): BranchReviewWindowState {
  if (getSelectedProject()?.path !== repositoryPath) {
    return { repositoryPath, selected: false, items: {}, menus: {}, createPullRequest: null, wholeStack: null, commitMenus: {}, groups: null }
  }
  const items: Record<string, BranchReviewItem[]> = {}
  const menus: Record<string, BranchReviewMenuItem[][]> = {}
  const commitMenus: Record<string, BranchReviewMenuItem[][]> = {}
  for (const name of chain) {
    const branch = getBranch(name)
    if (branch !== undefined) {
      items[name] = getBranchReviewItems(branch).map(toReviewItem)
      menus[name] = getBranchContextMenuItems(branch, revealRow, getOperations())
        .map(group => group.filter(item => item.id !== undefined && item.children === undefined).map(toMenuItem))
        .filter(group => group.length > 0)
      for (const commit of branch.commits) {
        if (commit.hash !== undefined) {
          commitMenus[commit.hash] = toMenuGroups(getReviewCommitMenu(branch, commit))
        }
      }
    }
  }
  const dialogBranch = dialog === null || dialog.wholeStack ? undefined : getBranch(dialog.branchName)
  const baselineBranch = branchSyncStore.getState().baselineBranch ?? ""
  return {
    repositoryPath,
    selected: true,
    items,
    menus,
    createPullRequest: dialogBranch === undefined ? null : getCreatePullRequestContext(dialogBranch),
    wholeStack: wholeStackScopeOf(chain, baselineBranch),
    commitMenus,
    // the check of the reviewed branch answers while the window is open: its groups follow it
    groups: chain.length === 0 ? null : knownReviewGroupsOf(chain.at(-1)!),
  }
}

/** Groups of a menu for the window: its items without a submenu (a submenu stays in the row's menu), as plain data */
function toMenuGroups(groups: MenuItemGroups): BranchReviewMenuItem[][] {
  return groups.map(group => group.filter(item => item.id !== undefined && item.children === undefined).map(toMenuItem)).filter(group => group.length > 0)
}

/**
 * The menu of a commit of a branch for the review window, from the builder of its row's (`getCommitMenuGroups`) and
 * "Reword…": the forms (Move to…, the squash, the reword in the preview) open at the row (`requestCommitForm`); the
 * copies are the window's own
 */
function getReviewCommitMenu(branch: Branch, commit: SyncedCommit): MenuItemGroups {
  const formOf =
    (form: CommitFormRequest["form"]) =>
    (commits: readonly SyncedCommit[]): void =>
      requestCommitForm({ branchName: branch.name, commitIds: commits.map(it => it.originalHash), form })
  const groups = getCommitMenuGroups(branch.name, [commit], {
    openTarget: formOf("move"),
    openSquash: formOf("squash"),
    removePrefix: commits => void moveCommits(commits, null),
  })
  const reword = getRewordMenuItem(commit, it => formOf("reword")([it as SyncedCommit]))
  return [[reword], ...groups.map(group => group.filter(item => !item.id?.startsWith("copy-")))]
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

/**
 * Tells the review window what changed for the actions of its chain, when it changed since it was told; nothing while
 * it is closed. Main window only: the window reads the heads a sync built from its own session.
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
      const wholeStack = request.wholeStack === true
      dialog = { branchName, wholeStack, release: demandReviewReadiness(wholeStack ? stackReadinessKey(branchName) : branchName) }
    }
    link.broadcast()
    return true
  }
  if ("scope" in request || "review" in request) {
    return moveReview(request, send)
  }
  if ("commitMenuItemId" in request) {
    return runCommitMenuItem(request, send)
  }
  // the review of the whole stack asks for the actions of the whole stack of its root
  const wholeStack = request.wholeStack === true
  const subject = wholeStack ? formatWholeStack(branchName) : branchName
  const menuItem =
    "menuItemId" in request
      ? wholeStack
        ? wholeStackMenu(branchName).find(item => item.id === request.menuItemId)
        : findBranchMenuItem(branchName, request.menuItemId)
      : undefined
  const stackItem = wholeStack && "itemId" in request ? getWholeStackReviewItems(branchName).find(item => item.id === request.itemId) : undefined
  const action =
    "menuItemId" in request
      ? requestAction({ menuLabel: menuItem?.label ?? request.menuItemId }, subject)
      : "itemId" in request
        ? wholeStack || request.itemId === "safe-push-whole-stack" || request.itemId === "push-with-children"
          ? requestAction({ menuLabel: stackItem?.label ?? request.itemId }, subject)
          : requestAction(request.itemId, subject)
        : requestAction(request.dialog === "create" ? "create-pull-request" : request.dialog === "continueOnGitHub" ? "continue-on-github" : "safe-push-without-review", subject)
  // a reason, or what GitHub said
  const refuse = refuserOf(`Cannot ${action}`, send)
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
    const context = wholeStack ? getStackPullRequestContext(branchName) : getCreatePullRequestContext(branch)
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
    return continueOnGitHub(wholeStack ? getStackPullRequestContext(branchName) : getCreatePullRequestContext(branch), request.fields).then(
      () => true,
      (error: unknown) => refuse(error),
    )
  }
  if ("menuItemId" in request && menuItem === undefined) {
    return refuse(`The menu of ${subject} has no such action now`)
  }
  const item =
    "menuItemId" in request
      ? menuItem
      : "itemId" in request
        ? wholeStack
          ? stackItem
          : getBranchReviewItems(branch).find(it => it.id === request.itemId)
        : (getSafePushWithoutReviewItem(branch) ?? undefined)
  if (item === undefined) {
    const itemId = "itemId" in request ? request.itemId : "safe-push-without-review"
    return refuse(itemId === "safe-push-whole-stack" || itemId === "push-with-children" ? `${subject} has no such action now` : unavailableReason(itemId, branch))
  }
  return runItem(item, { branchName, commitIds: [] }, refuse)
})

/** Refuses a request of the window with the reason (or what GitHub said), shown in the window and recorded in Activity */
function refuserOf(title: string, send: (feedback: BranchReviewFeedback) => void): (reason: unknown) => false {
  return reason => {
    send({ kind: "error", title, message: recordError(title, reason) })
    return false
  }
}

/** Runs an item the window asked for as the row would: the main window comes to the front and shows the row; a disabled one is refused */
function runItem(item: MenuItem, reveal: RevealRequest, refuse: (reason: unknown) => false): boolean {
  if (item.disabled === true) {
    return refuse(item.description ?? `${item.label} is not available now`)
  }
  getCurrentWindow()
    .setFocus()
    .catch((error: unknown) => logError("Cannot focus the main window", error))
  requestReveal(reveal)
  item.onSelect(FROM_MENU)
  return true
}

/** Runs an item of the menu of a commit in the review window, as its row's menu would: a form opens at the row */
async function runCommitMenuItem(
  request: BranchReviewRequest & { readonly commitMenuItemId: string; readonly commitId: string },
  send: (feedback: BranchReviewFeedback) => void,
): Promise<boolean> {
  const { repositoryPath, branchName, commitId } = request
  const branch = getBranch(branchName)
  const commit = branch?.commits.find(it => it.hash === commitId)
  const item =
    branch === undefined || commit === undefined
      ? undefined
      : getReviewCommitMenu(branch, commit)
          .flat()
          .find(it => it.id === request.commitMenuItemId)
  const refuse = refuserOf(`Cannot run "${(item?.label ?? request.commitMenuItemId).replace(/…$/, "")}" on ${formatShortHash(commitId)}`, send)
  const notShown = await showRepository(repositoryPath)
  if (notShown !== null) {
    return refuse(notShown)
  }
  if (branch === undefined) {
    return refuse(branchGoneReason(branchName))
  }
  if (item === undefined) {
    return refuse(commit === undefined ? `${branchName} no longer has the commit: a sync rebuilt it` : `The menu of the commit has no such action now`)
  }
  return runItem(item, { branchName, commitIds: [commit!.originalHash] }, refuse)
}

/**
 * Moves the review window up to the whole stack of the chain it shows ("Whole Stack" of its stack bar), or down to a
 * branch of the stack it shows, at a commit of the whole stack: the main window opens the review as its menus would
 */
async function moveReview(
  request: BranchReviewRequest & ({ readonly scope: "wholeStack" } | { readonly review: "branch" } | { readonly review: "commit"; readonly commitKey: string }),
  send: (feedback: BranchReviewFeedback) => void,
): Promise<boolean> {
  const { repositoryPath, branchName } = request
  const shown = link.shown()
  const chain = shown !== null && !shown.wholeStack ? shown.chain : [branchName]
  const root = chain[0] ?? branchName
  const refuse = refuserOf(`Cannot review ${"scope" in request ? formatWholeStack(root) : branchName}`, send)
  const notShown = await showRepository(repositoryPath)
  if (notShown !== null) {
    return refuse(notShown)
  }
  if ("scope" in request) {
    const scope = wholeStackScopeOf(chain, branchSyncStore.getState().baselineBranch ?? "")
    if (scope === null || scope.disabled === true) {
      return refuse(scope?.description ?? `${root} heads no stack beyond ${chain.at(-1)}`)
    }
    if (isForkingStack(getStackBranch(root))) {
      return openWholeStackReviewWindow(root, { from: chain })
    }
    return openBranchReviewWindow(mainLineOf(branchSyncStore.getState(), root).at(-1) ?? root, { range: "all" })
  }
  const branch = getBranch(branchName)
  if (branch === undefined) {
    return refuse(branchGoneReason(branchName))
  }
  const commitKey = request.review === "commit" ? request.commitKey : null
  const commit = commitKey === null ? undefined : branch.commits.find(it => reviewCommitKey({ ...it, subject: it.strippedSubject }) === commitKey)
  return (commit === undefined ? null : reviewBranchCommit(branchName, commit.originalHash, null)) ?? openBranchReviewWindow(branchName)
}

/** The item of the branch's menu the review window asks to run, as the row's menu has it now */
function findBranchMenuItem(branchName: string, id: string): MenuItem | undefined {
  const branch = getBranch(branchName)
  return branch === undefined ? undefined : findMenuItem(getBranchContextMenuItems(branch, revealRow, getOperations()), id)
}

/** Why the main window doesn't offer the action of a request on the branch */
function unavailableReason(itemId: "safe-push" | "push" | "safe-push-without-review", branch: Branch): string {
  switch (itemId) {
    case "push":
      return `${branch.name} has nothing to push`
    case "safe-push":
      return "Patronus is not available for the repository"
    case "safe-push-without-review": {
      // the step says why it offers no start without review: what it waits for, a reason of the branch, or approvals
      // that don't wait for a pull request
      const step = getBranchNextStepOf(branch)
      return step.kind === "pending"
        ? (step.safePushBlocker ?? `The approvals of ${branch.name} are not known yet`)
        : `The approvals of ${branch.name} don't wait for a pull request now`
    }
  }
}
