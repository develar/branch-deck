// @spec spec/repository/title-bar.spec.md
import type { Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "../helpers/branch-helpers"
import { getSubWindowReadyOpen, getWindowShell } from "../helpers/sub-window-helpers"
import { holdCommand } from "../helpers/wait-helpers"

/** The repository was synced an hour ago: the window syncs it by itself when it opens it */
const SYNCED_AN_HOUR_AGO = 60 * 60 * 1000

/** The rows of the `branch_filter` template: a stack, a side stack and lone branches, enough for the filter bar */
const BRANCH_FILTER_ROWS = 11

/** The branch rows the main window's page had when it said its first frame is drawn (`sub_window_ready`), or null */
async function rowsWhenReady(page: Page): Promise<number | null> {
  return page.evaluate(() => (window as unknown as { __e2eWindowReadyRows?: number }).__e2eWindowReadyRows ?? null)
}

test.describe("The first frame of the main window", () => {
  test("should show the main window on the first rows of its sync, the table loading where they come until then", async ({ page, setupRepo }) => {
    // the first sync of the window, held: what the window shows before its rows
    const release = await holdCommand(page, "sync")
    // the setup waits for Sync, which the held sync keeps disabled
    const setup = setupRepo("branch_filter", { prepopulateStore: true, syncedAgoMs: SYNCED_AN_HOUR_AGO, syncedBranchCount: BRANCH_FILTER_ROWS })

    // the title row and, where the rows come, the table loading in the room of the last sync's rows and their filter bar
    const loading = page.getByTestId("branch-table-loading")
    await expect(loading).toHaveAttribute("aria-busy", "true")
    await expect(loading.getByRole("columnheader", { name: "Branch Name" })).toBeVisible()
    await expect(loading).toContainText("Syncing branches…")
    await expect(page.getByTestId("sync-button")).toBeVisible()
    const loadingBox = await loading.boundingBox()
    // the shell is said with the title row and the loading table: the 1 s budget of the rows counts from it, so the
    // window the budget shows has them, never a bare canvas
    await expect.poll(() => getWindowShell(page)).toEqual({ titleRows: 1, loadingTables: 1, branchRows: 0 })
    // the window is still hidden: the backend shows it by its budget if the rows take longer
    expect(await getSubWindowReadyOpen(page)).toBeNull()

    release()
    await setup
    await expect(findBranchRow(page, "core")).toBeVisible()
    await expect.poll(() => getSubWindowReadyOpen(page)).toBe(1)
    // said with the rows in the page, not before them
    expect(await rowsWhenReady(page)).toBe(BRANCH_FILTER_ROWS)
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
    expect(await rowsWhenReady(page)).toBe(0)
    await expect(page.getByTestId("branch-table-loading")).toHaveCount(0)
  })

  test("should show the main window on Ready to Sync, which waits for the user, not for a sync", async ({ page, setupRepo }) => {
    await setupRepo("simple", { prepopulateStore: true })
    await expect(page.getByTestId("ready-to-sync-card")).toBeVisible()
    await expect.poll(() => getSubWindowReadyOpen(page)).toBe(1)
    await expect(page.getByTestId("branch-table-loading")).toHaveCount(0)
  })

  test("should show the main window when git defers its first sync, without a table that loads for ever", async ({ page, setupRepo }) => {
    // a rebase in progress: the sync on open ends without rows (`SyncDeferred`)
    await setupRepo("simple", { prepopulateStore: true, syncedAgoMs: SYNCED_AN_HOUR_AGO, syncedBranchCount: 1, rebaseInProgress: true })
    await expect(page.getByTestId("sync-button")).toHaveAttribute("data-sync-count", "1")
    await expect.poll(() => getSubWindowReadyOpen(page)).toBe(1)
    expect(await rowsWhenReady(page)).toBe(0)
    await expect(page.getByTestId("branch-table-loading")).toHaveCount(0)
  })
})
