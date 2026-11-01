import type { ReviewSegment, ReviewTarget, TargetReview } from "@/lib/bindings"

/** The target of the review of a chain: the branch with its stack ancestors, or the whole stack of the root it names alone */
export function chainTarget(chain: readonly string[], wholeStack: boolean): ReviewTarget {
  return wholeStack ? { kind: "wholeStack", root: chain[0] ?? "" } : { kind: "branch", chain: [...chain] }
}

/** The parts of a range a review answered; none for a list of commits */
export function segmentsOf(review: TargetReview): ReviewSegment[] {
  return review.kind === "range" ? review.segments : []
}
