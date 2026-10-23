// @spec spec/branch/move-commits.spec.md
import { describe, expect, it } from "vite-plus/test"
import { internalError } from "@/lib/api-error"
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

  it("tells a stopped wait from a failure by the code of the command's error", () => {
    expect(isLockWaitCanceled({ ...internalError("The wait for /repo/.git/branch-deck.lock was canceled"), code: "canceled" })).toBe(true)
    expect(isLockWaitCanceled(Object.assign(new Error("The wait was canceled"), { code: "canceled" }))).toBe(true)
    expect(isLockWaitCanceled(internalError("canceled"))).toBe(false)
    expect(isLockWaitCanceled(new Error("boom"))).toBe(false)
  })
})
