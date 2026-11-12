import type { PlanWait, ReviewPlan } from "@/lib/bindings"
import type { ReviewFacts } from "@/presenters/review-plan"

/*
 * What a review window holds of its reads of the plan (`useReviewPlan`, openspec/specs/branch/review-plan/spec.md): each event of
 * a read replaces the answer, except a plan that still waits for the index of a read again of the same changes; the
 * state the window shows of it.
 */

/**
 * The plan of the review and its read; the plan of the same branch stays while the next one is read, and the plan of
 * the same changes (read again after Mark Reviewed) stays whole until the next one is whole
 */
export interface ReviewPlanState {
  readonly plan: ReviewPlan | null
  /** A read is in flight */
  readonly loading: boolean
  /** The plan is of the changes asked: the subject, "Since Your Review" and the head; its facts hold for the diffs shown */
  readonly current: boolean
  /**
   * What the plan still waits for while Rust asks the index, the plan with it following (`ReviewPlanEvent.waits`):
   * every answer of the index, or the tests of the steps alone; null for the plan whole
   */
  readonly waits: PlanWait | null
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
  readonly waits: PlanWait | null
  readonly disabled: boolean
  /** The command answered: no other event of it follows */
  readonly done: boolean
  readonly error: string | null
}

export const NO_PLAN: ReviewPlanState = { plan: null, loading: false, current: false, waits: null, disabled: false, error: null }

/**
 * The answer after an event of a read: it replaces the one held, except that a read again of the changes of a whole
 * plan (Mark Reviewed) keeps that plan while it sends the plans that still wait for the index: the steps of the tree,
 * the sections of the uses left outside and the tests of the steps stay until the plan whole comes
 */
export function receivePlanEvent(previous: Answer | null, next: Answer): Answer {
  const whole = previous !== null && previous.plan !== null && previous.waits === null && !previous.disabled
  return whole && previous.changesKey === next.changesKey && previous.requestKey !== next.requestKey && next.waits !== null ? previous : next
}

/**
 * The answer once its read ended well: the last event of it, done; a plan of the same changes kept while it read stays
 * (a read ends with the plan whole, so only one without any event keeps it)
 */
export function settlePlanAnswer(previous: Answer | null, read: Omit<Answer, "plan" | "waits" | "disabled" | "done" | "error">): Answer {
  if (previous !== null && (previous.requestKey === read.requestKey || (previous.changesKey === read.changesKey && previous.plan !== null))) {
    return { ...previous, requestKey: read.requestKey, done: true }
  }
  return { ...read, plan: null, waits: null, disabled: false, done: true, error: null }
}

/** What the hook shows of the answer for the request of these keys */
export function planStateOf(answer: Answer | null, keys: { subjectKey: string; changesKey: string; requestKey: string }): ReviewPlanState {
  if (answer === null || answer.subjectKey !== keys.subjectKey) {
    return { ...NO_PLAN, loading: true }
  }
  // a plan that still waits for the index is shown already: `waits` says what comes
  const loading = answer.requestKey !== keys.requestKey || (!answer.done && answer.plan === null && !answer.disabled)
  return { plan: answer.plan, loading, current: answer.changesKey === keys.changesKey, waits: answer.waits, disabled: answer.disabled, error: answer.error }
}

/**
 * The read of the plan ended for the changes asked: the plan whole, `disabled`, a failure, or no plan; a review that
 * waits for it keeps the one on screen meanwhile
 */
export function planSettled(state: ReviewPlanState): boolean {
  return state.current && state.waits === null && (state.plan !== null || state.disabled || state.error !== null || !state.loading)
}

/** The two states show the same: the review keeps the one it took */
export function samePlanState(a: ReviewPlanState, b: ReviewPlanState): boolean {
  return a.plan === b.plan && a.loading === b.loading && a.current === b.current && a.waits === b.waits && a.disabled === b.disabled && a.error === b.error
}

/**
 * The facts of the plan for the tree and the story of the diffs: those of the changes shown, also while the plan is read
 * again after Mark Reviewed; a plan still read again for another head or scope is of other changes
 */
export function planFacts(state: ReviewPlanState): ReviewFacts | null {
  return state.plan === null || !state.current ? null : state.plan
}
