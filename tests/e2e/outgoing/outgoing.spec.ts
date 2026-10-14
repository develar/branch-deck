// @spec spec/repository/outgoing.spec.md
import type { Locator, Page } from "@playwright/test"
import { test as baseTest, testWithIssueLinks as test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, getPushButton, verifyBranchState } from "../helpers/branch-helpers"

// the `issue_links` template: 7 branches, none pushed; JIRA-456 has 3 commits, ABC-123 4, the others 1
const BRANCH_COUNT = 7
const TOTAL = 12

function getIndicator(page: Page): Locator {
  return page.getByTestId("outgoing-indicator")
}

async function openOverview(page: Page): Promise<Locator> {
  await getIndicator(page).hover()
  const popover = page.getByTestId("outgoing-popover")
  await expect(popover).toBeVisible()
  return popover
}

function getOverviewRow(popover: Locator, name: string): Locator {
  return popover.locator(`[data-testid="outgoing-branch"][data-branch-name="${name}"]`)
}

interface PushResult {
  unpushedCommits?: string[]
  commitsBehind?: number
}

/** Answers `push_branch` in the page (the E2E server has no remote): the remote status after the push. */
async function routePush(page: Page, results: Record<string, PushResult>) {
  await page.route("**/invoke/push_branch", async (route) => {
    const branchName = (route.request().postDataJSON() as { branchName: string }).branchName
    const result = results[branchName] ?? {}
    const unpushedCommits = result.unpushedCommits ?? []
    await route.fulfill({
      json: {
        branchName,
        remoteExists: true,
        unpushedCommits,
        commitsBehind: result.commitsBehind ?? 0,
        myUnpushedCount: unpushedCommits.length,
        lastPushTime: 1704139200,
      },
    })
  })
}

test.describe("Outgoing indicator", () => {
  test("should show the commits to push with a bar segment per branch", async ({ page }) => {
    const indicator = getIndicator(page)
    await expect(indicator).toHaveAccessibleName(`${TOTAL} commits to push in ${BRANCH_COUNT} branches`)
    await expect(indicator.getByTestId("outgoing-total")).toHaveText(String(TOTAL))
    const segments = indicator.getByTestId("outgoing-bar").locator("[data-state]")
    await expect(segments).toHaveCount(BRANCH_COUNT)
    // nothing is pushed yet: every push creates the remote branch
    await expect(indicator.getByTestId("outgoing-bar").locator("[data-state=create]")).toHaveCount(BRANCH_COUNT)
  })

  test("should list the outgoing branches in the overview on hover", async ({ page }) => {
    const popover = await openOverview(page)
    await expect(popover.getByRole("heading")).toHaveText(`${TOTAL} commits to push in ${BRANCH_COUNT} branches`)
    const rows = popover.getByTestId("outgoing-branch")
    await expect(rows).toHaveCount(BRANCH_COUNT)
    // the order of the branch table
    const tableOrder = await page.getByTestId("branch-row").evaluateAll(rows => rows.map(row => row.getAttribute("data-branch-name")))
    expect(await rows.evaluateAll(rows => rows.map(row => row.getAttribute("data-branch-name")))).toEqual(tableOrder)

    const row = getOverviewRow(popover, "JIRA-456")
    await expect(row).toContainText("3 (3)↑")
    await expect(row).toContainText("new")
    await expect(row.getByRole("button", { name: "Create Remote" })).toBeEnabled()
    await expect(popover.getByTestId("outgoing-legend")).toHaveText("new remote")
  })

  test("should show and expand the row of a branch clicked in the overview", async ({ page }) => {
    const popover = await openOverview(page)
    await verifyBranchState(findBranchRow(page, "ABC-123"), "closed")

    await getOverviewRow(popover, "ABC-123").getByRole("button", { name: "ABC-123", exact: true }).click()

    await expect(popover).toBeHidden()
    await verifyBranchState(findBranchRow(page, "ABC-123"), "open")
    await expect(findBranchRow(page, "ABC-123")).toBeInViewport()
  })

  test("should push a branch from the overview and follow the push results", async ({ page }) => {
    await routePush(page, {
      // the remote had a commit the branch no longer has
      "ABC-123": { unpushedCommits: ["a1"], commitsBehind: 1 },
    })
    const popover = await openOverview(page)

    await getOverviewRow(popover, "JIRA-456").getByRole("button", { name: "Create Remote" }).click()
    await expect(page.getByText("Branch JIRA-456 pushed successfully")).toBeVisible()
    // pushed: gone from the overview, the popover stays open
    await expect(getOverviewRow(popover, "JIRA-456")).toHaveCount(0)
    await expect(popover.getByRole("heading")).toHaveText(`${TOTAL - 3} commits to push in ${BRANCH_COUNT - 1} branches`)

    const row = getOverviewRow(popover, "ABC-123")
    await row.getByRole("button", { name: "Create Remote" }).click()
    await expect(row).toHaveAttribute("data-state", "force")
    await expect(row).toContainText("1 (1)↑ · 1↓")
    await expect(row).toContainText("force")
    await expect(row.getByRole("button", { name: "Force Push (1)" })).toBeEnabled()
    await expect(popover.getByTestId("outgoing-legend")).toHaveText("new remoteforce push")

    const indicator = getIndicator(page)
    await expect(indicator.getByTestId("outgoing-total")).toHaveText(String(TOTAL - 3 - 4 + 1))
    await expect(indicator.getByTestId("outgoing-bar").locator("[data-state=force]")).toHaveCount(1)
  })
})

baseTest.describe("Outgoing indicator without outgoing commits", () => {
  baseTest("should hide when every branch is pushed", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("simple")
    await syncAndWaitForBranches()
    await routePush(page, {})
    const indicator = getIndicator(page)
    await expect(indicator).toHaveAccessibleName("1 commit to push in 1 branch")

    await getPushButton(findBranchRow(page, "test-branch")).click()

    await expect(page.getByText("Branch test-branch pushed successfully")).toBeVisible()
    await expect(indicator).toHaveCount(0)
  })
})
