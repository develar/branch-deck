// @spec spec/repository/main-views.spec.md
import type { Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { findBranchRow } from "../helpers/branch-helpers"
import { DAY_ROBOTS, ROBOTS_NOW, availableRobots, failedFastTests, setupRobots, unavailableRobots } from "../helpers/patronus-helpers"
import { getOpenedUrls } from "../helpers/pull-request-helpers"
import { openLandingOverview } from "../helpers/review-status-helpers"
import { emitTauriEvent, runAndWaitForSync, waitForTauriListener } from "../helpers/sync-helpers"
import type { HistoryRobotFixture, TestRepositoryBuilder } from "../helpers/test-repository"
import { openMainMenuView, TITLE_BARS, useTitleBar } from "../helpers/title-bar-helpers"
import { waitForStorePersistence } from "../helpers/wait-helpers"

const HOUR = 3600
const DAY = 86_400
// `ROBOTS_NOW` is Tuesday 2024-01-02 10:00 UTC, the page's clock and the statistics' now; the statistics of the E2E
// server are in UTC
const MONDAY_NINE = ROBOTS_NOW - DAY - HOUR

/**
 * The history beside the robots of the day (`DAY_ROBOTS`, the user's): one more merge and one more failure of the user
 * on a branch of last week, and five other people with three merges each, started at 09:00 on Monday, an hour long
 */
const HISTORY: HistoryRobotFixture[] = [
  { id: "mine-merged", owner: "1", status: "success", startTime: ROBOTS_NOW - 2 * DAY, finishTime: ROBOTS_NOW - 2 * DAY + 2 * HOUR, branch: "old-work" },
  { id: "mine-failed", owner: "1", status: "failure", startTime: ROBOTS_NOW - 3 * DAY, finishTime: ROBOTS_NOW - 3 * DAY + HOUR, branch: "old-work" },
  { id: "mine-dry-run", owner: "1", status: "success", startTime: ROBOTS_NOW - 4 * DAY, finishTime: ROBOTS_NOW - 4 * DAY + HOUR, dryRun: true },
  ...["2", "3", "4", "5", "6"].flatMap(owner =>
    [0, 1, 2].map(index => ({
      id: `theirs-${owner}-${index}`,
      owner,
      status: "success" as const,
      startTime: MONDAY_NINE + index * 60,
      finishTime: MONDAY_NINE + index * 60 + HOUR,
    })),
  ),
]

async function setupHistory(
  page: Page,
  setupRepo: Parameters<typeof setupRobots>[1],
  syncAndWaitForBranches: () => Promise<unknown>,
  history: HistoryRobotFixture[] = HISTORY,
): Promise<TestRepositoryBuilder> {
  const repo = await setupRobots(page, setupRepo, syncAndWaitForBranches, DAY_ROBOTS.merged, DAY_ROBOTS.failed)
  await repo.setPatronusHistory(history)
  await repo.setPatronusDetails(failedFastTests(DAY_ROBOTS.failed.id))
  return repo
}

function landingView(page: Page) {
  return page.getByTestId("landing-view")
}

/** Chooses a period through the Select of the header, as the user does */
async function choosePeriod(page: Page, name: string): Promise<void> {
  await landingView(page).getByRole("combobox", { name: "Period" }).click()
  await page.getByRole("option", { name }).click()
}

/** The UTC epoch weeks (from Thursday) the lists of the statistics start with, as Patronus gets them */
function weekStart(unixSeconds: number): string {
  const week = 7 * DAY
  return new Date(Math.floor(unixSeconds / week) * week * 1000).toISOString().replace(".000Z", "Z")
}

/** The lists of the statistics the fake Patronus got: everyone's, with an end (the user's own list has none) */
async function statisticsLists(repo: TestRepositoryBuilder): Promise<Record<string, string>[]> {
  return (await repo.getPatronusLog()).requests.listQueries.filter(query => "end" in query)
}

async function expectStatisticsShown(page: Page): Promise<void> {
  await expect(landingView(page)).toBeVisible()
  await expect(page.getByTestId("main-view-landing")).toHaveAttribute("aria-pressed", "true")
  await expect(landingView(page).getByTestId("landing-merged")).toContainText("2")
}

test.describe("Landing view", () => {
  test("should show how the Safe Pushes went: the merges against everyone else, the time lost, the failure causes and the hours", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupHistory(page, setupRepo, syncAndWaitForBranches)
    await page.getByTestId("main-view-landing").click()
    await expectStatisticsShown(page)
    const view = landingView(page)
    // 30 days by default
    await expect(view.getByRole("combobox", { name: "Period" })).toContainText("Last 30 days")
    await expect(view.getByTestId("landing-window")).toHaveText("Into master · UTC")
    // 2 merged of 4 finished robots; the dry run apart; everyone else as the median of the five people
    await expect(view.getByTestId("landing-merged")).toContainText("Merged22 failed+1 dry run")
    await expect(view.getByTestId("landing-robots-per-merge")).toContainText("Robots per merge2.0100% more than everyone elseeveryone else 1.0")
    // the median of 40 minutes and 2 hours, a third longer than everyone else's hour: worse, in the destructive colour
    await expect(view.getByTestId("landing-merge-time")).toContainText("Time to merge1h 20m33% more than everyone elsep90 2h · everyone else 1h")
    await expect(view.getByTestId("landing-merge-time-comparison")).toHaveClass(/text-destructive/)
    await expect(view.getByTestId("landing-delivery")).toContainText("against everyone else, the median of 5 people")
    // the robots of each week, the table twin of the chart: the 5 Mondays of the 30 days from Sunday 2023-12-03; the
    // week of the day, so far, has the merge and the failure
    const weeks = view.getByRole("table", { name: "Robots per week" }).getByRole("row")
    await expect(weeks).toHaveCount(6)
    await expect(weeks.last()).toHaveText("This week, so far110")
    // the time to merge by week against everyone else's: last week the merge of old-work, this week the merge of the
    // day against the hour of everyone else
    const mergeTimes = view.getByRole("table", { name: "Median time to merge by week" }).getByRole("row")
    await expect(mergeTimes.filter({ hasText: /^(Dec 25|This week)/ })).toHaveText(["Dec 252h–", "This week40m1h"])
    // the trends are of the weeks that ended: the time lost last week to the failure of old-work and its wait
    await expect(view.getByRole("img", { name: "Time lost by week" })).toBeVisible()
    // where the time went: 2 h 40 min in merges, 1 h 30 min in failures, 23 hours from last week's failure of old-work
    // to its merge
    await expect(view.getByTestId("landing-time-lost")).toContainText("Lost to failures25 h90% of the robot time")
    await expect(view.getByTestId("time-legend").getByRole("listitem")).toHaveText([
      "Merged robots3 h10%",
      "Failed robots2 h6%",
      "Canceled robots0 min0%",
      "Waiting to restart23 h85%",
    ])
    // the merge of the day was the first try, the one of old-work came after a failure
    await expect(view.getByTestId("landing-first-try")).toHaveText("First try50%1 of 2 merges, estimated")
    // the failed robot of the day failed Fast Tests; the one of last week answers no check
    const failures = view.getByTestId("landing-failures")
    await expect(failures.getByRole("table", { name: "Failed robots by kind" }).getByRole("row")).toHaveText(["KindRobots", "Checks failed1", "Unknown1"])
    await expect(failures.getByTestId("landing-failed-checks").getByRole("listitem")).toHaveText(["Fast Tests1"])
    // a check opens the newest robot it failed
    await failures.getByRole("button", { name: "Fast Tests, 1: open the newest robot where it failed" }).click()
    expect(await getOpenedUrls(page)).toEqual([expect.stringMatching(new RegExp(`/robot/${DAY_ROBOTS.failed.id}$`))])
    // Monday 09:00 UTC: five people, an hour each; the user has too few robots at any hour
    const hours = view.getByRole("table", { name: "Median time to merge by start hour" }).getByRole("row")
    await expect(hours.filter({ hasText: /^09:00/ })).toHaveText("09:001h15–0")
    // the keyboard walks the marks of a chart and its tooltip says each (the accessibility layer of the charts): the
    // axis runs from the first hour with a value, 09:00, to the last, so the first press shows it
    const hourChart = view.getByTestId("hour-chart")
    await hourChart.getByRole("application").focus()
    await page.keyboard.press("ArrowRight")
    await expect(hourChart).toContainText("09:00Everyone else, median1h · 15 robots")
    // one hour says nothing of the fastest
    await expect(view.getByTestId("landing-hours")).toBeVisible()
    await expect(view.getByTestId("landing-fastest-hours")).toHaveCount(0)
    const weekChart = view.getByTestId("week-chart")
    await weekChart.getByRole("application").focus()
    // the fifth week, the current one
    for (let week = 0; week < 5; week++) {
      await page.keyboard.press("ArrowRight")
    }
    await expect(weekChart).toContainText("This week, so farMerged1Failed1Canceled0")
    await captureHtmlSnapshot(view, "landing-statistics")

    // everyone's robots into the target, week by week: no owner, an end
    const lists = await statisticsLists(repo)
    expect(lists).toHaveLength(5)
    expect(lists.every(query => !("owner" in query) && query.targetBranch === "master")).toBe(true)
  })

  test("should open from the native View menu and Show Statistics, and give the branches back as they were", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupHistory(page, setupRepo, syncAndWaitForBranches)
    const row = findBranchRow(page, "ABC-123")
    await row.focus()
    await expect(row).toBeFocused()
    const expanded = await row.getAttribute("data-state")

    // ⌘2 of the native menu: the focus was in the branches, it moves to the heading of the view
    await waitForTauriListener(page, "show_landing")
    await emitTauriEvent(page, "show_landing")
    await expectStatisticsShown(page)
    await expect(landingView(page).getByRole("heading", { level: 1, name: "Safe Push Statistics" })).toBeFocused()
    await expect(row).toBeHidden()

    // ⌘1: the row as it was, focused, open or closed as before
    await emitTauriEvent(page, "show_branches")
    await expect(row).toBeFocused()
    await expect(row).toHaveAttribute("data-state", expanded ?? "")
    await expect(page.getByTestId("main-view-branches")).toHaveAttribute("aria-pressed", "true")

    // Show Statistics of the landing overview
    const popover = await openLandingOverview(page)
    await popover.getByRole("button", { name: "Show Statistics" }).click()
    await expect(popover).toBeHidden()
    await expectStatisticsShown(page)

    // the switcher by its keys: an arrow moves the focus, Enter shows the focused view
    await page.getByTestId("main-view-landing").focus()
    await page.keyboard.press("ArrowLeft")
    await expect(page.getByTestId("main-view-branches")).toBeFocused()
    await expect(row).toBeHidden()
    await page.keyboard.press("Enter")
    await expect(page.getByTestId("main-view-branches")).toHaveAttribute("aria-pressed", "true")
    await expect(row).toBeVisible()
  })

  test("should change the view only by the user: a sync keeps it, a reveal of a branch shows the branches", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupHistory(page, setupRepo, syncAndWaitForBranches)
    await page.getByTestId("main-view-landing").click()
    await expectStatisticsShown(page)
    // a sync while the view shows: it stays
    await waitForTauriListener(page, "sync_branches")
    await runAndWaitForSync(page, () => emitTauriEvent(page, "sync_branches"))
    await expect(landingView(page)).toBeVisible()
    // Go to a branch of the palette reveals it: the branches with the row focused
    await waitForTauriListener(page, "open_command_palette")
    await emitTauriEvent(page, "open_command_palette")
    const search = page.getByTestId("command-palette").getByRole("combobox", { name: "Command or branch" })
    await expect(search).toBeFocused()
    await search.fill("abc-1")
    await expect(page.getByTestId("command-palette").getByRole("option").first()).toHaveText(/^Go to ABC-123/)
    await page.keyboard.press("Enter")
    await expect(findBranchRow(page, "ABC-123")).toBeFocused()
    await expect(landingView(page)).toBeHidden()

    // Landing of the palette
    await emitTauriEvent(page, "open_command_palette")
    await expect(search).toBeFocused()
    await search.fill("landing")
    await expect(page.getByTestId("command-palette").getByRole("option").first()).toHaveText(/^Landing/)
    await page.keyboard.press("Enter")
    await expectStatisticsShown(page)
  })

  test("should read nothing while the view is hidden, and ask again on Refresh", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupHistory(page, setupRepo, syncAndWaitForBranches)
    // the branches show: a focus of the window reads no statistics (the same focus reads them below, once they show)
    await emitTauriEvent(page, "tauri://focus")
    await page.getByTestId("main-view-landing").click()
    await expectStatisticsShown(page)
    await expect(landingView(page)).toHaveAttribute("aria-busy", "false")
    // one read of the 5 weeks of 30 days: the one of the show (the E2E server keeps nothing between reads)
    expect(await statisticsLists(repo)).toHaveLength(5)

    await emitTauriEvent(page, "tauri://focus")
    await expect.poll(async () => (await statisticsLists(repo)).length).toBe(10)
    await landingView(page).getByTestId("landing-refresh").click()
    await expect.poll(async () => (await statisticsLists(repo)).length).toBe(15)
    await expect(landingView(page).getByTestId("landing-merged")).toContainText("2")
  })

  test("should say why there are no statistics", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("issue_links", { patronus: unavailableRobots({ status: "notLoggedIn" }) })
    await syncAndWaitForBranches()
    await page.getByTestId("main-view-landing").click()
    await expect(landingView(page).getByTestId("landing-state")).toHaveText("Run ./safePush.cmd once to log in to PatronusCopy Command")

    await repo.setPatronus(unavailableRobots({ status: "notPatronus" }))
    await landingView(page).getByTestId("landing-refresh").click()
    await expect(landingView(page).getByTestId("landing-state")).toHaveText("This repository is not on Space: Patronus has no robots of it")
  })

  test("should say when robots merge fastest: the fastest hours against the slowest", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // five more people merge in three hours when they start at 14:00 on Monday
    const slow = ["7", "8", "9", "10", "11"].map(owner => ({
      id: `slow-${owner}`,
      owner,
      status: "success" as const,
      startTime: MONDAY_NINE + 5 * HOUR,
      finishTime: MONDAY_NINE + 8 * HOUR,
    }))
    await setupHistory(page, setupRepo, syncAndWaitForBranches, [...HISTORY, ...slow])
    await page.getByTestId("main-view-landing").click()
    await expect(landingView(page).getByTestId("landing-fastest-hours")).toHaveText("Robots that start 09:00–10:00 merge fastest: 1h, against 3h at 14:00")
    // the chart illustrates the sentence: the key names the run, and the two medians stand on their bars
    await expect(landingView(page).getByTestId("hour-legend").getByRole("listitem")).toHaveText(["Everyone else, median", "Fastest hours 09:00–10:00", "You"])
    await expect(landingView(page).getByTestId("hour-chart").locator("svg text.tabular-nums")).toHaveText(["1h", "3h"])
  })

  test("should say there was no Safe Push in the period", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_links", { patronus: availableRobots() })
    await syncAndWaitForBranches()
    await page.getByTestId("main-view-landing").click()
    await expect(landingView(page).getByTestId("landing-state")).toHaveText("No Safe Push into master in the last 30 days")
    await choosePeriod(page, "Last 6 months")
    await expect(landingView(page).getByTestId("landing-state")).toHaveText("No Safe Push into master in the last 6 months")
  })

  test("should count the chosen period: its robots, its weeks, and keep the choice", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // a merge of three weeks ago: in 30 days, not in 14
    const repo = await setupHistory(page, setupRepo, syncAndWaitForBranches, [
      ...HISTORY,
      { id: "mine-older", owner: "1", status: "success", startTime: ROBOTS_NOW - 20 * DAY, finishTime: ROBOTS_NOW - 20 * DAY + HOUR, branch: "older-work" },
    ])
    await page.getByTestId("main-view-landing").click()
    const view = landingView(page)
    await expect(view.getByTestId("landing-merged")).toContainText("Merged3")
    await expect(view.getByTestId("landing-numbers")).toHaveAttribute("data-period", "30d")
    const before = (await statisticsLists(repo)).length

    await choosePeriod(page, "Last 14 days")
    await expect(view.getByRole("combobox", { name: "Period" })).toContainText("Last 14 days")
    await expect(view.getByTestId("landing-numbers")).toHaveAttribute("data-period", "14d")
    await expect(view.getByTestId("landing-merged")).toContainText("Merged2")
    // the 2 Mondays of 14 days from Tuesday, and Patronus asked for the 3 UTC weeks that cover them, two at once
    await expect(view.getByRole("table", { name: "Robots per week" }).getByRole("row")).toHaveCount(3)
    const lists = (await statisticsLists(repo)).slice(before)
    expect(lists.map(query => query.start ?? "").toSorted((a, b) => a.localeCompare(b))).toEqual([
      weekStart(ROBOTS_NOW - 14 * DAY),
      weekStart(ROBOTS_NOW - 7 * DAY),
      weekStart(ROBOTS_NOW),
    ])

    // the choice outlives the window
    await waitForStorePersistence(page, "appSettings", settings => (settings as { statsPeriod?: string } | undefined)?.statsPeriod === "14d")
    await page.reload()
    await syncAndWaitForBranches()
    await page.getByTestId("main-view-landing").click()
    await expect(view.getByRole("combobox", { name: "Period" })).toContainText("Last 14 days")
    await expect(view.getByTestId("landing-merged")).toContainText("Merged2")
  })
})

test.describe("Landing view of a custom title bar", () => {
  test("should open from Ctrl+2 and from View → Landing of ☰, and come back by Ctrl+1", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await useTitleBar(page, TITLE_BARS.gnome)
    await setupHistory(page, setupRepo, syncAndWaitForBranches)
    await page.keyboard.press("Control+Digit2")
    await expectStatisticsShown(page)
    await page.keyboard.press("Control+Digit1")
    await expect(findBranchRow(page, "ABC-123")).toBeVisible()

    await openMainMenuView(page)
    await page.getByRole("menuitem", { name: /Landing/ }).click()
    await expectStatisticsShown(page)
  })
})
