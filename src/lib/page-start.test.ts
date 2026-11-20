// @spec openspec/specs/repository/ide-embedding/spec.md
import { describe, expect, it } from "vite-plus/test"
import { markPageStarted, whenPageStarted } from "@/lib/page-start"

describe("the start of the page", () => {
  it("runs each waiter once the page started, and a later one at once", () => {
    const ran: string[] = []
    whenPageStarted(() => ran.push("before"))
    expect(ran).toEqual([])
    markPageStarted()
    // a second start runs nothing again
    markPageStarted()
    whenPageStarted(() => ran.push("after"))
    expect(ran).toEqual(["before", "after"])
  })
})
