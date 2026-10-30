import { useEffect, useState } from "react"
import { Channel } from "@tauri-apps/api/core"
import type { ReviewPlan, ReviewPlanEvent, ReviewSubject } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import type { DiffSubject } from "@/lib/commit-files"
import type { ReviewFacts } from "@/presenters/review-plan"

/*
 * The review plan of a review window (spec/branch/review-plan.spec.md, Status: Experimental): `get_review_plan` of the
 * shown part, read again when the part, its head or "Since Your Review" change, and after Mark Reviewed. Rust sends the
 * plan without the usages while it asks the index, then with them, or `disabled`: each event replaces what the hook
 * holds. One plan of the window is in flight: Rust supersedes the older one, which answers `canceled` and is dropped
 * here.
 */

/** The plan of the review and its read; the plan of the same branch stays while the next one is read */
export interface ReviewPlanState {
  readonly plan: ReviewPlan | null
  /** A read is in flight */
  readonly loading: boolean
  /** The plan has no usages yet: Rust asks the index, the plan with them follows */
  readonly pending: boolean
  /** The repository turns the review plan off: the window shows none */
  readonly disabled: boolean
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
  readonly pending: boolean
  readonly disabled: boolean
  /** The command answered: no other event of it follows */
  readonly done: boolean
  readonly error: string | null
}

const NO_PLAN: ReviewPlanState = { plan: null, loading: false, pending: false, disabled: false, error: null }

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
    const events = new Channel<ReviewPlanEvent>()
    events.onmessage = event => {
      if (current) {
        const disabled = event.kind === "disabled"
        setAnswer({ subjectKey, requestKey, plan: disabled ? null : event.plan, pending: !disabled && event.pending, disabled, done: false, error: null })
      }
    }
    void commands.getReviewPlan({ repositoryPath, subject, sinceReviewed }, events).then(result => {
      // a newer read of the window superseded this one: its answer is `canceled`, the newer one comes
      if (!current || (result.status === "error" && result.error.code === "canceled")) {
        return
      }
      setAnswer(previous =>
        result.status === "ok"
          ? { ...(previous?.requestKey === requestKey ? previous : { subjectKey, requestKey, plan: null, pending: false, disabled: false, error: null }), done: true }
          : { subjectKey, requestKey, plan: null, pending: false, disabled: false, done: true, error: result.error.message },
      )
    })
    return () => {
      current = false
    }
  }, [subjectKey, requestKey])
  if (request === null || answer === null || answer.subjectKey !== subjectKey) {
    return request === null ? NO_PLAN : { ...NO_PLAN, loading: true }
  }
  // the plan without its usages is shown already: `pending` says the rest comes
  const loading = answer.requestKey !== requestKey || (!answer.done && answer.plan === null && !answer.disabled)
  return { plan: answer.plan, loading, pending: answer.pending, disabled: answer.disabled, error: answer.error }
}

/**
 * The plan of the diffs a window shows that has no session of its branch (the review of a commit, of a pull request):
 * of a range or a commit, as the diffs read it. Rust answers `disabled` for a repository that turns the plan off, so the
 * window needs no flag of its own
 */
export function useSubjectPlan(repositoryPath: string, subject: DiffSubject | null): ReviewPlanState {
  const request: ReviewPlanRequest | null =
    subject === null
      ? null
      : subject.kind === "commit"
        ? { repositoryPath, subject: { kind: "commit", id: subject.id }, sinceReviewed: false, head: subject.id, generation: 0 }
        : { repositoryPath, subject: { kind: "range", base: subject.base, head: subject.head }, sinceReviewed: false, head: subject.head, generation: 0 }
  return useReviewPlan(request)
}

/** The facts of the plan for the tree and the story of the diffs, once read: a plan still read again may be of another subject */
export function planFacts(state: ReviewPlanState): ReviewFacts | null {
  return state.plan === null || state.loading ? null : state.plan
}
