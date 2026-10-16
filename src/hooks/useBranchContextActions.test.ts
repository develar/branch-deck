// @spec spec/branch/branch-review.spec.md
import { afterEach, describe, expect, it } from "vitest"
import { getSafePushBlocker } from "@/hooks/useBranchContextActions"
import { inlineRowActionStore } from "@/stores/inline-row-action"
import { testBranch } from "@/stores/test-branch"

describe("getSafePushBlocker", () => {
  afterEach(() => {
    inlineRowActionStore.setState({ activeInline: null })
  })

  it("says why a branch can't start a Safe Push, in the words of every surface that offers it", () => {
    expect(getSafePushBlocker(testBranch("login", null))).toBeNull()
    expect(getSafePushBlocker(testBranch("login", null, { hasError: true }))).toBe("The branch failed to sync")
    expect(getSafePushBlocker(testBranch("login", null, { commitCount: 0 }))).toBe("No commits to push")
    inlineRowActionStore.setState({ activeInline: { type: "rename", branchName: "login", processing: true } })
    expect(getSafePushBlocker(testBranch("login", null))).toBe("Wait for the action on the branch to finish")
  })
})
