// @spec spec/repository/robots-indicator.spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, verifyBranchState } from "../helpers/branch-helpers"
import { robot, setupRobots } from "../helpers/patronus-helpers"
import { getOpenedUrls } from "../helpers/pull-request-helpers"
import { EARLIER_COMMIT } from "../helpers/review-status-helpers"
import { routeCommand } from "../helpers/wait-helpers"

// the page's clock is at 2024-01-02 10:00Z
const NOW = 1704189600
const minutes = (count: number) => count * 60

// JIRA-456 runs for 18 minutes; a Safe Push usually takes 40 (the merged robot of feature-db and one of a branch merged
// and archived since)
const RUNNING = robot("JIRA-456", "running-robot", { startTime: NOW - minutes(18) })
const FAILED = robot("ABC-123", "failed-robot", { status: "failure", startTime: NOW - minutes(80), finishTime: NOW - minutes(50) })
const MERGED = robot("feature-db", "merged-robot", { status: "success", startTime: NOW - minutes(120), finishTime: NOW - minutes(80) })
const ARCHIVED = robot("billing", "archived-robot", { status: "success", sourceRevision: EARLIER_COMMIT, startTime: NOW - minutes(200), finishTime: NOW - minutes(160) })
// a week ago: not of the last day
const OLD = robot("feature-docs", "old-robot", { status: "canceled", startTime: NOW - minutes(3000), finishTime: NOW - minutes(2990) })

async function openRobots(page: Page): Promise<Locator> {
  await page.getByTestId("robots-indicator").hover()
  const popover = page.getByTestId("robots-popover")
  await expect(popover).toBeVisible()
  return popover
}

/** What the page set in the Dock, in order (`__e2eDockCalls` of the Tauri mock) */
function getDockCalls(page: Page): Promise<Record<string, unknown>[]> {
  return page.evaluate(() => (window as unknown as { __e2eDockCalls?: Record<string, unknown>[] }).__e2eDockCalls ?? [])
}

test.describe("Robots indicator", () => {
  test("should show what runs and what failed in the header, and list the robots of the last day", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches, RUNNING, FAILED, MERGED, ARCHIVED, OLD)
    const indicator = page.getByTestId("robots-indicator")
    await expect(indicator).toHaveAccessibleName("Patronus robots: 1 running, 1 failed")
    await expect(indicator.getByTestId("robot-ring")).toBeVisible()
    await expect(indicator.getByTestId("robots-failed")).toHaveText("· 1 failed")
    // the failed robot is told here, not with the dot of the outgoing indicator
    await expect(page.getByTestId("outgoing-indicator").getByTestId("outgoing-attention-dot")).toHaveCount(0)

    const popover = await openRobots(page)
    const running = popover.getByTestId("robots-running").getByTestId("robots-row")
    await expect(running).toHaveCount(1)
    await expect(running.getByTestId("robots-row-state")).toHaveText("Safe Push · 18m · ~22m left")
    await expect(running.getByTestId("robot-progress")).toHaveAttribute("aria-valuenow", "45")
    await expect(popover.getByTestId("robots-attention").getByTestId("robots-row")).toHaveAttribute("data-branch-name", "ABC-123")
    await expect(popover.getByTestId("robots-attention").getByTestId("robots-row-state")).toHaveText("Safe Push · Failed · 50 minutes ago")
    // the others that finished in the last day, newest first, a branch archived since without a row to show
    const finished = popover.getByTestId("robots-finished").getByTestId("robots-row")
    await expect(finished).toHaveCount(2)
    await expect(finished.nth(0)).toHaveAttribute("data-branch-name", "feature-db")
    await expect(finished.nth(0).getByTestId("robots-row-state")).toHaveText("Safe Push · Merged · 1 hour ago")
    await expect(finished.nth(1)).toHaveAttribute("data-branch-name", "billing")
    await expect(finished.nth(1).getByRole("button", { name: "billing", exact: true })).toHaveCount(0)

    await finished.nth(0).getByRole("button", { name: "Open Safe Push of feature-db in Patronus" }).click()
    expect(await getOpenedUrls(page)).toEqual([MERGED.url])

    // a branch name shows its row
    await running.getByRole("button", { name: "JIRA-456", exact: true }).click()
    await expect(popover).toBeHidden()
    await verifyBranchState(findBranchRow(page, "JIRA-456"), "open")
  })

  test("should cancel a running robot from the list", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches, RUNNING)
    const cancels = await routeCommand(page, "cancel_patronus_robot", () => ({ ...RUNNING, status: "canceling" }))
    const popover = await openRobots(page)
    await popover.getByRole("button", { name: "Cancel Safe Push of JIRA-456" }).click()
    expect(cancels).toEqual([expect.objectContaining({ robotId: "running-robot" })])
    await expect(page.locator("[data-slot=toast]", { hasText: "Safe Push is canceling" })).toBeVisible()
    await expect(popover.getByTestId("robots-running").getByTestId("robots-row-state")).toHaveText("Safe Push · Canceling")
  })

  test("should restart a failed robot from the list, and open the pull request it was started from", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const pullRequest = { number: 7, url: "https://github.com/acme/monorepo/pull/7" }
    await setupRobots(page, setupRepo, syncAndWaitForBranches, { ...FAILED, pullRequest })
    const restarts = await routeCommand(page, "restart_patronus_robot", () => robot("ABC-123", "restarted-robot", { startTime: NOW }))
    const popover = await openRobots(page)
    const row = popover.getByTestId("robots-attention").getByTestId("robots-row")
    // the actions of the branch menu and the robot details, in their order and under their names
    await expect(row.getByRole("button", { name: "Restart Safe Push of ABC-123" })).toBeVisible()
    expect(await row.locator("button[aria-label]").evaluateAll(buttons => buttons.map(it => it.getAttribute("aria-label")))).toEqual([
      "Restart Safe Push of ABC-123",
      "Open Safe Push of ABC-123 in Patronus",
      "Open Pull Request #7 of ABC-123",
    ])

    await row.getByRole("button", { name: "Open Pull Request #7 of ABC-123" }).click()
    expect(await getOpenedUrls(page)).toEqual([pullRequest.url])
    await row.getByRole("button", { name: "Restart Safe Push of ABC-123" }).click()
    expect(restarts).toEqual([expect.objectContaining({ robotId: "failed-robot" })])
    await expect(page.locator("[data-slot=toast]", { hasText: "Safe Push restarted" })).toBeVisible()
  })

  test("should be hidden while nothing runs and nothing failed", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // fixed since it failed: nothing to act on
    await setupRobots(page, setupRepo, syncAndWaitForBranches, MERGED, { ...FAILED, sourceRevision: EARLIER_COMMIT })
    await expect(findBranchRow(page, "feature-db").getByTestId("patronus-chip")).toHaveText("Merged")
    await expect(page.getByTestId("robots-indicator")).toHaveCount(0)
  })
})

test.describe("Dock", () => {
  // the user agent of the Desktop Chrome device is the one of Windows, which has no badge count
  test.use({ userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36" })

  test("should show the progress of the robot nearest to finishing, and a badge only for what needs the user", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches, RUNNING, FAILED, MERGED)
    await expect.poll(() => getDockCalls(page)).toEqual([{ progressBar: { status: "normal", progress: 45 } }, { badgeCount: 1 }])

    // only the clock moves: the progress follows it, the badge is not set again
    await page.clock.fastForward("02:00")
    await expect.poll(async () => (await getDockCalls(page)).at(-1)).toEqual({ progressBar: { status: "normal", progress: 50 } })
    expect((await getDockCalls(page)).filter(call => "badgeCount" in call)).toHaveLength(1)
  })

  test("should show an indeterminate progress without history, and no badge for a running robot", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches, RUNNING)
    await expect.poll(() => getDockCalls(page)).toEqual([{ progressBar: { status: "indeterminate", progress: 0 } }])
  })
})

test.describe("Taskbar", () => {
  test("should show the progress, without a badge on Windows", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRobots(page, setupRepo, syncAndWaitForBranches, RUNNING, FAILED, MERGED)
    await expect.poll(() => getDockCalls(page)).toEqual([{ progressBar: { status: "normal", progress: 45 } }])
    await expect(page.getByTestId("robots-indicator")).toHaveAccessibleName("Patronus robots: 1 running, 1 failed")
    expect(await getDockCalls(page)).toHaveLength(1)
  })
})
