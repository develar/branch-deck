import type { PatronusReviewRequirement } from "@/lib/bindings"
import { getBranch } from "@/stores/branch-sync"
import type { BranchReviewGroup } from "@/types/sub-window"

/*
 * The reviewer groups of the last code-owner check of each branch, as the readiness of the session answered it in this
 * window (`applyReadinessFacts`): the review of a branch opens with them, and its "Needs approval" chips follow an
 * answer that comes while it is open.
 */

/** The groups of the last code-owner check of a branch by name, at the head it was checked at */
const lastReviewGroups = new Map<string, { head: string | undefined; groups: BranchReviewGroup[] }>()

export function toReviewGroups(requirement: PatronusReviewRequirement): BranchReviewGroup[] {
  return requirement.groups.map(group => ({ name: group.name, paths: group.paths, branchWide: group.branchWide }))
}

/** Remembers what the code owners of the branch at this head review (the Safe Push popover checked it) */
export function rememberReviewGroups(branchName: string, requirement: PatronusReviewRequirement): void {
  lastReviewGroups.set(branchName, { head: getBranch(branchName)?.commits[0]?.hash, groups: toReviewGroups(requirement) })
}

/** The groups of the last code-owner check of the branch at its head; none while none answered for it */
export function reviewGroupsOf(branchName: string): BranchReviewGroup[] {
  return knownReviewGroupsOf(branchName) ?? []
}

/** The groups of the last code-owner check of the branch at its head; null while none answered for it */
export function knownReviewGroupsOf(branchName: string): BranchReviewGroup[] | null {
  const last = lastReviewGroups.get(branchName)
  return last !== undefined && last.head === getBranch(branchName)?.commits[0]?.hash ? last.groups : null
}
