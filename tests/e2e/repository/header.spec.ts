// @spec spec/repository/update-project.spec.md
// @spec spec/repository/settings.spec.md
import type { Page } from "@playwright/test"
import { expect, test } from "../fixtures/test-fixtures"
import type { Box } from "../helpers/header-helpers"
import { boxOf, expectHeaderControlsOnOneLine, header, HEADER_CONTROLS, HEADER_HEIGHT, MIN_WINDOW } from "../helpers/header-helpers"
import { expectSettingsOpened } from "../helpers/settings-helpers"
import { emitTauriEvent, finishedSyncCount, waitForSyncComplete } from "../helpers/sync-helpers"
import { holdCommand, holdWithProgress } from "../helpers/wait-helpers"

// The `behind_upstream` template: the header has every control, the Update button (a clean update) and the outgoing
// indicator (feature-auth is not pushed).

/** The controls whose position moved on every sync before: they must stay where they are */
async function actionBoxes(page: Page): Promise<Box[]> {
  return Promise.all(["update-project-button", "outgoing-indicator", "sync-button"].map(id => boxOf(page.getByTestId(id))))
}

test.describe("Header", () => {
  test.beforeEach(async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await page.setViewportSize(MIN_WINDOW)
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
  })

  test("should keep its controls apart on one line at the minimum window width", async ({ page }) => {
    await expectHeaderControlsOnOneLine(page, HEADER_CONTROLS)
  })

  test("should keep its controls apart while an update runs", async ({ page }) => {
    const update = await holdWithProgress(page, "update_project")
    await page.getByTestId("update-project-button").click()
    await update.send({ phase: "fetching", fetch: { percent: 99, transferred: null, speed: null, line: "Receiving objects:  99% (99/100)" } })
    await expect(page.getByTestId("update-project-button")).toHaveText(/^Updating \d+%$/)
    await expectHeaderControlsOnOneLine(page, HEADER_CONTROLS)
    update.release()
    await expect(page.getByTestId("update-project-button")).toHaveText("Update")
  })

  test("should not move while a sync runs", async ({ page }) => {
    const lastSync = page.getByTestId("last-sync-time")
    await expect(lastSync.getByText(/^Last synced/)).toBeVisible()
    const before = await actionBoxes(page)

    // hold the sync until the layout is measured
    const releaseSync = await holdCommand(page, "sync_branches")
    const syncRequests: string[] = []
    page.on("request", request => {
      if (request.url().endsWith("/invoke/sync_branches")) {
        syncRequests.push(request.url())
      }
    })
    const previousCount = await finishedSyncCount(page)
    const syncButton = page.getByTestId("sync-button")
    await syncButton.click()
    await expect(syncButton).toHaveAttribute("aria-busy", "true")

    // the spinner of Sync is the one indicator: the time of the last sync stays as it is, no "Syncing…" beside it
    await expect(lastSync).toHaveText(/^Last synced/)
    expect((await boxOf(header(page))).height).toBeCloseTo(HEADER_HEIGHT, 0)
    expect(await actionBoxes(page)).toEqual(before)
    // its tooltip names the step, without the shortcut that does nothing now; the click closed it, the next hover opens it
    await page.mouse.move(0, 0)
    await syncButton.hover()
    await expect(page.getByTestId("sync-tooltip")).toHaveText("Syncing branches…")

    // ⌘R while it runs starts no second sync, only a check after it: nothing changed, so it syncs nothing either
    await emitTauriEvent(page, "sync_branches")
    releaseSync()
    await waitForSyncComplete(page, previousCount)
    await expect(lastSync).toHaveText("Checked just now")
    expect(syncRequests).toHaveLength(1)
  })

  test("should name the shortcut of Sync and keep the selected repository without a tooltip", async ({ page }) => {
    // the click of the sync before closed the tooltip; it opens on the next hover
    await page.mouse.move(0, 0)
    await page.getByTestId("sync-button").hover()
    await expect(page.getByTestId("sync-tooltip")).toHaveText(/^Sync branches(⌘R|Ctrl\+R)$/)

    // with a tooltip delay of 0, a tooltip of the selector would open before the one of Sync is gone
    await page.getByTestId("browse-repository-button").hover()
    await expect(page.getByRole("tooltip")).toHaveCount(0)
  })

  test("should open Settings from the button after Sync, its tooltip naming the key", async ({ page }) => {
    const button = header(page).getByRole("button", { name: "Settings", exact: true })
    await page.mouse.move(0, 0)
    await button.hover()
    await expect(page.getByRole("tooltip")).toHaveText(/^Settings(⌘,|Ctrl\+,)$/)
    await button.click()
    await expectSettingsOpened(page, {})
  })
})

test.describe("Header without a repository", () => {
  test("should offer Settings before a repository is chosen", async ({ page, setupRepo }) => {
    await setupRepo("NO_REPO", { prepopulateStore: false })
    await expect(page.getByTestId("welcome-card")).toBeVisible()
    await header(page).getByTestId("settings-button").click()
    await expectSettingsOpened(page, {})
  })
})
