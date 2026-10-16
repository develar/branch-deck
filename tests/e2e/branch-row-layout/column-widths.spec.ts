// @spec spec/branch/branch-row.spec.md
import { test, expect } from "../fixtures/test-fixtures"
import { waitForBranchSyncComplete } from "../helpers/sync-helpers"
import { findBranchRow, getBranchCells } from "../helpers/branch-helpers"

// https://github.com/develar/branch-deck/issues/72
test.describe("Branch table column widths", () => {
  test("gives the free width to the branch name and keeps the other columns at content width", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await page.setViewportSize({ width: 1400, height: 900 })
    await setupRepo("long_subjects")
    await syncAndWaitForBranches()

    const branchRow = findBranchRow(page, "IJPL-215166")
    await waitForBranchSyncComplete(page, "IJPL-215166")

    const { branchName, commits, status, actions } = getBranchCells(branchRow)
    const row = await branchRow.boundingBox()
    const name = await branchName.boundingBox()
    const commitsBox = await commits.boundingBox()
    const statusBox = await status.boundingBox()
    const actionsBox = await actions.boundingBox()
    expect(row && name && commitsBox && statusBox && actionsBox).toBeTruthy()

    // the name column takes more than half of the table
    expect(name!.width).toBeGreaterThan(row!.width / 2)
    // the right columns are no wider than their content plus padding (px-4 = 16px each side)
    const padding = 2 * 16 + 1
    for (const [cell, box] of [[commits, commitsBox], [actions, actionsBox]] as const) {
      const content = await cell.locator("> *").first().boundingBox()
      expect(content).not.toBeNull()
      expect(box!.width).toBeLessThanOrEqual(content!.width + padding)
    }
    // the Status column keeps its 12rem minimum (the state of the branch in a fixed 9rem, its robot or the status text,
    // then the pull request and the Landed chip), so a robot or a chip that loads later doesn't reflow the table; "not
    // pushed" without chips fits in it
    expect(statusBox!.width).toBeCloseTo(12 * 16, 0)

    // the long subject is still truncated, not wrapped: the row stays a single line high
    expect(row!.height).toBeLessThan(80)
    const summary = branchName.locator(".truncate")
    await expect(summary).toContainText("add UI test that enables a plugin")
  })

  test("drops a subject that has no room to be read", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // below the 960px minimum of the window, as a zoomed page has it: the compact Actions cell leaves the names more room
    // than the room a longer name with its chips leaves at the minimum
    await page.setViewportSize({ width: 780, height: 800 })
    await setupRepo("issue_links")
    await syncAndWaitForBranches()

    const summaries = page.getByTestId("branch-row").getByTestId("branch-summary")
    await expect(summaries.first()).toBeAttached()
    let hidden = 0
    for (const summary of await summaries.all()) {
      if (await summary.isVisible()) {
        // a visible subject has at least 6rem, never a letter or two
        expect((await summary.boundingBox())!.width).toBeGreaterThanOrEqual(6 * 16)
      }
      else {
        hidden++
      }
    }
    // the issue chips of feature-auth and feature-db leave about 3rem for their subjects
    expect(hidden).toBeGreaterThan(0)
    await expect(findBranchRow(page, "feature-auth").getByTestId("branch-summary")).toBeHidden()
  })
})
