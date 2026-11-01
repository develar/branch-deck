// @spec spec/review/review-window.spec.md
import type { Page } from "@playwright/test"
import type { CommitsReviewWindowData } from "../../../src/types/sub-window"
import { test, expect } from "../fixtures/test-fixtures"
import { getUnassignedCommitRow } from "../helpers/branch-helpers"
import { openedReview, openReviewWindow } from "../helpers/review-window-helpers"
import { openLinkedSubWindow, reopenLinkedSubWindow } from "../helpers/sub-window-helpers"

/** The review the window shows: the files of its diffs, in their order */
async function reviewedPaths(window: Page): Promise<string[]> {
  return window.getByTestId("review-file-header").evaluateAll(headers => headers.map(header => header.getAttribute("data-path")!))
}

test.describe("One review window", () => {
  // the `single_unassigned` template: `feature` with auth.js, and the unassigned "Fix critical bug" with bugfix.txt
  test.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("single_unassigned")
    await syncAndWaitForBranches()
  })

  test("should review a branch and then a commit of the unassigned list in the same window, and go back with ⌘[ and with Back", async ({ page }) => {
    const window = await openLinkedSubWindow(page, await openReviewWindow(page, "feature"))
    const segments = window.getByTestId("branch-review-segment")
    await expect(segments).toHaveCount(1)
    await expect.poll(() => reviewedPaths(window)).toEqual(["auth.js"])
    // nothing to go back to yet
    await expect(window.getByTestId("review-back")).toHaveCount(0)

    // D on the unassigned commit in the main window: the same window starts over on the list, the branch in its history
    const reviewCommit = async (count: number) => {
      await getUnassignedCommitRow(page, "Fix critical bug").hover()
      await page.keyboard.press("d")
      const opened = await openedReview<CommitsReviewWindowData>(page, count)
      expect(opened.data).toMatchObject({ kind: "commits", listLabel: "Unassigned" })
      await reopenLinkedSubWindow(window, opened)
      await expect(window.getByTestId("window-title")).toContainText("Unassigned")
      await expect.poll(() => reviewedPaths(window)).toEqual(["bugfix.txt"])
      await expect(segments).toHaveCount(0)
    }
    await reviewCommit(2)

    // ⌘[: the branch again, read again; nothing left to go back to
    await expect(window.getByRole("button", { name: "Back to feature" })).toBeVisible()
    // the modifier of the page's platform: ⌘ on macOS, Ctrl elsewhere
    const mac = await window.evaluate(() => navigator.userAgent.toLowerCase().includes("mac"))
    await window.keyboard.press(mac ? "Meta+BracketLeft" : "Control+BracketLeft")
    await expect(segments).toHaveCount(1)
    await expect.poll(() => reviewedPaths(window)).toEqual(["auth.js"])
    await expect(window.getByTestId("review-back")).toHaveCount(0)
    // the branch is the window's again: its actions, from its own session
    await expect(window.getByTestId("branch-review-actions").getByRole("button", { name: "More actions for feature" })).toBeVisible()

    // the list again, then the Back button
    await reviewCommit(3)
    await window.getByRole("button", { name: "Back to feature" }).click()
    await expect(segments).toHaveCount(1)
    await expect.poll(() => reviewedPaths(window)).toEqual(["auth.js"])
  })
})
