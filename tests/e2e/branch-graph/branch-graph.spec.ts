// @spec spec/branch/branch-row.spec.md
import { test, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { findBranchRow } from "../helpers/branch-helpers"
import { availableRobots } from "../helpers/patronus-helpers"

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
    await expect(page.getByRole("tooltip")).toHaveText("Not on the remote yet: push it")
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
    await expect(page.getByRole("tooltip")).toHaveText("Safe Push lands 7 branches: Patronus checks the approvals when the robot starts")
    // the node of its root row says it of the branch alone
    await page.mouse.move(0, 0)
    await findBranchRow(page, "air").getByTestId("lane-node-target").hover()
    await expect(page.getByRole("tooltip").filter({ hasText: "Safe Push:" }).last()).toHaveText("Safe Push: Patronus checks the approvals when the robot starts")
  })
})
