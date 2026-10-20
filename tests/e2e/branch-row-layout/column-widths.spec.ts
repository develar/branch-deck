// @spec spec/branch/branch-row.spec.md
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, getBranchCells } from "../helpers/branch-helpers"
import { expectColumnAligned } from "../helpers/column-helpers"
import { availableRobots, robot } from "../helpers/patronus-helpers"
import { waitForBranchSyncComplete } from "../helpers/sync-helpers"

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
    for (const [cell, box] of [
      [commits, commitsBox],
      [status, statusBox],
      [actions, actionsBox],
    ] as const) {
      const content = await cell.locator("> *").first().boundingBox()
      expect(content).not.toBeNull()
      expect(box!.width).toBeLessThanOrEqual(content!.width + padding)
    }
    // the long subject is still truncated, not wrapped: the row stays a single line high
    expect(row!.height).toBeLessThan(80)
    const summary = branchName.locator(".truncate")
    await expect(summary).toContainText("add UI test that enables a plugin")
  })

  test("keeps the Actions column at its buttons: no place is held for a push, a step or a copy that no row shows", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // a running robot (no push, no step), a restart, Safe Push without a push of its own (the first push is a step of it)
    await page.setViewportSize({ width: 1400, height: 900 })
    await setupRepo("issue_links", {
      patronus: availableRobots(robot("JIRA-456", "running-robot"), robot("ABC-123", "failed-robot", { status: "failure", finishTime: 1704142800 })),
    })
    await syncAndWaitForBranches()
    await expect(findBranchRow(page, "ABC-123").getByRole("button", { name: "Restart Safe Push" })).toBeVisible()
    await expect(findBranchRow(page, "feature-auth").getByRole("button", { name: "Safe Push", exact: true })).toBeVisible()
    await expect(page.getByTestId("push-button")).toHaveCount(0)
    // "Restart Safe Push" and "⋯" with the padding, the gutter between them: 328px when every row held a push and a 10rem step
    const actions = getBranchCells(findBranchRow(page, "JIRA-456")).actions
    await expect.poll(async () => (await actions.boundingBox())!.width).toBeLessThan(13 * 16)
    await expectColumnAligned(page.getByTestId("next-step-slot"))
    await expectColumnAligned(page.getByTestId("branch-row").getByRole("button", { name: /^More actions for / }))
  })

  test("drops a subject that has no room to be read", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the 768px minimum of the window at a zoom above 125% (`minWindowWidth`): the columns at their content leave every
    // name its subject
    await page.setViewportSize({ width: 768, height: 800 })
    await setupRepo("issue_links")
    await syncAndWaitForBranches()

    const summaries = page.getByTestId("branch-row").getByTestId("branch-summary")
    await expect(summaries.first()).toBeAttached()
    const visibleWidths = async () => {
      const widths: number[] = []
      for (const summary of await summaries.all()) {
        if (await summary.isVisible()) {
          widths.push((await summary.boundingBox())!.width)
        }
      }
      return widths
    }
    await expect(findBranchRow(page, "feature-auth").getByTestId("branch-summary")).toBeVisible()
    expect(await visibleWidths()).toHaveLength(await summaries.count())

    // narrower than the window gets: the issue chips of feature-auth and feature-db leave too little for their subjects,
    // which drop out; a subject that shows has at least 6rem, never a letter or two
    await page.setViewportSize({ width: 700, height: 800 })
    await expect(findBranchRow(page, "feature-auth").getByTestId("branch-summary")).toBeHidden()
    for (const width of await visibleWidths()) {
      expect(width).toBeGreaterThanOrEqual(6 * 16)
    }
  })
})
