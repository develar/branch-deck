// @spec openspec/specs/branch/review-plan/spec.md
import { describe, expect, it } from "vite-plus/test"
import type { PlanWait, ReviewPlan } from "@/lib/bindings"
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

/** An event of a read: the plan of `head` and what it still waits for of the index, null for the plan whole */
function event(read: ReturnType<typeof keys>, head: string, waits: PlanWait | null): Answer {
  return { ...read, plan: plan(head), waits, disabled: false, done: false, error: null }
}

/** The answer of a read of the head `h1` that ended with the plan whole, after each of its stages */
function wholeAnswer(): Answer {
  const read = keys("h1")
  const staged = receivePlanEvent(receivePlanEvent(null, event(read, "h1", "index")), event(read, "h1", "tests"))
  return settlePlanAnswer(receivePlanEvent(staged, event(read, "h1", null)), read)
}

describe("the answer of a read of the review plan", () => {
  it("keeps the whole plan of the same changes while Mark Reviewed reads it again, until the next one is whole", () => {
    const again = keys("h1", 1)
    const whole = wholeAnswer()
    // neither the plan that waits for every answer of the index nor the one that waits for its tests replaces it
    const kept = receivePlanEvent(receivePlanEvent(whole, event(again, "h1", "index")), event(again, "h1", "tests"))
    expect(kept).toBe(whole)
    const state = planStateOf(kept, again)
    expect([state.loading, state.current, state.waits, planFacts(state) === kept.plan]).toEqual([true, true, null, true])

    const next = receivePlanEvent(kept, event(again, "h1", null))
    const settled = planStateOf(settlePlanAnswer(next, again), again)
    expect([settled.loading, settled.current, settled.plan === next.plan]).toEqual([false, true, true])
  })

  it("keeps no facts of a plan of another head or scope while the next one is read, but the plan of the branch for its card", () => {
    const moved = keys("h2", 0)
    const afterSync = planStateOf(receivePlanEvent(wholeAnswer(), event(moved, "h2", "index")), moved)
    // the first event of the read of the new head replaces the plan at once: it is of the changes shown
    expect([afterSync.waits, afterSync.current, planFacts(afterSync) === afterSync.plan, afterSync.plan?.head]).toEqual(["index", true, true, "h2"])

    const stillRead = planStateOf(wholeAnswer(), moved)
    expect([stillRead.loading, stillRead.current, planFacts(stillRead), stillRead.plan?.head]).toEqual([true, false, null, "h1"])

    const since = keys("h1", 0, true)
    expect(planFacts(planStateOf(wholeAnswer(), since))).toBeNull()
  })

  it("shows each stage of a read as it comes: the plan that waits for the index, the one that waits for its tests, the plan whole", () => {
    const read = keys("h1")
    const first = receivePlanEvent(null, event(read, "h1", "index"))
    const second = receivePlanEvent(first, event(read, "h1", "tests"))
    const whole = receivePlanEvent(second, event(read, "h1", null))
    const shown = [first, second, whole].map(answer => planStateOf(answer, read))
    expect(shown.map(state => [state.waits, state.loading, state.plan === null])).toEqual([
      ["index", false, false],
      ["tests", false, false],
      [null, false, false],
    ])
    expect([second.plan === first.plan, whole.plan === second.plan]).toEqual([false, false])
  })

  it("replaces a plan that still waits for the index by the next read's, and starts a read of another subject without a plan", () => {
    const read = keys("h1")
    const again = keys("h1", 1)
    for (const waits of ["index", "tests"] as const) {
      const waiting = receivePlanEvent(null, event(read, "h1", waits))
      expect(receivePlanEvent(waiting, event(again, "h1", "index")).requestKey).toBe(again.requestKey)
    }
    expect(planStateOf(wholeAnswer(), { ...keys("h1"), subjectKey: "other" })).toMatchObject({ plan: null, loading: true, current: false })
  })

  it("ends the read of a plan whole, disabled, a failure or no plan, never with a plan of other changes or one that still waits for the index", () => {
    const read = keys("h1")
    expect(planSettled(planStateOf(wholeAnswer(), read))).toBe(true)
    expect(planSettled(planStateOf(receivePlanEvent(null, event(read, "h1", "index")), read))).toBe(false)
    expect(planSettled(planStateOf(receivePlanEvent(null, event(read, "h1", "tests")), read))).toBe(false)
    expect(planSettled(planStateOf(wholeAnswer(), keys("h2")))).toBe(false)
    expect(planSettled(planStateOf(null, read))).toBe(false)
    const disabled: Answer = { ...read, plan: null, waits: null, disabled: true, done: false, error: null }
    expect(planSettled(planStateOf(disabled, read))).toBe(true)
    expect(planSettled(planStateOf({ ...disabled, disabled: false, done: true, error: "index failed" }, read))).toBe(true)
    expect(planSettled(planStateOf(settlePlanAnswer(null, read), read))).toBe(true)
  })
})
