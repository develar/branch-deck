import { useStore } from "zustand"
import { getBranchCopyMenuItems } from "@/hooks/useBranchCopyActions"
import { getSimpleBranchName } from "@/lib/branch"
import { reapplyBranch } from "@/lib/git/reapply"
import type { MenuItem, MenuItemGroups } from "@/lib/menu"
import { pushUnappliedBranch } from "@/lib/push-run"
import { keptSubject } from "@/lib/review-target"
import type { KeptKind } from "@/lib/review-target"
import { getRowPart } from "@/model/store"
import { presentAction } from "@/presenters/actions"
import { branchSyncStore } from "@/stores/branch-sync"
import { getOperationWords, indicatorsStore } from "@/stores/indicators"
import { openInline } from "@/stores/inline-row-action"
import { getUnappliedBranch, unappliedBranchesStore } from "@/stores/unapplied-branches"

/**
 * The menu of a kept row by the actions Rust offers for it (`domain_policy::kept`): Review Changes (`onReview`, none in
 * its own review), of an unapplied branch Reapply, Push (Force Push when the remote has another history), Safe Push
 * disabled with why, Copy and its delete; of an archived one Copy and its delete. `name` is its full name
 */
export function getKeptBranchMenu(kind: KeptKind, name: string, onReview?: () => void): MenuItemGroups {
  const actions = getRowPart(keptSubject(kind, name), "actions")
  const unapplied = kind === "unapplied" ? getUnappliedBranch(name) : undefined
  const branch = unapplied?.branch ?? getSimpleBranchName(name)
  const words = { ...getOperationWords(), subject: branch }
  const review = onReview === undefined ? null : presentAction(actions, "review-changes", { onSelect: onReview }, words)
  const reapply = presentAction(
    actions,
    "reapply",
    { description: "Puts its commits back on top of your branch, their files too", onSelect: () => void reapplyBranch(name) },
    words,
  )
  const push = presentAction(
    actions,
    "push",
    {
      label: unapplied?.remoteStatus?.action.kind === "force" ? "Force Push Branch" : undefined,
      description: `Keeps a copy on the remote as ${name}`,
      kbds: null,
      onSelect: () => void pushUnappliedBranch(name),
    },
    words,
  )
  const safePush = presentAction(actions, "safe-push", { kbds: null, onSelect: () => {} }, words)
  const remove = presentAction(
    actions,
    "delete-kept-branch",
    {
      label: kind === "unapplied" ? "Delete Unapplied Branch…" : "Delete Archived Branch…",
      onSelect: () => openInline(kind === "unapplied" ? "delete-unapplied" : "delete-archived", name),
    },
    words,
  )
  const present = (items: readonly (MenuItem | null)[]): MenuItem[] => items.filter(it => it !== null)
  const groups = [
    present([review]),
    present([reapply]),
    present([push, safePush]),
    getBranchCopyMenuItems(name, true),
    present(remove === null ? [] : [{ ...remove, color: "destructive" }]),
  ]
  return groups.filter(group => group.length > 0)
}

/** By subject: the menu built last, and the store states it was built from (`getKeptBranchMenuNow`) */
const shownMenus = new Map<string, { sources: readonly unknown[]; groups: MenuItemGroups }>()

/**
 * The menu of the kept branch as the stores are now (`getKeptBranchMenu`), one object while the stores it reads stay: the
 * selector of `useKeptBranchMenu`, so a menu that stays mounted shows what runs now (React Compiler memoizes a builder
 * called in a component on its arguments)
 */
function getKeptBranchMenuNow(kind: KeptKind, name: string, onReview: () => void): MenuItemGroups {
  const key = keptSubject(kind, name)
  const sources = [branchSyncStore.getState(), indicatorsStore.getState(), unappliedBranchesStore.getState(), onReview]
  const shown = shownMenus.get(key)
  if (shown !== undefined && shown.sources.every((source, index) => source === sources[index])) {
    return shown.groups
  }
  const groups = getKeptBranchMenu(kind, name, onReview)
  shownMenus.set(key, { sources, groups })
  return groups
}

/**
 * The menu of the kept row `name` (its full name), rendered again when its actions, the operations or its record change;
 * `onReview` opens its review (keep it stable: a new one is a new menu)
 */
export function useKeptBranchMenu(kind: KeptKind, name: string, onReview: () => void): MenuItemGroups {
  const select = () => getKeptBranchMenuNow(kind, name, onReview)
  useStore(branchSyncStore, select)
  useStore(indicatorsStore, select)
  return useStore(unappliedBranchesStore, select)
}
