import { describe, expect, it } from "vitest"
import type { TransferProgress } from "@/lib/bindings"
import { transferAmount, transferLabel } from "@/lib/transfer-progress"

const writing: TransferProgress = { percent: 51, transferred: "2.61 MiB", speed: "5.20 MiB/s", line: "Writing objects:  45% (136/302), 2.61 MiB | 5.20 MiB/s" }

describe("transferLabel", () => {
  it("shows the percent once git reports a stage", () => {
    expect(transferLabel("Pushing", null)).toBe("Pushing…")
    expect(transferLabel("Pushing", writing)).toBe("Pushing 51%")
  })
})

describe("transferAmount", () => {
  it("shows the amount and the rate as git prints them", () => {
    expect(transferAmount(null)).toBeNull()
    expect(transferAmount({ ...writing, transferred: null, speed: null })).toBeNull()
    expect(transferAmount({ ...writing, speed: null })).toBe("2.61 MiB")
    expect(transferAmount(writing)).toBe("2.61 MiB · 5.20 MiB/s")
  })
})
