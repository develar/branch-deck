import { describe, expect, it } from "vite-plus/test"
import { markOpenPhase, takeOpenTimingLine } from "@/lib/open-timing"

describe("takeOpenTimingLine", () => {
  it("tells the phases of an open after its opener asked, the first time of each, once", () => {
    const open = { id: 3, openedAt: 1000 }
    markOpenPhase(open, "loaded", 1041)
    markOpenPhase(open, "files", 1044)
    markOpenPhase(open, "files", 1090)
    markOpenPhase(open, "diffs", 1047)
    markOpenPhase(open, "ready", 1096)
    expect(takeOpenTimingLine("commit-review", 3)).toBe("commit-review open 96 ms: loaded 41, files 44, diffs 47")
    expect(takeOpenTimingLine("commit-review", 3)).toBeNull()
  })

  it("starts over for another open and tells nothing of an open it did not time", () => {
    markOpenPhase({ id: 4, openedAt: 0 }, "loaded", 10)
    markOpenPhase({ id: 5, openedAt: 100 }, "ready", 150)
    expect(takeOpenTimingLine("branch-review", 4)).toBeNull()
    expect(takeOpenTimingLine("branch-review", 5)).toBe("branch-review open 50 ms")
  })
})
