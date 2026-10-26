// @spec spec/repository/outgoing.spec.md
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, getPushButton } from "../helpers/branch-helpers"
import { availablePullRequests, pullRequest } from "../helpers/pull-request-helpers"
import { syncFromMenu } from "../helpers/sync-helpers"

/** Three hours before the fixed clock of the page */
const THEIR_TIME = Date.parse("2024-01-02T07:00:00Z") / 1000

test.describe("Remote commits of others", () => {
  test("should fetch the branch of a pull request someone pushed to and name their commit before any push", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("simple", { pullRequests: availablePullRequests(pullRequest("test-branch", 800)) })
    await syncAndWaitForBranches()
    const row = findBranchRow(page, "test-branch")
    const theirs = await repo.pushFromElsewhere("user-name/virtual/test-branch", {
      author: "Lev Leontev <lev@example.com>",
      file: "fix.txt",
      content: "fix\n",
      timestamp: THEIR_TIME,
    })
    await repo.setPullRequests(availablePullRequests(pullRequest("test-branch", 800, { headRefOid: theirs })))

    // a sync reads the remote branch as pushed; its listing of the pull requests shows the head the remote moved to
    await repo.advanceBackendClock(60_000)
    await syncFromMenu(page)

    const toast = page.locator("[data-slot=toast]", { hasText: "Lev Leontev pushed 1 commit to test-branch" })
    await expect(toast).toContainText("Your branch doesn't have it: a push drops it.")
    await expect(row).toContainText("1↓")
    // the push the row offers drops their commit, so it asks first
    await expect(getPushButton(row)).toHaveAccessibleName("Force Push")
    await getPushButton(row).hover()
    await expect(page.getByTestId("push-description")).toContainText("The remote has 1 commit your branch doesn't (by Lev Leontev).")

    // the same head again fetches nothing more and tells nothing more
    await repo.advanceBackendClock(60_000)
    await syncFromMenu(page)
    await expect(page.locator("[data-slot=toast]", { hasText: "Lev Leontev pushed" })).toHaveCount(1)
  })
})
