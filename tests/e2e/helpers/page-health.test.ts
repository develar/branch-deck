import { describe, expect, it } from "vite-plus/test"
import { answeredTogether } from "./page-health"

describe("answeredTogether", () => {
  it("is false when a page did not answer, even beside a page that closed in its probe", () => {
    expect(answeredTogether([false, null])).toBe(false)
    expect(answeredTogether([true, false])).toBe(false)
  })

  it("is unknown when a page closed in its probe and none was stuck, or when no page was asked", () => {
    expect(answeredTogether([null, true])).toBeNull()
    expect(answeredTogether([])).toBeNull()
  })

  it("is true when every page answered", () => {
    expect(answeredTogether([true, true])).toBe(true)
  })
})
