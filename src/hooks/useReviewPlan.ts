import { useEffect, useState } from "react"
import type { ReviewPlan, ReviewSubject } from "@/lib/bindings"
import { commands } from "@/lib/bindings"

/*
 * The review plan of a review window (spec/branch/review-plan.spec.md, Status: Experimental): `get_review_plan` of the
 * shown part, read again when the part, its head or "Since Your Review" change, and after Mark Reviewed. One plan of
 * the window is in flight: Rust supersedes the older one, which answers `canceled` and is dropped here.
 */

/** The plan of the review and its read; the plan of the same branch stays while the next one is read */
export interface ReviewPlanState {
  readonly plan: ReviewPlan | null
  /** A read is in flight */
  readonly loading: boolean
  /** Why the last read failed; null after an answer */
  readonly error: string | null
}

/** What the plan is read for; null reads none */
export interface ReviewPlanRequest {
  readonly repositoryPath: string
  readonly subject: ReviewSubject
  /** Only the commits added or rewritten since the review of the branch */
  readonly sinceReviewed: boolean
  /** The head of the shown part: a sync that moved it reads the plan again */
  readonly head: string
  /** Bumped by Mark Reviewed: the interdiff changed */
  readonly generation: number
}

interface Answer {
  /** The subject the answer is of, without `sinceReviewed`: a plan of the same branch stays while the next is read */
  readonly subjectKey: string
  readonly requestKey: string
  readonly plan: ReviewPlan | null
  readonly error: string | null
}

const NO_PLAN: ReviewPlanState = { plan: null, loading: false, error: null }

export function useReviewPlan(request: ReviewPlanRequest | null): ReviewPlanState {
  const subjectKey = request === null ? null : JSON.stringify([request.repositoryPath, request.subject])
  const requestKey = request === null ? null : JSON.stringify([subjectKey, request.sinceReviewed, request.head, request.generation])
  const [answer, setAnswer] = useState<Answer | null>(null)
  useEffect(() => {
    if (requestKey === null || subjectKey === null) {
      return
    }
    const [repositoryPath, subject] = JSON.parse(subjectKey) as [string, ReviewSubject]
    const [, sinceReviewed] = JSON.parse(requestKey) as [string, boolean]
    let current = true
    void commands.getReviewPlan({ repositoryPath, subject, sinceReviewed }).then(result => {
      // a newer read of the window superseded this one: its answer is `canceled`, the newer one comes
      if (!current || (result.status === "error" && result.error.code === "canceled")) {
        return
      }
      setAnswer(result.status === "ok" ? { subjectKey, requestKey, plan: result.data, error: null } : { subjectKey, requestKey, plan: null, error: result.error.message })
    })
    return () => {
      current = false
    }
  }, [subjectKey, requestKey])
  if (request === null || answer === null || answer.subjectKey !== subjectKey) {
    return request === null ? NO_PLAN : { plan: null, loading: true, error: null }
  }
  return { plan: answer.plan, loading: answer.requestKey !== requestKey, error: answer.error }
}
