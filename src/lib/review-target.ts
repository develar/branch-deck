import type { ReviewSegment, ReviewTarget, TargetReview } from "@/lib/bindings"

/** The target of the review of a chain: the branch with its stack ancestors, or the whole stack of the root it names alone */
export function chainTarget(chain: readonly string[], wholeStack: boolean): ReviewTarget {
  return wholeStack ? { kind: "wholeStack", root: chain[0] ?? "" } : { kind: "branch", chain: [...chain] }
}

/** The parts of a range a review answered; none for a list of commits */
export function segmentsOf(review: TargetReview): ReviewSegment[] {
  return review.kind === "range" ? review.segments : []
}

/** Where a branch is kept outside HEAD: an unapply took it out (`{prefix}/unapplied/<name>`), a sync archived it */
export type KeptKind = "unapplied" | "archived"

/** The subject of the actions of a kept branch in the view (`unapplied:<full name>`, `archived:<full name>`) */
export function keptSubject(kind: KeptKind, name: string): string {
  return `${kind}:${name}`
}
