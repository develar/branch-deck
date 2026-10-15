import type { PatronusReviewRequirement } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { stackAncestors } from "@/lib/stack"
import { openSubWindow } from "@/lib/window-management"
import { branchSyncStore, getBranch } from "@/stores/branch-sync"
import { getEffectiveBranchPrefix, getSelectedProject } from "@/stores/repository"
import type { BranchReviewGroup, BranchReviewWindowData } from "@/types/sub-window"

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
  const data: BranchReviewWindowData = {
    repositoryPath,
    branchPrefix: getEffectiveBranchPrefix(),
    baselineBranch,
    chain: [...stackAncestors(branchName, getBranch).map(branch => branch.name), branchName],
    range: focus.range ?? "own",
    groups,
    group: focus.group ?? null,
    path: focus.path ?? null,
    openedAt: Date.now(),
  }
  try {
    await openSubWindow({ windowId: BRANCH_REVIEW_WINDOW, url: "/branch-review", title: `Review ${branchName}`, data })
  }
  catch (error) {
    notifyError(`Cannot open the review of ${branchName}`, error)
  }
}
