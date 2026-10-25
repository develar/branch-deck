// @spec spec/branch/patronus.spec.md
import type { Locator, Page } from "@playwright/test"
import type { PatronusRobotProfile } from "../../../src/lib/bindings"
import { test as baseTest, expect } from "../fixtures/test-fixtures"
import { openActivityWindow } from "../helpers/activity-helpers"
import { expectNothingToPush, findBranchRow, getBranchCells, getLandingNode, verifyBranchState } from "../helpers/branch-helpers"
import { readClipboard } from "../helpers/clipboard-helpers"
import { expectColumnAligned } from "../helpers/column-helpers"
import { INTENT_DWELL_PAST_MS, resetIpc, settledInvokes } from "../helpers/ipc-helpers"
import { availableRobots, buildsNeverStarted, robot, unavailableRobots } from "../helpers/patronus-helpers"
import { getOpenedUrls } from "../helpers/pull-request-helpers"
import { EARLIER_COMMIT, hoverStatusChip, landingRow, openBranchPreview, openLandingOverview, pushBranchFromRow, switchBranchPreview } from "../helpers/review-status-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { holdCommandCalls } from "../helpers/wait-helpers"

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
  repo: [
    async ({ page, setupRepo, syncAndWaitForBranches }, use) => {
      const repo = await setupRepo("issue_links", { patronus: ALL })
      await syncAndWaitForBranches()
      // the unassigned commits card can render after the branches and move the rows away from the pointer
      await expect(page.getByTestId("unassigned-commits-section")).toBeVisible()
      // the branch changed since the canceled robot of feature-api: its slot offers a new Safe Push
      await expect(page.getByTestId("patronus-chip")).toHaveCount(4)
      await use(repo)
    },
    { auto: true },
  ],
})

/** What the app invokes on its own timers, wherever they fire: the debounced writes of the settings, the check for updates after the start */
const APP_TIMERS: ReadonlySet<string> = new Set(["set_setting", "check_for_updates"])

/** The commands of the page but those of the app's own timers */
function withoutAppTimers(invokes: Record<string, number>): Record<string, number> {
  return Object.fromEntries(Object.entries(invokes).filter(([name]) => !APP_TIMERS.has(name)))
}

function getChip(page: Page, branchName: string): Locator {
  return findBranchRow(page, branchName).getByTestId("patronus-chip")
}

function hoverChip(page: Page, branchName: string): Promise<Locator> {
  return hoverStatusChip(page, getChip(page, branchName), "patronus-popover")
}

function getStatusCell(page: Page, branchName: string): Locator {
  return getBranchCells(findBranchRow(page, branchName)).status
}

function getSlot(page: Page, branchName: string): Locator {
  return findBranchRow(page, branchName).getByTestId("next-step-slot")
}

test.describe("Patronus chip", () => {
  test("should show each robot state in the Status cell, and the next step in the Safe Push slot", async ({ page }) => {
    // the live robot, in the Status cell: its time, and the usual time of a Safe Push once it runs longer
    const running = getChip(page, "JIRA-456")
    await expect(running).toHaveText("14h · usually ~1h")
    await expect(running).toHaveAccessibleName("Safe Push: Running, 14 hours, longer than usual")
    await expect(running).toHaveAttribute("data-kind", "running")
    await expect(running.getByTestId("robot-ring")).toBeVisible()
    await expect(getStatusCell(page, "JIRA-456").getByTestId("patronus-chip")).toHaveCount(1)
    // past the usual time the progress line under its time runs across
    await expect(getStatusCell(page, "JIRA-456").getByTestId("robot-progress")).toHaveAttribute("data-indeterminate", "true")
    // while it runs there is no step: Cancel is in "⋯" and the details
    await expect(getSlot(page, "JIRA-456")).toHaveAttribute("data-step", "none")

    await expect(getChip(page, "ABC-123")).toHaveAttribute("data-kind", "failed")
    await expect(getChip(page, "ABC-123")).toHaveAttribute("data-color", "error")
    await expect(getSlot(page, "ABC-123")).toHaveAttribute("data-step", "restart")
    await expect(getChip(page, "feature-db")).toHaveText("Merged")
    await expect(getChip(page, "feature-db")).toHaveAttribute("data-color", "info")
    // merged: nothing left to start
    await expect(getSlot(page, "feature-db")).toHaveAttribute("data-step", "none")
    await expect(getChip(page, "feature-docs")).toHaveAccessibleName("Dry Run: Dry run passed")
    await expect(getChip(page, "feature-docs")).toHaveAttribute("data-color", "success")
    await expect(getSlot(page, "feature-docs").getByRole("button", { name: "Safe Push", exact: true })).toBeEnabled()
    // tests the local tip: no warning dot
    await expect(running.getByTestId("patronus-outdated-dot")).toHaveCount(0)

    // the branch changed since its robot: the git status, a new Safe Push; the robot is in the preview
    await expect(getStatusCell(page, "feature-api").getByTestId("branch-status")).toHaveText("not pushed")
    await expect(getSlot(page, "feature-api")).toHaveAttribute("data-step", "safePush")
    await expect(getSlot(page, "feature-api").getByRole("button", { name: "Safe Push", exact: true })).toBeEnabled()
    await expect(getSlot(page, "feature-auth")).toHaveAttribute("data-step", "safePush")
    // the steps and "⋯" stand in their columns in every row, whatever the row shows before them
    await expectColumnAligned(page.getByTestId("next-step-slot"))
    await expectColumnAligned(page.getByTestId("branch-row").getByRole("button", { name: /^More actions for / }))
  })

  test("should restart a failed robot from its row, the state staying in the Status cell", async ({ page, repo }) => {
    const restart = getSlot(page, "ABC-123").getByRole("button", { name: "Restart Safe Push" })
    // no hover swaps it: the failure stays where the eye reads the state
    await expect(restart).toBeVisible()
    await findBranchRow(page, "ABC-123").locator("td").nth(1).hover()
    await expect(getChip(page, "ABC-123")).toHaveText("Failed")
    await restart.click()
    await expect.poll(async () => (await repo.getPatronusLog()).restarts).toEqual([FAILED.id])
    await verifyBranchState(findBranchRow(page, "ABC-123"), "closed")
  })

  test("should restart a robot once while its restart runs, running in the slot and in its details, landing from the click", async ({ page, repo }) => {
    const restarts = await holdCommandCalls(page, "restart_patronus_robot")
    const restart = getSlot(page, "ABC-123").getByRole("button", { name: "Restart Safe Push" })
    const node = getLandingNode(findBranchRow(page, "ABC-123"))
    await expect(node).toHaveAttribute("data-kind", "needsFix")
    await restart.click()
    await expect(restart).toHaveAttribute("aria-busy", "true")
    await expect(restart).toBeDisabled()
    // the restart is the move made: the branch lands before Patronus answers it
    await expect(node).toHaveAttribute("data-kind", "landing")
    // the details offer the same restart: it runs there too, a click starts no second one
    const popover = await hoverChip(page, "ABC-123")
    const again = popover.getByRole("button", { name: "Restart Safe Push" })
    await expect(again).toHaveAttribute("aria-busy", "true")
    await expect(again).toBeDisabled()
    await again.click({ force: true })

    restarts.release()
    await expect(page.getByText("Safe Push restarted", { exact: true })).toBeVisible()
    expect(restarts.calls).toHaveLength(1)
    expect((await repo.getPatronusLog()).restarts).toEqual([FAILED.id])
    await expect(node).toHaveAttribute("data-kind", "landing")
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
    await expect(getSlot(page, "feature-api")).toHaveAttribute("data-step", "safePush")
    // the remote doesn't have the change: a restart would test the commit the remote has, so the push comes first
    let preview = await openBranchPreview(page, "feature-api")
    let details = preview.getByTestId("branch-preview-patronus")
    await expect(preview.getByTestId("patronus-chip").getByTestId("patronus-outdated-dot")).toBeVisible()
    await expect(details.getByTestId("patronus-outdated")).toHaveText(
      /^Started on 01234567; the branch has changed since \(now [0-9a-f]{8}\): the result won't cover the newest commits\. Restart Safe Push to test the current branch\.$/,
    )
    // the branch changed here: the restart pushes first, under the restart's own name
    await expect(details.getByRole("button", { name: "Restart Safe Push", exact: true })).toHaveAttribute("data-action", "push-and-restart-robot")
    await expect(details.getByTestId("patronus-cancellation-reason")).toHaveText("Canceled by the user")
    await page.keyboard.press("Escape")
    await expect(preview).toHaveCount(0)

    // pushed since the robot started: a restart takes the pushed commit
    await pushBranchFromRow(page, "feature-api")
    await expect(getSlot(page, "feature-api")).toHaveAttribute("data-step", "safePush")
    // pushed: the button gave way to its empty place
    await expectNothingToPush(findBranchRow(page, "feature-api"))
    await findBranchRow(page, "feature-api").focus()
    preview = await openBranchPreview(page, "feature-api")
    details = preview.getByTestId("branch-preview-patronus")
    await expect(details.getByTestId("patronus-outdated")).toHaveText(/: the result won't cover the newest commits\. Restart Safe Push to test the current branch\.$/)
    // pushed: a plain restart takes the pushed commit
    await expect(details.getByRole("button", { name: "Restart Safe Push", exact: true })).toHaveAttribute("data-action", "restart-robot")
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

  test("should ask the session nothing for the hover and the menu of a row, and tell it only the details of the robot whose popover opens", async ({ page, repo }) => {
    const row = findBranchRow(page, "JIRA-456")
    // the hover reads the review of the branch ahead and no fact: the facts are the session's, which sends them
    await resetIpc(page)
    await row.hover()
    await page.clock.fastForward(INTENT_DWELL_PAST_MS)
    expect(withoutAppTimers(await settledInvokes(page))).toEqual({ prefetch_branch_review: 1 })

    await resetIpc(page)
    await row.click({ button: "right" })
    await expect(page.getByRole("menu")).toBeVisible()
    expect(withoutAppTimers(await settledInvokes(page))).toEqual({})
    await page.keyboard.press("Escape")
    await expect(page.getByRole("menu")).toHaveCount(0)

    // the popover tells the session what it shows, once, and the session reads the details
    await resetIpc(page)
    const calls = (await repo.getPatronusLog()).detailCalls
    await hoverChip(page, "JIRA-456")
    await expect.poll(async () => (await repo.getPatronusLog()).detailCalls).toBe(calls + 1)
    expect(withoutAppTimers(await settledInvokes(page))).toEqual({ set_demand: 1 })
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
  // two Safe Pushes of 40 minutes a day ago, of other branches
  const weekHistory = () =>
    ["a", "b"].map(id => robot(`old-${id}`, `old-${id}`, { status: "success", sourceRevision: EARLIER_COMMIT, startTime: NOW - 86400, finishTime: NOW - 86400 + 2400 }))

  baseTest("should read the estimate of the CI only while the details of a running robot are open", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // two Safe Pushes of 40 minutes this week; JIRA-456 started 18 minutes ago, its Unit Tests have 9 minutes left
    const history = weekHistory()
    // the new API of Settings → Patronus: Patronus folds the progress of the robot in one request
    const repo = await setupRepo("issue_links", {
      patronus: availableRobots(robot("JIRA-456", "running-robot", { startTime: NOW - 18 * 60 }), ...history),
      initialStoreValues: { appSettings: { patronusNewApi: true } },
    })
    await repo.setPatronusProgress({
      "running-robot": { remainingSeconds: 540, checkName: "Unit Tests", checkPercent: 93, overtimeSeconds: null, queued: 1, probablyHanging: false },
    })
    await syncAndWaitForBranches()
    await expect(page.getByTestId("unassigned-commits-section")).toBeVisible()
    const chip = getChip(page, "JIRA-456")
    await expect(chip).toHaveText("18m · ~22m")
    // no details open: the row, the indicator, the Dock and the timers of the page and of the session ask nothing
    await page.clock.fastForward("02:00")
    await repo.advanceBackendClock(120_000)
    await expect(chip).toHaveText("20m · ~20m")
    expect((await repo.getPatronusLog()).detailCalls).toBe(0)

    const popover = await hoverChip(page, "JIRA-456")
    await expect(popover.getByTestId("patronus-progress")).toHaveText("about 9 min left · Unit Tests 93% · 1 check queued")
    // the time left is the CI's: the start line keeps only the usual time
    await expect(popover.getByTestId("patronus-started")).toHaveText(/ · usually ~40 min for Safe Push$/)
    // while they are open the slot takes it
    await expect(chip).toHaveText("20m · ~9m")
    expect((await repo.getPatronusLog()).detailCalls).toBe(1)
    // the session asks them again once a minute of its clock while they are open
    await page.clock.fastForward("01:01")
    await repo.advanceBackendClock(61_000)
    await expect.poll(async () => (await repo.getPatronusLog()).detailCalls).toBe(2)
    await page.keyboard.press("Escape")
    await expect(popover).toBeHidden()
    await page.mouse.move(0, 0)

    // closed: the estimate from history again, the session dropped them and asks nothing more
    await expect(chip).toHaveText("21m · ~19m")
    await page.clock.fastForward("03:00")
    await repo.advanceBackendClock(180_000)
    expect((await repo.getPatronusLog()).detailCalls).toBe(2)

    // the robots of the landing overview asks while it is open
    await page.getByTestId("landing-indicator").hover()
    const row = page.getByTestId("landing-popover").locator("[data-testid=robots-row][data-branch-name=JIRA-456]")
    await expect(row.getByTestId("robots-row-checks")).toHaveText("Unit Tests 93% · 1 check queued")
    await expect(row.getByTestId("robots-row-state")).toHaveText("24m · ~9m")
    expect((await repo.getPatronusLog()).detailCalls).toBe(3)
  })

  baseTest("should estimate by the merge profile, roughly by the push mode without it, and ask Patronus nothing for it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const LSC = { id: "lsc", name: "Large Scale Change", skipsReview: null }
    // three Large Scale Changes of 110 minutes and two Safe Pushes without a profile of 40, of branches merged since
    const finished = (id: string, minutes: number, profile: PatronusRobotProfile) =>
      robot(`old-${id}`, `old-${id}`, { status: "success", sourceRevision: EARLIER_COMMIT, startTime: NOW - 86400, finishTime: NOW - 86400 + minutes * 60, profile })
    const history = [
      finished("a", 110, LSC),
      finished("b", 110, LSC),
      finished("c", 110, LSC),
      finished("d", 40, { id: null, name: null, skipsReview: null }),
      finished("e", 40, { id: null, name: null, skipsReview: null }),
    ]
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
    await expect(getChip(page, "JIRA-456")).toHaveText("18m · ~1h 32m")
    const calls = await repo.getPatronusLog()

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

    // the robots of the landing overview name the profile too, on hover of the time
    await page.getByTestId("landing-indicator").hover()
    const row = page.getByTestId("landing-popover").locator("[data-testid=robots-row][data-branch-name=JIRA-456]")
    await expect(row.getByTestId("robots-row-state")).toHaveText("18m · ~1h 32m")
    await row.getByTestId("robots-row-state").hover()
    await expect(page.getByRole("tooltip")).toHaveText("Safe Push · Large Scale Change")
    // no list and no poll for the estimates, only the details that were open
    expect(await repo.getPatronusLog()).toMatchObject({ listCalls: calls.listCalls, pollCalls: calls.pollCalls })
  })

  baseTest("should count the time of a running robot and the time it usually takes", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // two Safe Pushes of 40 minutes this week, of branches merged since; JIRA-456 started 18 minutes ago
    const history = weekHistory()
    await setupRepo("issue_links", { patronus: availableRobots(robot("JIRA-456", "running-robot", { startTime: NOW - 18 * 60 }), ...history) })
    await syncAndWaitForBranches()
    const chip = getChip(page, "JIRA-456")
    await expect(chip).toHaveText("18m · ~22m")
    await expect(chip).toHaveAccessibleName("Safe Push: Running, 18 minutes, about 22 minutes left")
    const progress = getStatusCell(page, "JIRA-456").getByTestId("robot-progress")
    await expect(progress).toHaveAttribute("aria-valuenow", "45")

    // only the clock moves: the time ticks without a request to Patronus
    await page.clock.fastForward("02:00")
    await expect(chip).toHaveText("20m · ~20m")
    await expect(progress).toHaveAttribute("aria-valuenow", "50")

    // past the usual time: the usual time instead of the time left, the progress line runs across
    await page.clock.fastForward("25:00")
    await expect(chip).toHaveText("45m · usually ~40m")
    await expect(progress).toHaveAttribute("data-indeterminate", "true")
  })
})

test.describe("Patronus in the landing overview", () => {
  test("should show a robot that failed on the branch tip once, as the move it is, with the chips of the other rows", async ({ page }) => {
    const indicator = page.getByTestId("landing-indicator")
    await expect(indicator).toHaveAttribute("data-urgent", "needsFix")
    const popover = await openLandingOverview(page)
    // the failed robot is the row of ABC-123, with its actions: no second row of the branch
    await expect(popover.getByTestId("landing-your-move").locator("[data-testid=robots-row][data-branch-name=ABC-123]")).toBeVisible()
    await expect(landingRow(popover, "ABC-123")).toHaveCount(0)
    // a branch without a robot has no Patronus chip in its row
    await expect(landingRow(popover, "feature-auth")).toBeVisible()
    await expect(landingRow(popover, "feature-auth").getByTestId("patronus-chip")).toHaveCount(0)
    await expect(popover.getByTestId("patronus-status-line")).toContainText("Patronus checked just now")
  })

  test("should list the robots again from the overview", async ({ page, repo }) => {
    const { listCalls } = await repo.getPatronusLog()
    const popover = await openLandingOverview(page)
    await popover.getByTestId("patronus-refresh").click()
    await expect.poll(async () => (await repo.getPatronusLog()).listCalls).toBe(listCalls + 1)
    await expect(popover.getByTestId("patronus-refresh")).toBeEnabled()
  })
})

test.describe("Patronus merge", () => {
  test("should tell when a running Safe Push merged, and update the project on request", async ({ page, repo }) => {
    await repo.setPatronus(availableRobots({ ...RUNNING, status: "success", finishTime: 1704142800 }, FAILED, MERGED, DRY_RUN, OUTDATED))
    // Update Project fetches the baseline itself: one click picks the merge up
    const updates: string[] = []
    page.on("request", request => {
      if (request.url().endsWith("/invoke/update_project")) {
        updates.push(request.url())
      }
    })
    // a focus polls the running robot at once
    await emitTauriEvent(page, "tauri://focus")
    const toast = page.locator("[data-slot=toast]").filter({ hasText: "JIRA-456 merged into master" })
    await expect(toast).toContainText("Safe Push")
    // the issue of the title opens in the tracker, as from the row
    const issueLink = toast.getByRole("link", { name: "JIRA-456" })
    await expect(issueLink).toHaveAttribute("href", "https://jira.example.com/browse/JIRA-456")
    await expect(issueLink).toHaveAttribute("target", "_blank")
    await expect(getChip(page, "JIRA-456")).toHaveText("Merged")

    // the second action opens the robot, the toast stays for Update Project
    await toast.getByRole("button", { name: "Open in Patronus" }).click()
    expect(await getOpenedUrls(page)).toEqual(["https://patronus.labs.jb.gg/robot/running-robot"])
    await toast.getByRole("button", { name: "Update Project" }).click()
    await expect.poll(() => updates.length).toBe(1)
    await expect(toast).toHaveCount(0)
  })
})

test.describe("Patronus failure", () => {
  test("should tell when a running Safe Push failed, record it and copy the prompt for an agent from the toast", async ({ page, repo }) => {
    await repo.setPatronus(availableRobots({ ...RUNNING, status: "failure", finishTime: 1704189000 }, FAILED, MERGED, DRY_RUN, OUTDATED))
    // a focus polls the running robot at once
    await emitTauriEvent(page, "tauri://focus")
    const toast = page.locator("[data-slot=toast]").filter({ hasText: "Safe Push of JIRA-456 failed" })
    await expect(toast).toContainText("JIRA-456 → master")
    // the title and the description link the issue
    await expect(toast.getByRole("link", { name: "JIRA-456" })).toHaveCount(2)
    await expect(toast.getByRole("link", { name: "JIRA-456" }).last()).toHaveAttribute("href", "https://jira.example.com/browse/JIRA-456")
    // the row says it, and its node is in the colour of the failure
    await expect(getChip(page, "JIRA-456")).toHaveText("Failed")
    await expect(getSlot(page, "JIRA-456")).toHaveAttribute("data-step", "restart")

    // the trace once the toast is gone
    const activity = await openActivityWindow(page)
    await expect(activity.getByTestId("activity-title").filter({ hasText: "Safe Push of JIRA-456 failed" })).toBeVisible()

    // the fix comes first: the toast copies the prompt, and says so in place
    await toast.getByRole("button", { name: "Copy Prompt for Agent" }).click()
    await expect(page.locator("[data-slot=toast]").filter({ hasText: "Copied the prompt for JIRA-456" })).toBeVisible()
    await expect.poll(() => readClipboard(page)).toContain("Safe Push of `JIRA-456` into `master` failed")
  })

  test("should restart from the toast a Safe Push whose builds never started", async ({ page, repo }) => {
    // the CI failed, not the branch
    await repo.setPatronusDetails(buildsNeverStarted(RUNNING.id))
    await repo.setPatronus(availableRobots({ ...RUNNING, status: "failure", finishTime: 1704189000 }, FAILED, MERGED, DRY_RUN, OUTDATED))
    await emitTauriEvent(page, "tauri://focus")
    const toast = page.locator("[data-slot=toast]").filter({ hasText: "Safe Push of JIRA-456 failed" })
    await toast.getByRole("button", { name: "Restart Safe Push" }).click()
    await expect(page.locator("[data-slot=toast]").filter({ hasText: "Safe Push restarted" })).toBeVisible()
    expect((await repo.getPatronusLog()).restarts).toEqual([RUNNING.id])
  })
})

baseTest.describe("Patronus unavailable", () => {
  baseTest("should hint quietly when the Safe Push CLI is not logged in", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_links", { patronus: unavailableRobots({ status: "notLoggedIn" }) })
    await syncAndWaitForBranches()
    const popover = await openLandingOverview(page)
    await expect(popover.getByTestId("patronus-status-line")).toContainText("Run ./safePush.cmd once to log in to Patronus")
    // the hint names a command: it copies, into the repository
    await popover.getByTestId("patronus-status-line-action").click()
    // the user agent of the tests is Windows': PowerShell
    await expect.poll(() => readClipboard(page)).toMatch(/^cd .+; \.\\safePush\.cmd$/)
    await expect(page.getByTestId("patronus-chip")).toHaveCount(0)
    // never a toast
    await expect(page.locator("[data-slot=toast]")).toHaveCount(0)
  })

  baseTest("should say nothing for a repository that is not on Space", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_links")
    await syncAndWaitForBranches()
    const popover = await openLandingOverview(page)
    await expect(popover.getByTestId("patronus-status-line")).toHaveCount(0)
    await expect(page.getByTestId("patronus-chip")).toHaveCount(0)
  })
})
