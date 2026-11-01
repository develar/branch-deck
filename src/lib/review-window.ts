import type { BranchReviewSource } from "@/hooks/useBranchReview"
import type { ReviewCommit, ReviewPart } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { reviewBlockerOf } from "@/lib/branch-review"
import type { BranchReviewFocus } from "@/lib/branch-review"
import { logError, notifyError } from "@/lib/errors"
import { keptBranchName } from "@/lib/review-actions"
import { reviewGroupsOf } from "@/lib/review-groups"
import { reviewSubjectName, reviewWindowTitle } from "@/lib/review-reuse"
import { chainTarget } from "@/lib/review-target"
import { REVIEW_WINDOW } from "@/lib/window-label"
import { openSubWindow } from "@/lib/window-management"
import { branchSyncStore, getBranch, getStackAncestors, mainLineOf } from "@/stores/branch-sync"
import { getDiffRepositoryPath, getSelectedProject } from "@/stores/repository"
import { getUnappliedBranch } from "@/stores/unapplied-branches"
import type { BranchReviewGroup, BranchReviewWindowData, CommitsReviewWindowData, KeptReview, ReviewedBranch, ReviewWindowData } from "@/types/sub-window"

/*
 * The openers of the one review window (spec/review/review-window.spec.md#one-window): each says what to show and opens
 * the window on it, from any window (the review window opens the next review itself: a branch of its stack, the whole
 * stack, the branch a reapplied kept branch is again). The page of the window decides how the open goes on from the
 * review on screen (`planReviewOpen`): it gets every open.
 */

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
    chain: [...getStackAncestors(branchName), branchName],
    groups: focus.groups ?? reviewGroupsOf(branchName),
  }
}

/** The part a review of a range shows first, when the backend can read it ahead: the own part or the whole stack */
function prefetchedPart(range: BranchReviewFocus["range"]): ReviewPart | null {
  return range === undefined || range === "own" ? "own" : range === "all" ? "all" : null
}

/** What was read ahead, by the repository, the chain, its heads and the part: once per state of the branch */
const prefetched = new Set<string>()

/** Reads `review` ahead (`prefetch_review`) once per `key`, beside the start of the window */
function prefetchOnce(key: string, review: Parameters<typeof commands.prefetchReview>[0], what: string): void {
  if (prefetched.has(key)) {
    return
  }
  prefetched.add(key)
  void commands.prefetchReview(review).then(result => {
    if (result.status === "error") {
      prefetched.delete(key)
      logError(`Cannot read the review of ${what} ahead`, result.error)
    }
  })
}

/**
 * Reads ahead what the review window of a branch asks for first (`prefetch_review`: its parts, the files and the patch
 * of the part it shows) as the window opens, beside its start, once per state of the branch. A hover or a focus of its
 * row reads nothing: a pointer that passes over the table costs no IPC and no git
 */
function prefetchReview(source: BranchReviewSource, focus: BranchReviewFocus): void {
  const part = prefetchedPart(focus.range)
  if (part === null) {
    return
  }
  const { repositoryPath, baselineBranch, chain } = source
  const key = [repositoryPath, part, ...chain.map(name => `${name}@${getBranch(name)?.commits[0]?.hash ?? ""}`)].join("\n")
  prefetchOnce(key, { review: { repositoryPath, baselineBranch, target: chainTarget(chain, false) }, part }, "a branch")
}

/** The focus a review starts at, of what an open asks (`focus`): the own part, every file, no commit, the first file */
function startFocus(focus: BranchReviewFocus): BranchReviewWindowData["focus"] {
  return { range: focus.range ?? "own", filter: focus.filter ?? { kind: "all" }, commitId: focus.commitId ?? null, path: focus.path ?? null }
}

/**
 * Reviews a branch in the review window, the only review of a branch: its stack from the root, the reviewer groups the
 * code owners check asked for, from `focus` (the part, the filter, the commit, the file). Resolves to whether the
 * window opened; a failure is told.
 */
export async function openBranchReviewWindow(branchName: string, focus: BranchReviewFocus = {}): Promise<boolean> {
  const source = getBranchReviewSource(branchName, focus)
  if (source === null) {
    return false
  }
  // git reads while the window starts
  prefetchReview(source, focus)
  return openWindow({ kind: "branch", ...source, chain: [...source.chain], groups: [...source.groups], branchName, asked: focus, focus: startFocus(focus), openedAt: Date.now() })
}

/** Opens the window on `data`; a failure is told, of what it reviews */
function openWindow(data: ReviewWindowData): Promise<boolean> {
  return openSubWindow({ windowId: REVIEW_WINDOW, url: "/review", title: reviewWindowTitle(data), data }).then(
    () => true,
    (error: unknown) => {
      notifyError(`Cannot open the review of ${reviewSubjectName(data)}`, error)
      return false
    },
  )
}

/**
 * Reviews a commit of a branch in the review window of the branch, at the commit (and `path`): the stack, its actions
 * and the commits of the branch beside it. Null when the branch review can't show it: the branch is not built on its
 * parents (blocked, failed, in conflict) or its virtual branch doesn't have the commit yet; the review of its list shows
 * it then.
 */
export function reviewBranchCommit(branchName: string, commitHash: string, path: string | null): Promise<boolean> | null {
  const branch = getBranch(branchName)
  if (branch === undefined || reviewBlockerOf(branch) !== null || branch.status === "MergeConflict" || branch.status === "AnalyzingConflict") {
    return null
  }
  const copy = branch.commits.find(it => it.originalHash === commitHash)
  if (copy?.hash === undefined || copy.status === "Error" || copy.status === "Blocked") {
    return null
  }
  return openBranchReviewWindow(branchName, { commitId: copy.hash, path })
}

/**
 * Reviews a branch kept outside HEAD (unapplied, archived) in the review window: its own commits from where they start
 * (`ReviewTarget::Kept`), one part, at `focus` (a commit, a file); its title bar holds its actions (Reapply, Push Branch),
 * its "⋯" its whole menu. Resolves to whether the window opened; a failure is told.
 */
export async function openKeptReviewWindow(kept: KeptReview, focus: Pick<BranchReviewFocus, "commitId" | "path"> = {}): Promise<boolean> {
  const repositoryPath = getSelectedProject()?.path
  const baselineBranch = branchSyncStore.getState().baselineBranch
  if (repositoryPath === undefined || baselineBranch === null) {
    return false
  }
  const branchName = keptBranchName(kept)
  const key = [repositoryPath, kept.name, getUnappliedBranch(kept.name)?.tip ?? ""].join("\n")
  prefetchOnce(key, { review: { repositoryPath, baselineBranch, target: { kind: "kept", name: kept.name } }, part: "own" }, "a kept branch")
  return openWindow({
    kind: "branch",
    repositoryPath,
    baselineBranch,
    chain: [branchName],
    groups: [],
    kept,
    branchName,
    asked: focus,
    focus: startFocus(focus),
    openedAt: Date.now(),
  })
}

/** By kind and full name: the open of the review of a kept branch, one function per row for its menu (`useKeptBranchMenu`) */
const keptOpeners = new Map<string, () => void>()

/** The open of the review of the kept branch `name` (its full name), the same function for the same branch */
export function keptReviewOpener(kind: KeptReview["kind"], name: string): () => void {
  const key = `${kind}:${name}`
  let open = keptOpeners.get(key)
  if (open === undefined) {
    open = () => void openKeptReviewWindow({ kind, name })
    keptOpeners.set(key, open)
  }
  return open
}

/**
 * Reviews the whole stack of `root` in the review window: its stack branch `{prefix}/stack/<root>`, every commit of the
 * stack on the baseline as one part ("Review Whole Stack"), each commit named by its branch; the segments of `from`
 * (the chain the window showed, else the main line) go back to a branch; its title bar holds the actions of the whole
 * stack (`getWholeStackReviewItems`), its "⋯" all of them. `groups`: the reviewer groups of the code-owner check of the
 * whole stack (the Safe Push popover), a place: the window starts over with them. Resolves to whether the window
 * opened; a failure is told.
 */
export async function openWholeStackReviewWindow(root: string, options: { readonly from?: readonly string[]; readonly groups?: BranchReviewGroup[] } = {}): Promise<boolean> {
  const from = options.from ?? mainLineOf(branchSyncStore.getState(), root)
  const source = getBranchReviewSource(root, {})
  if (source === null) {
    return false
  }
  return openWindow({
    kind: "branch",
    ...source,
    chain: [root],
    groups: options.groups ?? [],
    wholeStack: true,
    from: from.map((name): ReviewedBranch => ({ name, commits: getBranch(name)?.commitCount ?? 0 })),
    branchName: root,
    asked: options.groups === undefined ? {} : { groups: options.groups },
    focus: startFocus({}),
    openedAt: Date.now(),
  })
}

/**
 * Reviews `commits[index]` of a list (named `listLabel`) in the review window, at `path` (the first file when null):
 * the commits of the list above its files, ⌥↑/⌥↓ step through them; a commit of the list on screen moves there. From
 * any window: the missing commits of a conflict are a list in a sub-window. Resolves to whether the window opened; a
 * failure is told.
 */
export async function openCommitReviewWindow(commits: readonly ReviewCommit[], listLabel: string | null, index: number, path: string | null): Promise<boolean> {
  const repositoryPath = getDiffRepositoryPath()
  if (repositoryPath === undefined || commits[index] === undefined) {
    return false
  }
  const data: CommitsReviewWindowData = { kind: "commits", repositoryPath, commits: [...commits], listLabel, index, path, openedAt: Date.now() }
  return openWindow(data)
}
