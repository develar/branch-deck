import type { PatronusReviewRequirement, ReviewReadiness } from "@/lib/bindings"
import { useDemandReviewReadiness } from "@/model/demand"
import { freshReadiness, getReviewReadiness, recheckReviewReadiness, useReviewReadiness } from "@/stores/review-readiness"

export type ReviewRequirementState =
  | { status: "loading" }
  | { status: "loaded"; requirement: PatronusReviewRequirement; readiness: ReviewReadiness }
  | { status: "error"; message: string }

export interface ReviewRequirement {
  state: ReviewRequirementState
  /** Check Approvals Again: the pull request from GitHub, then Patronus, not the kept answer */
  recheck: () => void
}

/**
 * The code-owner review a Safe Push of the branch needs, from the one reader of the readiness (`review-readiness`),
 * asked first while `enabled` (the Safe Push popover, never for a dry run; the "Create pull request" dialog) and again when
 * its input changes: the tip of the branch (an unstack leaves only its own commits), the start, a review of the pull
 * request the robot would start from. Patronus judges the approvals; the backend keeps its answer by what it decides on.
 * An answer of an earlier input is loading here: the form acts only on the answer for the branch as it is.
 */
export function usePatronusReviewRequirement(branchName: string, enabled: boolean): ReviewRequirement {
  useDemandReviewReadiness(branchName, enabled)
  const state = reviewRequirementStateOf(freshReadiness(useReviewReadiness(branchName), branchName))
  return { state, recheck: () => void recheckReviewReadiness(branchName) }
}

/** Non-hook variant of the state of `usePatronusReviewRequirement`, from the store as it is (no demand) */
export function getReviewRequirementState(branchName: string): ReviewRequirementState {
  return reviewRequirementStateOf(freshReadiness(getReviewReadiness(branchName), branchName))
}

function reviewRequirementStateOf(entry: ReturnType<typeof freshReadiness>): ReviewRequirementState {
  if (entry === null) {
    return { status: "loading" }
  }
  if (entry.requirement === null) {
    return { status: "error", message: entry.readiness.kind === "unavailable" ? entry.readiness.message : "Patronus gave no answer" }
  }
  return { status: "loaded", requirement: entry.requirement, readiness: entry.readiness }
}
