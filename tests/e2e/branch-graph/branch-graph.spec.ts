// @spec spec/branch/branch-row.spec.md
import { test, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { findBranchRow } from "../helpers/branch-helpers"
import { readClipboard } from "../helpers/clipboard-helpers"
import { DAY_ROBOTS, availableRobots, robot, setupRobots } from "../helpers/patronus-helpers"
import { getOpenedUrls } from "../helpers/pull-request-helpers"
import { routeCommand } from "../helpers/wait-helpers"

test.describe("Branch graph", () => {
  test("should give every branch of a table without stacks its node, and keep the push on the dot of the status", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("simple")
    await syncAndWaitForBranches()
    const count = await page.getByTestId("branch-row").count()
    expect(count).toBeGreaterThan(0)
    await expect(page.getByTestId("branch-row").getByTestId("lane-node")).toHaveCount(count)
    await expect(page.getByTestId("branch-status-dot")).toHaveCount(count)
    // not on Space, no pull requests GitHub lists: the push is the step the node shows, its hover says it
    const row = findBranchRow(page, "test-branch")
    await expect(row.getByTestId("lane-node")).toHaveAttribute("data-kind", "yourStep")
    await row.getByTestId("lane-node-target").hover()
    await expect(page.getByTestId("lane-node-details")).toHaveText("Not on the remote yet: push it")
    await expect(row.getByTestId("branch-status-dot")).toHaveAttribute("data-color", "primary")
  })

  test("should explain the nodes in the legend after the Branch Name head", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_branches")
    await syncAndWaitForBranches()
    await expect(page.getByRole("columnheader", { name: /Branch Name/ })).toBeVisible()
    await page.getByRole("button", { name: "About Branch Graph" }).hover()
    const legend = page.getByTestId("branch-graph-legend")
    await expect(legend).toBeVisible()
    const kinds = await legend.getByTestId("lane-node").evaluateAll(nodes => nodes.map(node => node.getAttribute("data-kind")))
    expect(kinds).toEqual(["ready", "yourStep", "needsFix", "landing", "waiting", "inMaster"])
    await expect(legend.locator("dd").first()).toHaveText("Ready to land: Patronus counts the approvals, Safe Push lands it")
    await captureHtmlSnapshot(page.locator("[data-slot=popover-content]").filter({ has: legend }), "branch-graph-legend")
  })

  test("should say in the island header where the whole stack is on its way into master", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stack_island", { patronus: availableRobots() })
    await syncAndWaitForBranches()
    // the stack forks: one Safe Push of its stack branch lands its seven branches
    const header = page.locator('[data-testid="stack-island"][data-stack-root="air"]').getByTestId("stack-island-node")
    // GitHub lists no pull request here: nobody judges the approvals before the start, so it is the user's step
    await expect(header.getByTestId("lane-node")).toHaveAttribute("data-kind", "yourStep")
    await header.hover()
    await expect(page.getByTestId("lane-node-details")).toHaveText("Safe Push lands 7 branches: Patronus checks the approvals when the robot starts")
    // the node of its root row says it of the branch alone
    await page.mouse.move(0, 0)
    await findBranchRow(page, "air").getByTestId("lane-node-target").hover()
    await expect(page.getByTestId("lane-node-details").filter({ hasText: "Safe Push:" }).last()).toHaveText("Safe Push: Patronus checks the approvals when the robot starts")
  })

  test("should open the issue and the failed Safe Push from the hover of the node, and restart it or copy the prompt there", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const failed = DAY_ROBOTS.failed
    await setupRobots(page, setupRepo, syncAndWaitForBranches, failed)
    const restarts = await routeCommand(page, "restart_patronus_robot", () => robot("ABC-123", "restarted-robot"))
    const row = findBranchRow(page, "ABC-123")
    await expect(row.getByTestId("lane-node")).toHaveAttribute("data-kind", "needsFix")
    await expect(row).toHaveAttribute("data-state", "closed")

    await row.getByTestId("lane-node-target").hover()
    const details = page.getByTestId("lane-node-details")
    await expect(details.getByText("Safe Push failed on ABC-123 as it is: fix it, or Restart Safe Push")).toBeVisible()
    // the issue key is a link to the tracker, ↗ opens the robot in Patronus
    await expect(details.getByRole("link", { name: "ABC-123" })).toHaveAttribute("href", "https://jira.example.com/browse/ABC-123")
    const robotRow = details.getByTestId("lane-node-robot").getByTestId("robots-row")
    await expect(robotRow.getByTestId("robots-row-state")).toHaveText(/^Failed · /)
    await robotRow.getByRole("button", { name: "Open Safe Push of ABC-123 in Patronus" }).click()
    expect(await getOpenedUrls(page)).toEqual([failed.url])

    // the fix: the prompt for an agent; the actions of the robot as in the landing overview, the row stays as it was
    await robotRow.getByRole("button", { name: "Copy Prompt for Agent of ABC-123" }).click()
    await expect.poll(() => readClipboard(page)).toContain(`Safe Push of \`ABC-123\` into \`master\` failed: ${failed.url}`)
    await robotRow.getByRole("button", { name: "Restart Safe Push of ABC-123" }).click()
    expect(restarts).toEqual([expect.objectContaining({ robotId: "failed-robot" })])
    await expect(page.locator("[data-slot=toast]", { hasText: "Safe Push restarted" })).toBeVisible()
    await expect(row).toHaveAttribute("data-state", "closed")
  })

  test("should go on to the row on a click of the node, never pinning its hover", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches, DAY_ROBOTS.failed)
    const row = findBranchRow(page, "ABC-123")
    await expect(row).toHaveAttribute("data-state", "closed")
    const target = row.getByTestId("lane-node-target")
    await target.hover()
    const details = page.getByTestId("lane-node-details")
    await expect(details).toBeVisible()
    await target.click()
    await expect(row).toHaveAttribute("data-state", "open")
    // the hover closes when the pointer leaves: the click did not pin it
    await page.mouse.move(0, 0)
    await expect(details).toHaveCount(0)
  })
})
