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
    // the Status column keeps its 16rem minimum (the status text and a chip or two), so chips that load later don't
    // reflow the table; "not pushed" without chips fits in it
    expect(statusBox!.width).toBeCloseTo(16 * 16, 0)

    // the long subject is still truncated, not wrapped: the row stays a single line high
    expect(row!.height).toBeLessThan(80)
    const summary = branchName.locator(".truncate")
    await expect(summary).toContainText("add UI test that enables a plugin")
  })
})
