// @spec spec/repository/main-views.spec.md
import type { Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { findBranchRow } from "../helpers/branch-helpers"
import { DAY_ROBOTS, ROBOTS_NOW, availableRobots, failedFastTests, setupRobots, unavailableRobots } from "../helpers/patronus-helpers"
import { openLandingOverview } from "../helpers/review-status-helpers"
import { emitTauriEvent, runAndWaitForSync, waitForTauriListener } from "../helpers/sync-helpers"
import type { HistoryRobotFixture, TestRepositoryBuilder } from "../helpers/test-repository"
import { openMainMenuView, TITLE_BARS, useTitleBar } from "../helpers/title-bar-helpers"

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

async function setupHistory(page: Page, setupRepo: Parameters<typeof setupRobots>[1], syncAndWaitForBranches: () => Promise<unknown>): Promise<TestRepositoryBuilder> {
  const repo = await setupRobots(page, setupRepo, syncAndWaitForBranches, DAY_ROBOTS.merged, DAY_ROBOTS.failed)
  await repo.setPatronusHistory(HISTORY)
  await repo.setPatronusDetails(failedFastTests(DAY_ROBOTS.failed.id))
  return repo
}

function landingView(page: Page) {
  return page.getByTestId("landing-view")
}

/** The lists of the statistics the fake Patronus got: everyone's, with an end (the user's own list has none) */
async function statisticsLists(repo: TestRepositoryBuilder): Promise<Record<string, string>[]> {
  return (await repo.getPatronusLog()).requests.listQueries.filter(query => "end" in query)
}

async function expectStatisticsShown(page: Page): Promise<void> {
  await expect(landingView(page)).toBeVisible()
  await expect(page.getByTestId("main-view-landing")).toHaveAttribute("aria-selected", "true")
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
    await expect(view.getByTestId("landing-window")).toHaveText("Into master · the last 70 days · UTC")
    // 2 merged of 4 finished robots; the dry run apart; everyone else as the median of the five people
    await expect(view.getByTestId("landing-merged")).toHaveText("Merged22 failed · + 1 dry runs")
    await expect(view.getByTestId("landing-robots-per-merge")).toHaveText("Robots per merge2.0everyone else 1.0")
    // the median of 40 minutes and 2 hours
    await expect(view.getByTestId("landing-merge-time")).toHaveText("Time to merge1h 20mp90 2h · everyone else 1h")
    await expect(view.getByTestId("landing-delivery")).toContainText("against everyone else, the median of 5 people")
    // where the time went: 2 h 40 min in merges, 1 h 30 min in failures, 23 hours from last week's failure of old-work
    // to its merge
    await expect(view.getByTestId("landing-time").getByRole("listitem")).toHaveText(["Merged robots3 h", "Failed robots2 h", "Canceled robots0 min", "Waiting to restart23 h"])
    // the merge of the day was the first try, the one of old-work came after a failure
    await expect(view.getByTestId("landing-first-try")).toHaveText("First try50%1 of 2 merges, estimated")
    // the failed robot of the day failed Fast Tests; the one of last week answers no check
    const failures = view.getByTestId("landing-failures")
    await expect(failures.getByRole("list", { name: "Failures by kind" }).getByRole("listitem")).toHaveText(["Checks failed1", "Unknown1"])
    await expect(failures.getByTestId("landing-failed-checks").getByRole("listitem")).toHaveText(["Fast Tests1"])
    // Monday 09:00 UTC: five people, an hour each
    await expect(view.getByTestId("hour-9")).toHaveAccessibleName("09:00 · everyone 1h (15), among the fastest")
    await view.getByTestId("hour-9").hover()
    await expect(view.getByTestId("chart-tooltip")).toHaveText("09:00 · everyone 1h (15), among the fastest")
    await captureHtmlSnapshot(view, "landing-statistics")

    // everyone's robots into the target, week by week: no owner, an end
    const lists = await statisticsLists(repo)
    expect(lists).toHaveLength(11)
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
    await expect(page.getByTestId("main-view-branches")).toHaveAttribute("aria-selected", "true")

    // Show Statistics of the landing overview
    const popover = await openLandingOverview(page)
    await popover.getByRole("button", { name: "Show Statistics" }).click()
    await expect(popover).toBeHidden()
    await expectStatisticsShown(page)

    // the switcher by its arrow keys
    await page.getByTestId("main-view-landing").focus()
    await page.keyboard.press("ArrowLeft")
    await expect(page.getByTestId("main-view-branches")).toBeFocused()
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
    // one read of the 11 weeks: the one of the show (the E2E server keeps nothing between reads)
    expect(await statisticsLists(repo)).toHaveLength(11)

    await emitTauriEvent(page, "tauri://focus")
    await expect.poll(async () => (await statisticsLists(repo)).length).toBe(22)
    await landingView(page).getByTestId("landing-refresh").click()
    await expect.poll(async () => (await statisticsLists(repo)).length).toBe(33)
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

  test("should say there was no Safe Push", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("issue_links", { patronus: availableRobots() })
    await syncAndWaitForBranches()
    await page.getByTestId("main-view-landing").click()
    await expect(landingView(page).getByTestId("landing-state")).toHaveText("No Safe Push into master in the last 70 days")
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
