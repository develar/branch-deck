import { describe, expect, it } from "vite-plus/test"
import type { TransferProgress } from "@/lib/bindings"
import { lockWaitProgress, stopWaitingPushItem, transferAmount, transferAmountNotInLine, transferLabel, transferPercent } from "@/lib/transfer-progress"

const writing: TransferProgress = { percent: 51, transferred: "2.61 MiB", speed: "5.20 MiB/s", line: "Writing objects:  45% (136/302), 2.61 MiB | 5.20 MiB/s" }

describe("transferLabel", () => {
  it("shows the percent once git reports a stage", () => {
    expect(transferLabel("Pushing", null)).toBe("Pushing…")
    expect(transferLabel("Pushing", writing)).toBe("Pushing 51%")
  })

  it("names the holder of the repository lock the push waits for, without a percent", () => {
    const waiting = lockWaitProgress("Waiting for `bd pick` (pid 4242, for 12 s)", () => {})
    expect(transferLabel("Pushing", waiting)).toBe("Waiting for `bd pick` (pid 4242, for 12 s)")
    expect(transferPercent(waiting)).toBeNull()
    expect(transferPercent(writing)).toBe(51)
    expect(transferPercent(null)).toBeNull()
  })
})

describe("stopWaitingPushItem", () => {
  it("offers Stop Waiting of a push only while it waits for another holder of the repository lock", () => {
    const stops: string[] = []
    const progress = lockWaitProgress("Waiting for `bd pick` (pid 4242, for 12 s)", () => stops.push("op"))
    expect(stopWaitingPushItem(progress)).toMatchObject({ label: "Stop Waiting", description: "Waiting for `bd pick` (pid 4242, for 12 s); stop and push nothing" })
    stopWaitingPushItem(progress)!.onSelect({ surface: null })
    expect(stops).toEqual(["op"])
    expect(stopWaitingPushItem(writing)).toBeNull()
    expect(stopWaitingPushItem(null)).toBeNull()
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

describe("transferAmountNotInLine", () => {
  it("shows the amount below git's line only when the line doesn't say it", () => {
    expect(transferAmountNotInLine(null)).toBeNull()
    expect(transferAmountNotInLine(writing)).toBeNull()
    const resolving: TransferProgress = { ...writing, percent: 98, line: "Resolving deltas: 100% (2/2), completed with 2 local objects." }
    expect(transferAmountNotInLine(resolving)).toBe("2.61 MiB · 5.20 MiB/s")
    expect(transferAmountNotInLine({ ...resolving, transferred: null, speed: null })).toBeNull()
  })
})
