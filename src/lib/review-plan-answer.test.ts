// @spec openspec/specs/branch/review-plan/spec.md
import { describe, expect, it } from "vite-plus/test"
import type { ReviewPlan } from "@/lib/bindings"
import type { Answer } from "@/lib/review-plan-answer"
import { planFacts, planSettled, planStateOf, receivePlanEvent, settlePlanAnswer } from "@/lib/review-plan-answer"

/** A plan told apart by its head; the hook holds it as Rust sent it */
function plan(head: string): ReviewPlan {
  return { head } as unknown as ReviewPlan
}

/** The keys of a read of the branch `plan` at a head, "Since Your Review" and the generation of Mark Reviewed */
function keys(head: string, generation = 0, sinceReviewed = false) {
  const subjectKey = "plan"
  const changesKey = `${subjectKey} ${String(sinceReviewed)} ${head}`
  return { subjectKey, changesKey, requestKey: `${changesKey} ${generation}` }
}

function event(read: ReturnType<typeof keys>, head: string, pending: boolean): Answer {
  return { ...read, plan: plan(head), pending, disabled: false, done: false, error: null }
}

/** The answer of a read of the head `h1` that ended with the plan with its usages */
function wholeAnswer(): Answer {
  const read = keys("h1")
  return settlePlanAnswer(receivePlanEvent(receivePlanEvent(null, event(read, "h1", true)), event(read, "h1", false)), read)
}

describe("the answer of a read of the review plan", () => {
  it("keeps the whole plan of the same changes while Mark Reviewed reads it again, until the next one has its usages", () => {
    const again = keys("h1", 1)
    const whole = wholeAnswer()
    const kept = receivePlanEvent(whole, event(again, "h1", true))
    expect(kept).toBe(whole)
    const state = planStateOf(kept, again)
    expect([state.loading, state.current, planFacts(state) === kept.plan]).toEqual([true, true, true])

    const next = receivePlanEvent(kept, event(again, "h1", false))
    const settled = planStateOf(settlePlanAnswer(next, again), again)
    expect([settled.loading, settled.current, settled.plan === next.plan]).toEqual([false, true, true])
  })

  it("keeps no facts of a plan of another head or scope while the next one is read, but the plan of the branch for its card", () => {
    const moved = keys("h2", 0)
    const afterSync = planStateOf(receivePlanEvent(wholeAnswer(), event(moved, "h2", true)), moved)
    // the first event of the read of the new head replaces the plan at once: it is of the changes shown
    expect([afterSync.pending, afterSync.current, planFacts(afterSync) === afterSync.plan, afterSync.plan?.head]).toEqual([true, true, true, "h2"])

    const stillRead = planStateOf(wholeAnswer(), moved)
    expect([stillRead.loading, stillRead.current, planFacts(stillRead), stillRead.plan?.head]).toEqual([true, false, null, "h1"])

    const since = keys("h1", 0, true)
    expect(planFacts(planStateOf(wholeAnswer(), since))).toBeNull()
  })

  it("replaces a plan without its usages by the next read's, and starts a read of another subject without a plan", () => {
    const read = keys("h1")
    const pending = receivePlanEvent(null, event(read, "h1", true))
    const again = keys("h1", 1)
    expect(receivePlanEvent(pending, event(again, "h1", true)).requestKey).toBe(again.requestKey)
    expect(planStateOf(wholeAnswer(), { ...keys("h1"), subjectKey: "other" })).toMatchObject({ plan: null, loading: true, current: false })
  })

  it("ends the read of a plan with its usages, disabled, a failure or no plan, never with a plan of other changes or one without its usages", () => {
    const read = keys("h1")
    expect(planSettled(planStateOf(wholeAnswer(), read))).toBe(true)
    expect(planSettled(planStateOf(receivePlanEvent(null, event(read, "h1", true)), read))).toBe(false)
    expect(planSettled(planStateOf(wholeAnswer(), keys("h2")))).toBe(false)
    expect(planSettled(planStateOf(null, read))).toBe(false)
    const disabled: Answer = { ...read, plan: null, pending: false, disabled: true, done: false, error: null }
    expect(planSettled(planStateOf(disabled, read))).toBe(true)
    expect(planSettled(planStateOf({ ...disabled, disabled: false, done: true, error: "index failed" }, read))).toBe(true)
    expect(planSettled(planStateOf(settlePlanAnswer(null, read), read))).toBe(true)
  })
})
