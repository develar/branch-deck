import { useEffect, useState } from "react"
import { Channel } from "@tauri-apps/api/core"
import type { ReviewPlanEvent, ReviewSubject } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import type { DiffSubject } from "@/lib/commit-files"
import type { Answer, ReviewPlanState } from "@/lib/review-plan-answer"
import { NO_PLAN, planStateOf, receivePlanEvent, settlePlanAnswer } from "@/lib/review-plan-answer"

/*
 * The review plan of a review window (openspec/specs/branch/review-plan/spec.md, Status: Experimental): `get_review_plan` of the
 * shown part, read again when the part, its head or "Since Your Review" change, and after Mark Reviewed. Rust sends the
 * plan without the usages while it asks the index, then with them, or `disabled`: each event replaces what the hook
 * holds. One plan of the window is in flight: Rust supersedes the older one, which answers `canceled` and is dropped
 * here.
 */

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

export function useReviewPlan(request: ReviewPlanRequest | null): ReviewPlanState {
  const subjectKey = request === null ? null : JSON.stringify([request.repositoryPath, request.subject])
  const changesKey = request === null ? null : JSON.stringify([subjectKey, request.sinceReviewed, request.head])
  const requestKey = request === null ? null : JSON.stringify([changesKey, request.generation])
  const [answer, setAnswer] = useState<Answer | null>(null)
  useEffect(() => {
    if (requestKey === null || changesKey === null || subjectKey === null) {
      return
    }
    const [repositoryPath, subject] = JSON.parse(subjectKey) as [string, ReviewSubject]
    const [, sinceReviewed] = JSON.parse(changesKey) as [string, boolean]
    const keys = { subjectKey, changesKey, requestKey }
    let current = true
    const events = new Channel<ReviewPlanEvent>()
    events.onmessage = event => {
      if (current) {
        const disabled = event.kind === "disabled"
        const next = { ...keys, plan: disabled ? null : event.plan, pending: !disabled && event.pending, disabled, done: false, error: null }
        setAnswer(previous => receivePlanEvent(previous, next))
      }
    }
    void commands.getReviewPlan({ repositoryPath, subject, sinceReviewed }, events).then(result => {
      // a newer read of the window superseded this one: its answer is `canceled`, the newer one comes
      if (!current || (result.status === "error" && result.error.code === "canceled")) {
        return
      }
      setAnswer(previous =>
        result.status === "ok" ? settlePlanAnswer(previous, keys) : { ...keys, plan: null, pending: false, disabled: false, done: true, error: result.error.message },
      )
    })
    return () => {
      current = false
    }
  }, [subjectKey, changesKey, requestKey])
  if (request === null || subjectKey === null || changesKey === null || requestKey === null) {
    return NO_PLAN
  }
  return planStateOf(answer, { subjectKey, changesKey, requestKey })
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
