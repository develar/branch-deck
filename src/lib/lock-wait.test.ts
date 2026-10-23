// @spec spec/branch/move-commits.spec.md
import { describe, expect, it } from "vite-plus/test"
import { actionOfLabel, canceledBeforeStart, isLockWaitCanceled } from "@/lib/lock-wait"

describe("canceledBeforeStart", () => {
  it("says that an action stopped while it waited for the repository lock changed nothing", () => {
    expect(canceledBeforeStart("the push of auth", "Nothing was pushed")).toEqual({
      type: "info",
      title: "Canceled the push of auth before it started",
      description: "Nothing was pushed",
    })
    expect(canceledBeforeStart(actionOfLabel("Squashing 2 commits"))).toEqual({
      type: "info",
      title: "Canceled squashing 2 commits before it started",
      description: "Nothing changed",
    })
  })

  it("tells a stopped wait from a failure by the kind of the command's error", () => {
    expect(isLockWaitCanceled({ message: "The wait was canceled", kind: "lockWaitCanceled" })).toBe(true)
    expect(isLockWaitCanceled({ message: "boom", kind: null })).toBe(false)
    expect(isLockWaitCanceled(new Error("boom"))).toBe(false)
  })
})
