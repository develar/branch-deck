import { useStore } from "zustand"
import { getBranchReviewItems, menuSources, useMenuStores } from "@/hooks/useBranchContextActions"
import type { BranchActionItem } from "@/hooks/useBranchContextActions"
import type { MenuItem, MenuItemGroups } from "@/lib/menu"
import { repositoryName } from "@/lib/repository-config"
import {
  branchReviewMenu,
  getReviewCommitMenu,
  getWholeStackReviewItems,
  isCommitCopyItem,
  keptReviewItems,
  keptReviewMenu,
  reviewActionSubject,
  wholeStackMenu,
  wholeStackScopeOf,
} from "@/lib/review-actions"
import { knownReviewGroupsOf } from "@/lib/review-groups"
import { askReviewRun, opensRowForm } from "@/lib/review-runs"
import { archivedBranchesStore } from "@/stores/archived-branches"
import { getBranch, getStackBranch } from "@/stores/branch-sync"
import { getRecentProjects } from "@/stores/settings/repositorySettings"
import { unappliedBranchesStore } from "@/stores/unapplied-branches"
import type { BranchReviewGroup, KeptReview, WholeStackScope } from "@/types/sub-window"

/*
 * The actions of the review on screen, which the review window builds from its own session by the builders of the rows
 * (spec/review/review-window.spec.md#actions): it runs each itself, but one that opens a form under the row of its
 * subject (`opensRowForm`: Rust's `ROW_FORM_ACTIONS`), which it asks the main window to run there (`review/run`).
 */

/** What a review of a branch shows, of which it builds the actions */
export interface ReviewedSubject {
  readonly repositoryPath: string
  readonly baselineBranch: string
  readonly chain: readonly string[]
  readonly wholeStack: boolean
  readonly kept: KeptReview | undefined
}

/** An action of the title bar of a review: an item of the builders of the row, by its id */
export type ReviewActionItem = BranchActionItem & { readonly id: string }

/** The items of the title bar as the window runs them: each by its id (`routed`) */
function titleItems(items: readonly BranchActionItem[], run: { repositoryPath: string; subject: string }): ReviewActionItem[] {
  return items.flatMap(item => (item.id === undefined ? [] : [{ ...routed(item, run, byRowForm(item)), id: item.id }]))
}

/** The actions of the review on screen, by the branch of the chain they act on */
export interface ReviewActions {
  /** The actions of the title bar: of each branch of the chain, of a whole stack under its root, of a kept branch under the branch it was */
  readonly items: Readonly<Record<string, readonly ReviewActionItem[]>>
  /** The "⋯" of each: the whole menu of its row, without a submenu */
  readonly menus: Readonly<Record<string, MenuItemGroups>>
  /** The menu of each commit of the chain, by its id in the virtual branch */
  readonly commitMenus: Readonly<Record<string, MenuItemGroups>>
  /** "Whole Stack" after "From Root" in the stack bar; null where it would show nothing more */
  readonly wholeStack: WholeStackScope | null
  /** The reviewer groups of the last code-owner check of the reviewed branch at its head; null while none answered */
  readonly groups: readonly BranchReviewGroup[] | null
}

/**
 * The item as the review window runs it: itself, or one that opens a form under its row by the main window, at that
 * row (the hover says that the main window switches to the repository of the review first, while it shows another)
 */
function routed(item: MenuItem, run: { repositoryPath: string; subject: string; commitIds?: readonly string[] }, atRow: boolean): MenuItem {
  if (!atRow) {
    return item
  }
  const switches = getRecentProjects()[0]?.path !== run.repositoryPath && item.disabled !== true && item.running !== true
  return {
    ...item,
    ...(switches && { description: `Switches the main window to ${repositoryName(run.repositoryPath)}` }),
    onSelect: () =>
      void askReviewRun({
        repositoryPath: run.repositoryPath,
        subject: run.subject,
        itemId: item.id ?? item.label,
        ...(run.commitIds !== undefined && { commitIds: run.commitIds }),
      }),
  }
}

/** The groups of a menu as the review window runs them (`routed`): without key hints, the keys are the row's */
function routedGroups(groups: MenuItemGroups, run: { repositoryPath: string; subject: string; commitIds?: readonly string[] }, atRow: (item: MenuItem) => boolean): MenuItemGroups {
  return groups.map(group => group.map(item => ({ ...routed(item, run, atRow(item)), kbds: undefined })))
}

const byRowForm = (item: MenuItem): boolean => opensRowForm(item.id)

/** The actions of the review of `subject` as the stores of the window's session are now */
function buildReviewActions(subject: ReviewedSubject): ReviewActions {
  const { repositoryPath, chain, wholeStack, kept } = subject
  const root = chain[0] ?? ""
  if (kept !== undefined) {
    const run = { repositoryPath, subject: reviewActionSubject(root, { wholeStack: false, kept }) }
    return {
      items: { [root]: titleItems(keptReviewItems(kept), run) },
      menus: { [root]: routedGroups(keptReviewMenu(kept), run, byRowForm) },
      commitMenus: {},
      wholeStack: null,
      groups: null,
    }
  }
  if (wholeStack) {
    if (getStackBranch(root) === undefined) {
      return { items: {}, menus: {}, commitMenus: {}, wholeStack: null, groups: null }
    }
    const run = { repositoryPath, subject: reviewActionSubject(root, { wholeStack: true }) }
    return {
      items: { [root]: titleItems(getWholeStackReviewItems(root), run) },
      menus: { [root]: routedGroups([wholeStackMenu(root)], run, byRowForm) },
      commitMenus: {},
      wholeStack: null,
      groups: null,
    }
  }
  const items: Record<string, ReviewActionItem[]> = {}
  const menus: Record<string, MenuItemGroups> = {}
  const commitMenus: Record<string, MenuItemGroups> = {}
  for (const name of chain) {
    const branch = getBranch(name)
    if (branch === undefined) {
      continue
    }
    const run = { repositoryPath, subject: name }
    items[name] = titleItems(getBranchReviewItems(branch), run)
    menus[name] = routedGroups(branchReviewMenu(branch), run, byRowForm)
    for (const commit of branch.commits) {
      if (commit.hash !== undefined) {
        // a commit's forms and rewrites are its row's; its copies the window's
        commitMenus[commit.hash] = routedGroups(getReviewCommitMenu(branch, commit), { ...run, commitIds: [commit.hash] }, item => !isCommitCopyItem(item.id))
      }
    }
  }
  return {
    items,
    menus,
    commitMenus,
    wholeStack: wholeStackScopeOf(chain, subject.baselineBranch),
    // the check of the reviewed branch answers while the window is open: its groups follow it
    groups: chain.length === 0 ? null : knownReviewGroupsOf(chain.at(-1)!),
  }
}

/** The actions built last, by the subject, and the store states they were built from (`getReviewActionsNow`) */
let built: { key: string; sources: readonly unknown[]; actions: ReviewActions } | null = null

/**
 * The actions of the review of `subject` as the stores are now, one object while the stores they read stay: the
 * selector of `useReviewActions` (React Compiler memoizes a builder called in a component on its arguments, so the
 * actions would show their first state)
 */
function getReviewActionsNow(subject: ReviewedSubject): ReviewActions {
  const key = JSON.stringify([subject.repositoryPath, subject.baselineBranch, subject.chain, subject.wholeStack, subject.kept ?? null])
  const sources = [...menuSources(), unappliedBranchesStore.getState(), archivedBranchesStore.getState()]
  if (built !== null && built.key === key && built.sources.length === sources.length && built.sources.every((source, index) => source === sources[index])) {
    return built.actions
  }
  built = { key, sources, actions: buildReviewActions(subject) }
  return built.actions
}

/** The actions of the review of `subject`, built again when a store they read changes */
export function useReviewActions(subject: ReviewedSubject): ReviewActions {
  const select = () => getReviewActionsNow(subject)
  useStore(unappliedBranchesStore, select)
  useStore(archivedBranchesStore, select)
  return useMenuStores(select)
}
