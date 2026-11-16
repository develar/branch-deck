import { describe, expect, it } from "vite-plus/test"
import { isPending, keptAnswer, noAnswers, shownAnswer, withAnswer } from "@/lib/debounced-answers"

type Answer = { status: "ok"; data: string } | { status: "error"; error: string }

const keep = (answer: Answer) => answer.status === "ok"
const ok = (data: string): Answer => ({ status: "ok", data })
const failed: Answer = { status: "error", error: "Load failed" }

describe("the answers of a debounced read", () => {
  it("keeps an answer by its input, so typing back shows it at once and asks nothing", () => {
    const answers = withAnswer(noAnswers<Answer>("", undefined), "", "foo", ok("valid"), keep(ok("valid")))
    expect(keptAnswer(answers, "", "foo")).toEqual(ok("valid"))
    expect(isPending(answers, "", "foo")).toBe(false)
    expect(isPending(answers, "", "bar")).toBe(true)
  })

  it("shows a failed answer without keeping it: no pending one, and asked again, a later answer of the same input kept", () => {
    const answers = withAnswer(withAnswer(noAnswers<Answer>("", undefined), "", "bar", ok("valid"), keep(ok("valid"))), "", "foo", failed, keep(failed))
    expect(answers.latest).toEqual(failed)
    // nothing kept of it: the next pause of the typing at it and a submit ask again
    expect(keptAnswer(answers, "", "foo")).toBeUndefined()
    expect(isPending(answers, "", "foo")).toBe(false)
    // the answers kept before stay
    expect(keptAnswer(answers, "", "bar")).toEqual(ok("valid"))

    const again = withAnswer(answers, "", "foo", ok("valid"), keep(ok("valid")))
    expect(keptAnswer(again, "", "foo")).toEqual(ok("valid"))
    expect(again.unkept).toBeNull()
  })

  it("keeps the failure of an input when a slower window of another input ends after it with a kept answer", () => {
    // the window of "fo" is slow, that of "foo" fails first, then "fo" answers
    const failedFoo = withAnswer(noAnswers<Answer>("", undefined), "", "foo", failed, keep(failed))
    const answers = withAnswer(failedFoo, "", "fo", ok("valid"), keep(ok("valid")))
    expect(isPending(answers, "", "foo")).toBe(false)
    expect(shownAnswer(answers, "", "foo")).toEqual(failed)
    expect(shownAnswer(answers, "", "fo")).toEqual(ok("valid"))
    // a kept answer of "foo" itself ends its failure
    const kept = withAnswer(answers, "", "foo", ok("valid"), keep(ok("valid")))
    expect(kept.unkept).toBeNull()
    expect(shownAnswer(kept, "", "foo")).toEqual(ok("valid"))
  })

  it("drops an answer asked for another reset, and a new reset knows nothing but the latest", () => {
    const answers = withAnswer(noAnswers<Answer>("a", undefined), "b", "foo", ok("valid"), keep(ok("valid")))
    expect(keptAnswer(answers, "a", "foo")).toBeUndefined()
    const failedThen = withAnswer(noAnswers<Answer>("a", undefined), "a", "foo", failed, keep(failed))
    expect(isPending(failedThen, "b", "foo")).toBe(true)
  })
})
