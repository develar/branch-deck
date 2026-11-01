import type { ReviewPlan } from "@/lib/bindings"
import type { ReviewFacts } from "@/presenters/review-plan"

/*
 * What a review window holds of its reads of the plan (`useReviewPlan`, spec/branch/review-plan.spec.md): each event of
 * a read replaces the answer, except the plan without the usages of a read again of the same changes; the state the
 * window shows of it.
 */

/**
 * The plan of the review and its read; the plan of the same branch stays while the next one is read, and the plan of
 * the same changes (read again after Mark Reviewed) stays whole until the next one has its usages
 */
export interface ReviewPlanState {
  readonly plan: ReviewPlan | null
  /** A read is in flight */
  readonly loading: boolean
  /** The plan is of the changes asked: the subject, "Since Your Review" and the head; its facts hold for the diffs shown */
  readonly current: boolean
  /** The plan has no usages yet: Rust asks the index, the plan with them follows */
  readonly pending: boolean
  /** The repository turns the review plan off: the window shows none */
  readonly disabled: boolean
  /** Why the last read failed; null after an answer */
  readonly error: string | null
}

export interface Answer {
  /** The subject the answer is of, without `sinceReviewed`: a plan of the same branch stays while the next is read */
  readonly subjectKey: string
  /** The changes the plan is of: the subject, `sinceReviewed` and the head, without the generation of Mark Reviewed */
  readonly changesKey: string
  readonly requestKey: string
  readonly plan: ReviewPlan | null
  readonly pending: boolean
  readonly disabled: boolean
  /** The command answered: no other event of it follows */
  readonly done: boolean
  readonly error: string | null
}

export const NO_PLAN: ReviewPlanState = { plan: null, loading: false, current: false, pending: false, disabled: false, error: null }

/**
 * The answer after an event of a read: it replaces the one held, except that a read again of the changes of a whole
 * plan (Mark Reviewed) keeps that plan while it sends the one without the usages: the steps of the tree and the
 * sections of the uses left outside stay until the plan with them comes
 */
export function receivePlanEvent(previous: Answer | null, next: Answer): Answer {
  const whole = previous !== null && previous.plan !== null && !previous.pending && !previous.disabled
  return whole && previous.changesKey === next.changesKey && previous.requestKey !== next.requestKey && next.pending ? previous : next
}

/**
 * The answer once its read ended well: the last event of it, done; a plan of the same changes kept while it read stays
 * (a read ends with the plan with its usages, so only one without any event keeps it)
 */
export function settlePlanAnswer(previous: Answer | null, read: Omit<Answer, "plan" | "pending" | "disabled" | "done" | "error">): Answer {
  if (previous !== null && (previous.requestKey === read.requestKey || (previous.changesKey === read.changesKey && previous.plan !== null))) {
    return { ...previous, requestKey: read.requestKey, done: true }
  }
  return { ...read, plan: null, pending: false, disabled: false, done: true, error: null }
}

/** What the hook shows of the answer for the request of these keys */
export function planStateOf(answer: Answer | null, keys: { subjectKey: string; changesKey: string; requestKey: string }): ReviewPlanState {
  if (answer === null || answer.subjectKey !== keys.subjectKey) {
    return { ...NO_PLAN, loading: true }
  }
  // the plan without its usages is shown already: `pending` says the rest comes
  const loading = answer.requestKey !== keys.requestKey || (!answer.done && answer.plan === null && !answer.disabled)
  return { plan: answer.plan, loading, current: answer.changesKey === keys.changesKey, pending: answer.pending, disabled: answer.disabled, error: answer.error }
}

/**
 * The facts of the plan for the tree and the story of the diffs: those of the changes shown, also while the plan is read
 * again after Mark Reviewed; a plan still read again for another head or scope is of other changes
 */
export function planFacts(state: ReviewPlanState): ReviewFacts | null {
  return state.plan === null || !state.current ? null : state.plan
}
