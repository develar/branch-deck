// @spec openspec/specs/repository/title-bar/spec.md
import type { Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "../helpers/branch-helpers"
import { getSubWindowReadyOpen, getWindowReadyRows, getWindowShell, observeWindowStages } from "../helpers/sub-window-helpers"
import { windowCalls } from "../helpers/title-bar-helpers"

/** The repository was synced an hour ago: the window syncs it by itself when it opens it */
const SYNCED_AN_HOUR_AGO = 60 * 60 * 1000

/** The rows of the `branch_filter` template: a stack, a side stack and lone branches, enough for the filter bar */
const BRANCH_FILTER_ROWS = 11

test.describe("The first frame of the main window", () => {
  // what the page says of the stages of its window, with what it had in its DOM then
  test.beforeEach(async ({ page }) => {
    await observeWindowStages(page)
  })

  test("should show the main window on the first rows of its sync, the table loading where they come until then", async ({ page, setupRepo }) => {
    // the first sync of the session, held in it: what the window shows before its rows
    const repo = await setupRepo("branch_filter", {
      prepopulateStore: true,
      syncedAgoMs: SYNCED_AN_HOUR_AGO,
      syncedBranchCount: BRANCH_FILTER_ROWS,
      remoteAnswers: { hold: ["sync"] },
      waitForSync: false,
    })

    // the title row and, where the rows come, the table loading in the room of the last sync's rows and their filter bar
    const loading = page.getByTestId("branch-table-loading")
    await expect(loading).toHaveAttribute("aria-busy", "true")
    await expect(loading.getByRole("columnheader", { name: "Branch Name" })).toBeVisible()
    // the session's sync waits in its first step
    await expect(loading).toContainText("Grouping the commits…")
    await expect(page.getByTestId("sync-button")).toBeVisible()
    const loadingBox = await loading.boundingBox()
    // the shell is said with the title row and the loading table: the 1 s budget of the rows counts from it, so the
    // window the budget shows has them, never a bare canvas
    await expect.poll(() => getWindowShell(page)).toEqual({ titleRows: 1, loadingTables: 1, branchRows: 0 })
    // the window is still hidden: the backend shows it by its budget if the rows take longer
    expect(await getSubWindowReadyOpen(page)).toBeNull()

    await repo.holdRemoteAnswers({ release: ["sync"] })
    await expect(findBranchRow(page, "core")).toBeVisible()
    await expect.poll(() => getSubWindowReadyOpen(page)).toBe(1)
    // said with the rows in the page, not before them
    expect(await getWindowReadyRows(page)).toBe(BRANCH_FILTER_ROWS)
    await expect(loading).toHaveCount(0)
    // the rows and the filter bar took the room the loading table had: nothing below moved
    const table = page.getByTestId("branch-view").locator("[data-slot=card]").first()
    expect((await table.boundingBox())?.height).toBe(loadingBox?.height)
  })

  test("should show the main window without rows where the window has none to wait for", async ({ page, setupRepo }) => {
    // no repository: the Welcome card is the first frame
    await setupRepo("NO_REPO", { prepopulateStore: false })
    await expect(page.getByTestId("welcome-card")).toBeVisible()
    await expect.poll(() => getSubWindowReadyOpen(page)).toBe(1)
    expect(await getWindowReadyRows(page)).toBe(0)
    await expect(page.getByTestId("branch-table-loading")).toHaveCount(0)
  })

  test("should show the main window on Ready to Sync, which waits for the user, not for a sync", async ({ page, setupRepo }) => {
    await setupRepo("simple", { prepopulateStore: true })
    await expect(page.getByTestId("ready-to-sync-card")).toBeVisible()
    await expect.poll(() => getSubWindowReadyOpen(page)).toBe(1)
    await expect(page.getByTestId("branch-table-loading")).toHaveCount(0)
  })

  test("should show the main window on the failure of a render that threw, log it, and reload or close the window", async ({ page, setupRepo }) => {
    // the first load of the page gets a title bar whose buttons the row cannot read, so the render of the row throws;
    // a reload gets none, as a failure that a reload cures
    await page.addInitScript(() => {
      if (sessionStorage.getItem("e2e-broken-title-bar") !== null) {
        return
      }
      sessionStorage.setItem("e2e-broken-title-bar", "")
      ;(window as unknown as { __BD_TITLE_BAR__: unknown }).__BD_TITLE_BAR__ = {
        kind: "custom",
        controls: { start: null, end: [], style: "kde" },
        doubleClick: "none",
        cornerRadius: 0,
        checkForUpdates: false,
      }
    })
    await setupRepo("NO_REPO", { prepopulateStore: false, waitForPage: false })

    const failure = page.getByTestId("page-failure")
    await expect(failure.getByRole("heading", { name: "Cannot show the window" })).toBeVisible()
    // the window shows on the failure at once, not by a deadline over a blank canvas
    await expect.poll(() => getSubWindowReadyOpen(page)).toBe(1)
    // the log of the app has the error with its stack and the components it was thrown in
    const logged = await errorLogs(page)
    const renderFailure = logged.find(entry => entry.line === "Cannot show the window")
    expect(renderFailure?.keyValues.stack).toBeTruthy()
    expect(renderFailure?.keyValues.component_stack).toBeTruthy()
    await expect(failure).toContainText(renderFailure?.keyValues.error ?? "")
    // the page draws the title bar: the row above the failure drags the window, which has no other title bar
    await expect(page.getByTestId("page-failure-row")).toHaveAttribute("data-tauri-drag-region", "deep")

    await failure.getByRole("button", { name: "Close Window" }).click()
    await expect.poll(() => windowCalls(page)).toEqual(["close"])

    await failure.getByRole("button", { name: "Reload" }).click()
    await expect(page.getByTestId("welcome-card")).toBeVisible()
    await expect(failure).toHaveCount(0)
  })

  test("should show the failure of a page that did not load", async ({ page, setupRepo }) => {
    await setupRepo("NO_REPO", { prepopulateStore: false })
    await expect(page.getByTestId("welcome-card")).toBeVisible()
    // a path that no page has, of the same test repository
    await page.goto(`/no-such-page${new URL(page.url()).search}`)
    await expect(page.getByTestId("page-failure").getByRole("heading", { name: "Cannot open the page of the window" })).toBeVisible()
    expect((await errorLogs(page)).map(entry => entry.line)).toEqual(["Cannot open the page of the window"])
  })

  test("should log an error of a script and a rejection that nobody awaits", async ({ page, setupRepo }) => {
    await setupRepo("NO_REPO", { prepopulateStore: false })
    await expect(page.getByTestId("welcome-card")).toBeVisible()
    // a script of the page that throws as it loads, as a module would: an error of a script that the test evaluates
    // reaches no handler of the page
    await page.addScriptTag({ content: `void Promise.reject(new Error("nobody awaited")); throw new Error("a script threw")` })
    await expect
      .poll(async () => (await errorLogs(page)).map(entry => `${entry.line}: ${entry.keyValues.error}`).toSorted())
      .toEqual(["Uncaught error of the page: a script threw", "Unhandled rejection of the page: nobody awaited"])
    // the page goes on
    await expect(page.getByTestId("welcome-card")).toBeVisible()
  })

  test("should show the main window when git defers its first sync, without a table that loads for ever", async ({ page, setupRepo }) => {
    // a rebase in progress: the sync on open ends without rows (`SyncDeferred`)
    await setupRepo("simple", { prepopulateStore: true, syncedAgoMs: SYNCED_AN_HOUR_AGO, syncedBranchCount: 1, rebaseInProgress: true })
    await expect(page.getByTestId("sync-button")).toHaveAttribute("data-sync-count", "1")
    await expect.poll(() => getSubWindowReadyOpen(page)).toBe(1)
    expect(await getWindowReadyRows(page)).toBe(0)
    await expect(page.getByTestId("branch-table-loading")).toHaveCount(0)
  })
})

/** The lines of the error level that the page sent to the log of the app (`__e2eErrorLogs` of the mock of `plugin:log`) */
function errorLogs(page: Page): Promise<{ line: string; keyValues: Record<string, string | undefined> }[]> {
  return page.evaluate(() => (window as unknown as { __e2eErrorLogs?: { line: string; keyValues: Record<string, string | undefined> }[] }).__e2eErrorLogs ?? [])
}
