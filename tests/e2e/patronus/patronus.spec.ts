// @spec spec/branch/patronus.spec.md
import type { Locator, Page, Route } from "@playwright/test"
import { test as baseTest, expect } from "../fixtures/test-fixtures"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { expectNothingToPush, findBranchRow, verifyBranchState } from "../helpers/branch-helpers"
import { availableRobots, robot, unavailableRobots } from "../helpers/patronus-helpers"
import { getOpenedUrls } from "../helpers/pull-request-helpers"
import { EARLIER_COMMIT, hoverStatusChip, openBranchPreview, openOutgoingOverview, pushBranchFromRow, switchBranchPreview } from "../helpers/review-status-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"

// the `issue_links` template: 7 branches, none pushed; the page's clock is at 2024-01-02 10:00Z, the robots start 14 hours
// before, and the merged one took an hour: the usual time of a Safe Push
const NOW = 1704189600
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
    // the branch changed since the canceled robot of feature-api: its slot offers a new Safe Push
    await expect(page.getByTestId("patronus-chip")).toHaveCount(4)
    await use(repo)
  }, { auto: true }],
})

function getChip(page: Page, branchName: string): Locator {
  return findBranchRow(page, branchName).getByTestId("patronus-chip")
}

function hoverChip(page: Page, branchName: string): Promise<Locator> {
  return hoverStatusChip(page, getChip(page, branchName), "patronus-popover")
}

function getSlot(page: Page, branchName: string): Locator {
  return findBranchRow(page, branchName).getByTestId("safe-push-slot")
}

test.describe("Patronus chip", () => {
  test("should show each robot state in the Safe Push slot", async ({ page }) => {
    // the live robot: its time, and the usual time of a Safe Push once it runs longer
    const running = getChip(page, "JIRA-456")
    await expect(running).toHaveText("14h · usually ~1h")
    await expect(running).toHaveAccessibleName("Safe Push: Running, 14 hours, longer than usual")
    await expect(running).toHaveAttribute("data-kind", "running")
    await expect(running.getByTestId("robot-ring")).toBeVisible()
    // past the usual time the progress line under the row runs across
    await expect(findBranchRow(page, "JIRA-456").locator("xpath=following-sibling::tr[1]").getByTestId("robot-progress")).toHaveAttribute("data-indeterminate", "true")
    // the Status cell has no robot
    await expect(findBranchRow(page, "JIRA-456").getByTestId("branch-status")).toHaveText("not pushed")

    await expect(getChip(page, "ABC-123")).toHaveAttribute("data-kind", "failed")
    await expect(getChip(page, "ABC-123")).toHaveAttribute("data-color", "error")
    await expect(getChip(page, "feature-db")).toHaveText("Merged")
    await expect(getChip(page, "feature-db")).toHaveAttribute("data-color", "info")
    await expect(getChip(page, "feature-docs")).toHaveAccessibleName("Dry Run: Dry run passed")
    await expect(getChip(page, "feature-docs")).toHaveAttribute("data-color", "success")
    // tests the local tip: no warning dot
    await expect(running.getByTestId("patronus-outdated-dot")).toHaveCount(0)

    // the branch changed since its robot: a new Safe Push, the robot is in the preview
    await expect(getSlot(page, "feature-api")).toHaveAttribute("data-slot-kind", "start")
    await expect(getSlot(page, "feature-api").getByRole("button", { name: "Safe Push…" })).toBeEnabled()
    await expect(getSlot(page, "feature-auth")).toHaveAttribute("data-slot-kind", "start")
    // the slot has one width in every row: the push buttons and "⋯" line up
    const widths = await page.getByTestId("safe-push-slot").evaluateAll(slots => slots.map(slot => slot.getBoundingClientRect().width))
    expect(new Set(widths).size).toBe(1)
  })

  test("should offer a new Safe Push of a failed robot on the row hover, the state staying on its shield", async ({ page }) => {
    const row = findBranchRow(page, "ABC-123")
    const restart = getSlot(page, "ABC-123").getByRole("button", { name: "Safe Push…" })
    await expect(restart).toBeHidden()
    await row.locator("td").nth(1).hover()
    await expect(restart).toBeVisible()
    await expect(getChip(page, "ABC-123")).toHaveAccessibleName("Safe Push: Failed")
    // a merged branch has nothing to start
    await findBranchRow(page, "feature-db").locator("td").nth(1).hover()
    await expect(getSlot(page, "feature-db").getByRole("button", { name: "Safe Push…" })).toHaveCount(0)
  })

  test("should show the details on hover and open the robot on click", async ({ page }) => {
    const popover = await hoverChip(page, "JIRA-456")
    await expect(popover.getByTestId("patronus-name")).toHaveText("JIRA-456 fix the database connection pool")
    await expect(popover.getByTestId("patronus-state")).toHaveText("Running")
    await expect(popover.getByTestId("patronus-target")).toHaveText("master")
    // the profile of JIRA-456 is not known (a list doesn't name it): the rough estimate of the push mode
    await expect(popover.getByTestId("patronus-started")).toHaveText("Yesterday 9:00 PM · usually ~1 h for Safe Push · 14 h, longer than usual")

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
    // the row offers a new Safe Push; the preview has the robot
    await expect(getSlot(page, "feature-api")).toHaveAttribute("data-slot-kind", "start")
    // the remote doesn't have the change: a restart would test the commit the remote has, so the push comes first
    let preview = await openBranchPreview(page, "feature-api")
    let details = preview.getByTestId("branch-preview-patronus")
    await expect(preview.getByTestId("patronus-chip").getByTestId("patronus-outdated-dot")).toBeVisible()
    await expect(details.getByTestId("patronus-outdated")).toHaveText(/^Started on 01234567; the branch has changed since \(now [0-9a-f]{8}\): the result won't cover the newest commits\. Push and Restart Safe Push to test the current branch\.$/)
    await expect(details.getByRole("button", { name: "Push and Restart Safe Push" })).toBeVisible()
    await expect(details.getByRole("button", { name: "Restart Safe Push", exact: true })).toHaveCount(0)
    await expect(details.getByTestId("patronus-cancellation-reason")).toHaveText("Canceled by the user")
    await page.keyboard.press("Escape")
    await expect(preview).toHaveCount(0)

    // pushed since the robot started: a restart takes the pushed commit
    await pushBranchFromRow(page, "feature-api")
    await expect(getSlot(page, "feature-api")).toHaveAttribute("data-slot-kind", "start")
    // pushed: the button gave way to its empty place
    await expectNothingToPush(findBranchRow(page, "feature-api"))
    await findBranchRow(page, "feature-api").focus()
    preview = await openBranchPreview(page, "feature-api")
    details = preview.getByTestId("branch-preview-patronus")
    await expect(details.getByTestId("patronus-outdated")).toHaveText(/: the result won't cover the newest commits\. Restart Safe Push to test the current branch\.$/)
    await expect(details.getByRole("button", { name: "Restart Safe Push" })).toBeVisible()
    await expect(details.getByRole("button", { name: "Push and Restart Safe Push" })).toHaveCount(0)
  })

  test("should say only quietly that a robot started on an earlier build of the same commits", async ({ page }) => {
    // the remote branch is at the robot's commit and has the commits of the branch, which the sync rebuilt on a newer baseline
    await pushBranchFromRow(page, "feature-api", { remoteTip: EARLIER_COMMIT })
    // the robot of the branch as it is: back in the slot, without the dot
    await expect(getChip(page, "feature-api")).toHaveAttribute("data-kind", "canceled")
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

baseTest.describe("Patronus robot timing", () => {
  baseTest("should read the estimate of the CI only while the details of a running robot are open", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // two Safe Pushes of 40 minutes this week; JIRA-456 started 18 minutes ago, its Unit Tests have 9 minutes left
    const history = ["a", "b"].map(id => robot(`old-${id}`, `old-${id}`, { status: "success", sourceRevision: EARLIER_COMMIT, startTime: NOW - 86400, finishTime: NOW - 86400 + 2400 }))
    const repo = await setupRepo("issue_links", { patronus: availableRobots(robot("JIRA-456", "running-robot", { startTime: NOW - 18 * 60 }), ...history) })
    await repo.setPatronusProgress({ "running-robot": { remainingSeconds: 540, checkName: "Unit Tests", checkPercent: 93, overtimeSeconds: null, queued: 1, probablyHanging: false } })
    await syncAndWaitForBranches()
    await expect(page.getByTestId("unassigned-commits-section")).toBeVisible()
    const chip = getChip(page, "JIRA-456")
    await expect(chip).toHaveText("18m · ~22m left")
    // no details open: the row, the indicator, the Dock and the timers ask nothing
    await page.clock.fastForward("02:00")
    await expect(chip).toHaveText("20m · ~20m left")
    expect((await repo.getPatronusCalls()).detailCalls).toBe(0)

    const popover = await hoverChip(page, "JIRA-456")
    await expect(popover.getByTestId("patronus-progress")).toHaveText("about 9 min left · Unit Tests 93% · 1 check queued")
    // the time left is the CI's: the start line keeps only the usual time
    await expect(popover.getByTestId("patronus-started")).toHaveText(/ · usually ~40 min for Safe Push$/)
    // while they are open the slot takes it
    await expect(chip).toHaveText("20m · ~9m left")
    expect((await repo.getPatronusCalls()).detailCalls).toBe(1)
    // asked again once a minute while open
    await page.clock.fastForward("01:01")
    await expect.poll(async () => (await repo.getPatronusCalls()).detailCalls).toBe(2)
    await page.keyboard.press("Escape")
    await expect(popover).toBeHidden()
    await page.mouse.move(0, 0)

    // closed: the estimate from history again, and nothing asked
    await expect(chip).toHaveText("21m · ~19m left")
    await page.clock.fastForward("03:00")
    expect((await repo.getPatronusCalls()).detailCalls).toBe(2)

    // the list of the robots indicator asks while it is open
    await page.getByTestId("robots-indicator").hover()
    const row = page.getByTestId("robots-popover").locator("[data-testid=robots-row][data-branch-name=JIRA-456]")
    await expect(row.getByTestId("robots-row-checks")).toHaveText("Unit Tests 93% · 1 check queued")
    await expect(row.getByTestId("robots-row-state")).toHaveText("Safe Push · 24m · ~9m left")
    expect((await repo.getPatronusCalls()).detailCalls).toBe(3)
  })

  baseTest("should estimate by the merge profile, roughly by the push mode without it, and ask Patronus nothing for it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const LSC = { id: "lsc", name: "Large Scale Change" }
    // three Large Scale Changes of 110 minutes and two Safe Pushes without a profile of 40, of branches merged since
    const finished = (id: string, minutes: number, profile: { id: string | null, name: string | null }) =>
      robot(`old-${id}`, `old-${id}`, { status: "success", sourceRevision: EARLIER_COMMIT, startTime: NOW - 86400, finishTime: NOW - 86400 + minutes * 60, profile })
    const history = [finished("a", 110, LSC), finished("b", 110, LSC), finished("c", 110, LSC), finished("d", 40, { id: null, name: null }), finished("e", 40, { id: null, name: null })]
    const repo = await setupRepo("issue_links", {
      patronus: availableRobots(
        robot("JIRA-456", "lsc-robot", { startTime: NOW - 18 * 60, profile: LSC }),
        // a list doesn't name its profile
        robot("ABC-123", "unknown-robot", { startTime: NOW - 18 * 60 }),
        ...history,
      ),
    })
    await syncAndWaitForBranches()
    // the unassigned commits card renders after the branches and would move the rows away from the pointer
    await expect(page.getByTestId("unassigned-commits-section")).toBeVisible()
    await expect(getChip(page, "JIRA-456")).toHaveText("18m · ~1h 32m left")
    const calls = await repo.getPatronusCalls()

    let popover = await hoverChip(page, "JIRA-456")
    await expect(popover.getByTestId("patronus-started")).toContainText("usually ~1 h 50 min for Large Scale Change · about 1 h 32 min left")
    await expect(popover.getByTestId("patronus-profile-name")).toHaveText("Large Scale Change")
    await page.keyboard.press("Escape")
    await expect(popover).toBeHidden()
    await page.mouse.move(0, 0)
    popover = await hoverChip(page, "ABC-123")
    // the rough estimate names the push mode: the median of all five
    await expect(popover.getByTestId("patronus-started")).toContainText("usually ~1 h 50 min for Safe Push")
    await page.keyboard.press("Escape")
    await expect(popover).toBeHidden()

    // the list of the robots indicator names the profile too
    await page.getByTestId("robots-indicator").hover()
    const row = page.getByTestId("robots-popover").locator("[data-testid=robots-row][data-branch-name=JIRA-456]")
    await expect(row.getByTestId("robots-row-state")).toHaveText("Safe Push · Large Scale Change · 18m · ~1h 32m left")
    // no list and no poll for the estimates, only the details that were open
    expect(await repo.getPatronusCalls()).toMatchObject({ listCalls: calls.listCalls, pollCalls: calls.pollCalls })
  })

  baseTest("should count the time of a running robot and the time it usually takes", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // two Safe Pushes of 40 minutes this week, of branches merged since; JIRA-456 started 18 minutes ago
    const history = ["a", "b"].map(id => robot(`old-${id}`, `old-${id}`, { status: "success", sourceRevision: EARLIER_COMMIT, startTime: NOW - 86400, finishTime: NOW - 86400 + 2400 }))
    await setupRepo("issue_links", { patronus: availableRobots(robot("JIRA-456", "running-robot", { startTime: NOW - 18 * 60 }), ...history) })
    await syncAndWaitForBranches()
    const chip = getChip(page, "JIRA-456")
    await expect(chip).toHaveText("18m · ~22m left")
    await expect(chip).toHaveAccessibleName("Safe Push: Running, 18 minutes, about 22 minutes left")
    const progress = findBranchRow(page, "JIRA-456").locator("xpath=following-sibling::tr[1]").getByTestId("robot-progress")
    await expect(progress).toHaveAttribute("aria-valuenow", "45")

    // only the clock moves: the time ticks without a request to Patronus
    await page.clock.fastForward("02:00")
    await expect(chip).toHaveText("20m · ~20m left")
    await expect(progress).toHaveAttribute("aria-valuenow", "50")

    // past the usual time: the usual time instead of the time left, the progress line runs across
    await page.clock.fastForward("25:00")
    await expect(chip).toHaveText("45m · usually ~40m")
    await expect(progress).toHaveAttribute("data-indeterminate", "true")
  })
})

test.describe("Patronus in the outgoing overview", () => {
  test("should show the chips in a column, and leave a robot that failed on the branch tip to the robots indicator", async ({ page }) => {
    // the robots indicator tells the failed robot, once: no dot here
    const indicator = page.getByTestId("outgoing-indicator")
    await expect(indicator).toHaveAccessibleName("12 commits to push in 7 branches")
    await expect(indicator.getByTestId("outgoing-attention-dot")).toHaveCount(0)
    await expect(page.getByTestId("robots-indicator")).toHaveAccessibleName("Patronus robots: 1 running, 1 failed")
    const popover = await openOutgoingOverview(page)
    await expect(popover.getByTestId("outgoing-attention")).toHaveCount(0)
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

    // the second action opens the robot, the toast stays for Fetch
    await toast.getByRole("button", { name: "Open in Patronus" }).click()
    expect(await getOpenedUrls(page)).toEqual(["https://patronus.labs.jb.gg/robot/running-robot"])
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
