// @spec spec/branch/patronus.spec.md
import type { Locator, Page, Route } from "@playwright/test"
import { test as baseTest, expect } from "../fixtures/test-fixtures"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { findBranchRow, verifyBranchState } from "../helpers/branch-helpers"
import { availableRobots, robot, unavailableRobots } from "../helpers/patronus-helpers"
import { getOpenedUrls } from "../helpers/pull-request-helpers"
import { EARLIER_COMMIT, hoverStatusChip, openBranchPreview, openOutgoingOverview, openOverviewWithAttention, pushBranchFromRow, switchBranchPreview } from "../helpers/review-status-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"

// the `issue_links` template: 7 branches, none pushed
const RUNNING = robot("JIRA-456", "running-robot", { name: "JIRA-456 fix the database connection pool" })
const FAILED = robot("ABC-123", "failed-robot", { status: "failure", finishTime: 1704142800 })
const MERGED = robot("feature-db", "merged-robot", { status: "success", finishTime: 1704142800 })
const DRY_RUN = robot("feature-docs", "dry-run-robot", { status: "success", pushMode: "dryRun", finishTime: 1704142800 })
// tested a commit the local branch no longer is at, and the remote doesn't have (the branch is not pushed)
const OUTDATED = robot("feature-api", "outdated-robot", { status: "canceled", sourceRevision: EARLIER_COMMIT, cancellationReason: "Canceled by the user" })
const ALL = availableRobots(RUNNING, FAILED, MERGED, DRY_RUN, OUTDATED)

const test = baseTest.extend<{ repo: TestRepositoryBuilder }>({
  repo: [async ({ page, setupRepo, syncAndWaitForBranches }, use) => {
    const repo = await setupRepo("issue_links", { patronus: ALL })
    await syncAndWaitForBranches()
    // the unassigned commits card can render after the branches and move the rows away from the pointer
    await expect(page.getByTestId("unassigned-commits-section")).toBeVisible()
    await expect(page.getByTestId("patronus-chip")).toHaveCount(5)
    await use(repo)
  }, { auto: true }],
})

function getChip(page: Page, branchName: string): Locator {
  return findBranchRow(page, branchName).getByTestId("patronus-chip")
}

function hoverChip(page: Page, branchName: string): Promise<Locator> {
  return hoverStatusChip(page, getChip(page, branchName), "patronus-popover")
}

test.describe("Patronus chip", () => {
  test("should show each robot state in the status cell", async ({ page }) => {
    const running = getChip(page, "JIRA-456")
    await expect(running).toHaveText("Running")
    await expect(running).toHaveAccessibleName("Safe Push: Running")
    await expect(running).toHaveAttribute("data-color", "neutral")

    await expect(getChip(page, "ABC-123")).toHaveAttribute("data-kind", "failed")
    await expect(getChip(page, "ABC-123")).toHaveAttribute("data-color", "error")
    await expect(getChip(page, "feature-db")).toHaveText("Merged")
    await expect(getChip(page, "feature-db")).toHaveAttribute("data-color", "info")
    await expect(getChip(page, "feature-docs")).toHaveAccessibleName("Dry Run: Dry run passed")
    await expect(getChip(page, "feature-docs")).toHaveAttribute("data-color", "success")

    const outdated = getChip(page, "feature-api")
    await expect(outdated).toHaveAttribute("data-kind", "canceled")
    await expect(outdated.getByTestId("patronus-outdated-dot")).toBeVisible()
    // tests the local tip: no warning dot
    await expect(running.getByTestId("patronus-outdated-dot")).toHaveCount(0)

    await expect(getChip(page, "feature-auth")).toHaveCount(0)
  })

  test("should show the details on hover and open the robot on click", async ({ page }) => {
    const popover = await hoverChip(page, "JIRA-456")
    await expect(popover.getByTestId("patronus-name")).toHaveText("JIRA-456 fix the database connection pool")
    await expect(popover.getByTestId("patronus-state")).toHaveText("Running")
    await expect(popover.getByTestId("patronus-target")).toHaveText("master")

    await popover.getByRole("button", { name: "Open in Patronus" }).click()
    expect(await getOpenedUrls(page)).toEqual([RUNNING.url])
    await page.keyboard.press("Escape")
    await expect(popover).toBeHidden()

    // the row does not toggle
    await getChip(page, "ABC-123").click()
    expect(await getOpenedUrls(page)).toEqual([RUNNING.url, FAILED.url])
    await verifyBranchState(findBranchRow(page, "ABC-123"), "closed")
  })

  test("should say when the branch changed since the robot started, and how to test the branch as it is", async ({ page }) => {
    // the remote doesn't have the change: a restart would test the commit the remote has, so the push comes first
    let popover = await hoverChip(page, "feature-api")
    await expect(popover.getByTestId("patronus-outdated")).toHaveText(/^Started on 01234567; the branch has changed since \(now [0-9a-f]{8}\): the result won't cover the newest commits\. Push and restart to test the current branch\.$/)
    await expect(popover.getByRole("button", { name: "Push and Restart" })).toBeVisible()
    await expect(popover.getByRole("button", { name: "Restart Safe Push" })).toHaveCount(0)
    await expect(popover.getByTestId("patronus-cancellation-reason")).toHaveText("Canceled by the user")
    await page.keyboard.press("Escape")
    await expect(popover).toBeHidden()

    // pushed since the robot started: a restart takes the pushed commit
    await pushBranchFromRow(page, "feature-api")
    await expect(getChip(page, "feature-api").getByTestId("patronus-outdated-dot")).toBeVisible()
    popover = await hoverChip(page, "feature-api")
    await expect(popover.getByTestId("patronus-outdated")).toHaveText(/: the result won't cover the newest commits\. Restart to test the current branch\.$/)
    await expect(popover.getByRole("button", { name: "Restart Safe Push" })).toBeVisible()
    await expect(popover.getByRole("button", { name: "Push and Restart" })).toHaveCount(0)
  })

  test("should say only quietly that a robot started on an earlier build of the same commits", async ({ page }) => {
    // the remote branch is at the robot's commit and has the commits of the branch, which the sync rebuilt on a newer baseline
    await pushBranchFromRow(page, "feature-api", { remoteTip: EARLIER_COMMIT })
    await expect(getChip(page, "feature-api").getByTestId("patronus-outdated-dot")).toHaveCount(0)
    const popover = await hoverChip(page, "feature-api")
    await expect(popover.getByTestId("patronus-rebased")).toHaveText("Started on an earlier build of the same commits.")
    await expect(popover.getByTestId("patronus-outdated")).toHaveCount(0)
  })

  test("should show the robot in the branch preview", async ({ page }) => {
    const preview = await openBranchPreview(page, "ABC-123")
    await expect(preview.getByTestId("branch-preview-patronus").getByTestId("patronus-state")).toHaveText("Failed")
    await expect(preview.getByTestId("patronus-chip")).toHaveText("Failed")

    await switchBranchPreview(page, preview, "feature-auth")
    await expect(preview.getByTestId("branch-preview-patronus")).toHaveCount(0)
  })
})

test.describe("Patronus in the outgoing overview", () => {
  test("should show the chips in a column and a dot for a robot that failed on the branch tip", async ({ page }) => {
    // the overview says what the dot means
    const popover = await openOverviewWithAttention(page, "12 commits to push in 7 branches", "ABC-123 needs attention: failed Safe Push")
    // the shield only: the branch names need the room
    const chip = popover.locator("[data-testid=outgoing-branch][data-branch-name=ABC-123]").getByTestId("patronus-chip")
    await expect(chip).toHaveAccessibleName("Safe Push: Failed")
    await expect(chip).toHaveText("")
    await expect(popover.locator("[data-testid=outgoing-branch][data-branch-name=feature-auth]").getByTestId("patronus-chip")).toHaveCount(0)
    await expect(popover.getByTestId("patronus-status-line")).toContainText("Patronus checked just now")
  })

  test("should list the robots again from the overview", async ({ page, repo }) => {
    const { listCalls } = await repo.getPatronusCalls()
    const popover = await openOutgoingOverview(page)
    await popover.getByTestId("patronus-refresh").click()
    await expect.poll(async () => (await repo.getPatronusCalls()).listCalls).toBe(listCalls + 1)
    await expect(popover.getByTestId("patronus-refresh")).toBeEnabled()
  })
})

test.describe("Patronus merge", () => {
  test("should tell when a running Safe Push merged, and fetch the baseline on request", async ({ page, repo }) => {
    await repo.setPatronus(availableRobots({ ...RUNNING, status: "success", finishTime: 1704142800 }, FAILED, MERGED, DRY_RUN, OUTDATED))
    const fetches: unknown[] = []
    await page.route("**/invoke/fetch_baseline", async (route: Route) => {
      fetches.push(route.request().postDataJSON())
      await route.fulfill({ json: { upstream: "origin/master", fetchedAt: 1704142900 } })
    })
    // a focus polls the running robot at once
    await emitTauriEvent(page, "tauri://focus")
    const toast = page.locator("[data-slot=toast]").filter({ hasText: "JIRA-456 merged into master" })
    await expect(toast).toContainText("Safe Push")
    await expect(getChip(page, "JIRA-456")).toHaveText("Merged")

    await toast.getByRole("button", { name: "Fetch" }).click()
    await expect.poll(() => fetches.length).toBe(1)
    await expect(toast).toHaveCount(0)
  })
})

baseTest.describe("Patronus unavailable", () => {
  baseTest("should hint quietly when the Safe Push CLI is not logged in", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_links", { patronus: unavailableRobots({ status: "notLoggedIn" }) })
    await syncAndWaitForBranches()
    const popover = await openOutgoingOverview(page)
    await expect(popover.getByTestId("patronus-status-line")).toContainText("Run ./safePush.cmd once to log in to Patronus")
    await expect(page.getByTestId("patronus-chip")).toHaveCount(0)
    // never a toast
    await expect(page.locator("[data-slot=toast]")).toHaveCount(0)
  })

  baseTest("should say nothing for a repository that is not on Space", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_links")
    await syncAndWaitForBranches()
    const popover = await openOutgoingOverview(page)
    await expect(popover.getByTestId("patronus-status-line")).toHaveCount(0)
    await expect(page.getByTestId("patronus-chip")).toHaveCount(0)
  })
})
