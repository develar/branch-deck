import { actionsOf, getBranchContextMenuItems, getBranchReviewItems, getSafePushWithoutReviewItem } from "@/hooks/useBranchContextActions"
import type { BranchActionItem } from "@/hooks/useBranchContextActions"
import { getCommitMenuGroups } from "@/hooks/useCommitMoveList"
import { getKeptBranchMenu } from "@/hooks/useKeptBranchMenu"
import { moveCommits } from "@/hooks/useMoveCommits"
import { getRewordMenuItem } from "@/hooks/useRewordCommit"
import { wholeStackRestOf } from "@/hooks/useStackNextStep"
import { getWholeStackMenuItems } from "@/hooks/useWholeStackActions"
import { formatBranchesAndCommits, getSimpleBranchName } from "@/lib/branch"
import type { MenuItem, MenuItemGroups } from "@/lib/menu"
import { findMenuItem } from "@/lib/menu"
import { keptSubject } from "@/lib/review-target"
import { formatWholeStack, stackReadinessKey } from "@/lib/whole-stack"
import { actionBlocker } from "@/presenters/actions"
import { branchSyncStore, getBranch, getStackBranch, getStackDescendants, isForkingRoot, mainLineOf } from "@/stores/branch-sync"
import type { Branch, SyncedCommit } from "@/stores/branch-sync"
import { requestCommitForm, requestReveal } from "@/stores/commit-move"
import type { CommitFormRequest } from "@/stores/commit-move"
import { getOperationWords } from "@/stores/indicators"
import { getStackPullRequest } from "@/stores/pull-requests"
import { getUnappliedBranch } from "@/stores/unapplied-branches"
import type { KeptReview, WholeStackScope } from "@/types/sub-window"

/*
 * The actions of what a review shows, from the builders of the rows (spec/review/review-window.spec.md#actions): of a
 * branch its title bar (`getBranchReviewItems`) and its menu, of a whole stack the actions of its island, of a kept
 * branch its menu, of a commit of a branch the menu of its row. The review window builds them from its own session
 * (`useReviewActions`); the main window runs those that open a form under their row by the same builders
 * (`src/lib/review-runs.ts`), so both find an item by its id.
 */

/** The subject of the actions of a review: a branch by name, the whole stack of a root (`stack:<root>`), a kept branch (`unapplied:<full>`) */
export function reviewActionSubject(chainEnd: string, { wholeStack, kept }: { readonly wholeStack: boolean; readonly kept?: KeptReview }): string {
  return kept !== undefined ? keptSubject(kept.kind, kept.name) : wholeStack ? `stack:${chainEnd}` : chainEnd
}

/** The kept branch of a subject of `reviewActionSubject`; null for a branch or a whole stack */
export function keptOfSubject(subject: string): KeptReview | null {
  for (const kind of ["unapplied", "archived"] as const) {
    if (subject.startsWith(`${kind}:`)) {
      return { kind, name: subject.slice(kind.length + 1) }
    }
  }
  return null
}

/** The branch a kept branch was, as its row names it: `docs` of `jane/unapplied/docs` */
export function keptBranchName(kept: KeptReview): string {
  return (kept.kind === "unapplied" ? getUnappliedBranch(kept.name)?.branch : undefined) ?? getSimpleBranchName(kept.name)
}

/** The items of a menu for the review: without a submenu (it stays in the row's menu) and without an id-less item */
export function reviewMenuGroups(groups: MenuItemGroups): MenuItemGroups {
  return groups.map(group => group.filter(item => item.id !== undefined && item.children === undefined)).filter(group => group.length > 0)
}

/**
 * The menu of the branch for the "⋯" of its review: its row's, without the review it is and the view of the table (the
 * collapse of its stack); the forms of the row open there
 */
export function branchReviewMenu(branch: Branch): MenuItemGroups {
  return reviewMenuGroups(getBranchContextMenuItems(branch, revealRow)).map(group => group.filter(item => !TABLE_VIEW_ITEMS.has(item.id!) && item.id !== "review-changes"))
}

/** The items of the branch menu that change the view of the table, not the branch */
const TABLE_VIEW_ITEMS: ReadonlySet<string> = new Set(["expand-stack", "collapse-stack"])

/** The row of the branch expands for an item that works on its commits (Amend, Unapply), as the reveal of a move does */
export function revealRow(name: string, expanded: boolean): void {
  if (expanded) {
    requestReveal({ branchName: name, commitIds: [] })
  }
}

/** The actions of the kept branch in the title bar of its review: Reapply and its push, without keys */
const KEPT_REVIEW_ITEMS: readonly string[] = ["reapply", "push"]

/** The menu of the kept branch for the "⋯" of its review: without its review, which the window is */
export function keptReviewMenu(kept: KeptReview): MenuItemGroups {
  return reviewMenuGroups(getKeptBranchMenu(kept.kind, kept.name))
}

/** The actions of the kept branch in the title bar of its review */
export function keptReviewItems(kept: KeptReview): BranchActionItem[] {
  return keptReviewMenu(kept)
    .flat()
    .filter(item => KEPT_REVIEW_ITEMS.includes(item.id ?? ""))
    .map(item => ({ ...item, kbds: undefined }))
}

/** The whole menu of the whole stack for the "⋯" of its review: without its review, which the window is */
export function wholeStackMenu(root: string): MenuItem[] {
  return getWholeStackMenuItems(root, { withPush: true, landInPieces: true }).filter(
    item => item.id !== undefined && item.id !== "review-whole-stack" && item.children === undefined,
  )
}

/**
 * The actions of the whole stack in the title bar of its review, as a branch's: O its pull request (opened by the
 * window) or "Create Pull Request of the Stack…" (its dialog in the window), S "Safe Push Whole Stack" (Push to master,
 * Merge #7 of the pull request of its stack branch by the route of the repository), Push with Children of the root (no
 * key: P is the previous change)
 */
export function getWholeStackReviewItems(root: string): BranchActionItem[] {
  const pullRequest = getStackPullRequest(root)
  const keys: Readonly<Record<string, string[]>> = { "open-stack-pull-request": ["o"], "safe-push-whole-stack": ["s"], "push-to-baseline": ["s"], "merge-pull-request": ["s"] }
  // in the order of a branch's: its pull request, its Safe Push (or Push to master, or Merge #7), its push
  const order = ["open-stack-pull-request", "create-stack-pull-request", "safe-push-whole-stack", "push-to-baseline", "merge-pull-request", "push-with-children"]
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
 * root, which holds them all. Disabled with the reason while that review can't open, as Rust decides "Review Whole
 * Stack" (`review-whole-stack` of `stack:<root>`: its stack branch not built, of a straight chain the review of its
 * last branch)
 */
export function wholeStackScopeOf(chain: readonly string[], baselineBranch: string): WholeStackScope | null {
  const root = chain[0]
  const last = chain.at(-1)
  if (root === undefined || last === undefined) {
    return null
  }
  const blocker = actionBlocker(actionsOf(stackReadinessKey(root)), "review-whole-stack", { ...getOperationWords(), subject: root }) ?? null
  if (getStackBranch(root) !== undefined && isForkingRoot(branchSyncStore.getState(), root)) {
    if (blocker !== null) {
      return { root, disabled: true, description: blocker }
    }
    const rest = wholeStackRestOf(root)
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
  const commits = mainLine.reduce((sum, name) => sum + (getBranch(name)?.commitCount ?? 0), 0)
  return {
    root,
    ...(blocker !== null && { disabled: true }),
    description: blocker ?? `${root} to ${leaf} on ${baselineBranch}: ${formatBranchesAndCommits(mainLine.length, commits)}, what the Safe Push of ${leaf} lands`,
  }
}

/**
 * The menu of a commit of a branch for the review, from the builder of its row's (`getCommitMenuGroups`) and
 * "Reword…": the forms (Move to…, the squash, the reword in the preview) open at the row (`requestCommitForm`), the
 * copies are the window's own
 */
export function getReviewCommitMenu(branch: Branch, commit: SyncedCommit): MenuItemGroups {
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
  return reviewMenuGroups([[reword], ...groups.map(group => group.filter(item => !item.id?.startsWith("copy-")))])
}

/** Whether an item of the menu of a commit runs in the window: the copies; the rest is a form or a rewrite of its row */
export function isCommitCopyItem(id: string | undefined): boolean {
  return id?.startsWith("copy-") === true
}

/**
 * The item `itemId` of the subject of a review as the builders offer it now (`reviewActionSubject`; with `commitId` an
 * item of the menu of that commit of the branch, by its id in the virtual branch): what the main window runs for a
 * review window; undefined while the subject or the item is not there
 */
export function findReviewItem(subject: string, itemId: string, commitId: string | null): MenuItem | undefined {
  const kept = keptOfSubject(subject)
  if (kept !== null) {
    return findMenuItem(getKeptBranchMenu(kept.kind, kept.name), itemId)
  }
  if (subject.startsWith("stack:")) {
    const root = subject.slice("stack:".length)
    return [...getWholeStackReviewItems(root), ...wholeStackMenu(root)].find(item => item.id === itemId)
  }
  const branch = getBranch(subject)
  if (branch === undefined) {
    return undefined
  }
  if (commitId !== null) {
    const commit = branch.commits.find(it => it.hash === commitId)
    return commit === undefined ? undefined : findMenuItem(getReviewCommitMenu(branch, commit), itemId)
  }
  return (
    findMenuItem(getBranchContextMenuItems(branch, revealRow), itemId) ??
    getBranchReviewItems(branch).find(item => item.id === itemId) ??
    (itemId === "safe-push-without-review" ? (getSafePushWithoutReviewItem(branch) ?? undefined) : undefined)
  )
}
