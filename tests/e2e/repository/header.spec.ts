// @spec spec/repository/update-project.spec.md
import type { Locator, Page } from "@playwright/test"
import { expect, test } from "../fixtures/test-fixtures"
import { finishedSyncCount, waitForSyncComplete } from "../helpers/sync-helpers"
import { holdCommand } from "../helpers/wait-helpers"

// The `behind_upstream` template: the header has every control, the Update button (a clean update) and the outgoing
// indicator (feature-auth is not pushed). 960 px is the minimum width of the window (`min_inner_size`).
const MIN_WINDOW = { width: 960, height: 700 }
const HEADER_HEIGHT = 56

interface Box {
  x: number
  y: number
  width: number
  height: number
}

async function boxOf(locator: Locator): Promise<Box> {
  await expect(locator).toBeVisible()
  const box = await locator.boundingBox()
  expect(box).not.toBeNull()
  return box!
}

function overlaps(a: Box, b: Box): boolean {
  return a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

function header(page: Page): Locator {
  return page.getByTestId("configuration-header")
}

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
    const headerBox = await boxOf(header(page))
    expect(headerBox.height).toBeCloseTo(HEADER_HEIGHT, 0)

    const ids = ["browse-repository-button", "update-project-button", "outgoing-indicator", "last-sync-time", "sync-button", "sync-settings-button"]
    const boxes = await Promise.all(ids.map(id => boxOf(header(page).getByTestId(id))))
    boxes.forEach((box, index) => {
      // inside the header: nothing wraps to a second line or is pushed out of the card
      expect(box.x, `${ids[index]} starts in the header`).toBeGreaterThanOrEqual(headerBox.x)
      expect(box.x + box.width, `${ids[index]} ends in the header`).toBeLessThanOrEqual(headerBox.x + headerBox.width)
      expect(box.y, `${ids[index]} is on the header line`).toBeGreaterThanOrEqual(headerBox.y)
      expect(box.y + box.height, `${ids[index]} is on the header line`).toBeLessThanOrEqual(headerBox.y + headerBox.height)
      for (let other = index + 1; other < boxes.length; other++) {
        expect(overlaps(box, boxes[other]!), `${ids[index]} overlaps ${ids[other]}`).toBe(false)
      }
    })
  })

  test("should not move while a sync runs", async ({ page }) => {
    const lastSync = page.getByTestId("last-sync-time")
    await expect(lastSync.getByText(/^Last synced/)).toBeVisible()
    const before = await actionBoxes(page)

    // hold the sync until the layout is measured
    const releaseSync = await holdCommand(page, "sync_branches")
    const previousCount = await finishedSyncCount(page)
    const syncButton = page.getByTestId("sync-button")
    await syncButton.click()
    await expect(syncButton).toHaveAttribute("aria-busy", "true")

    // the time of the last sync gives its place to "Syncing…" instead of disappearing
    await expect(lastSync.getByText("Syncing…")).toBeVisible()
    await expect(lastSync.getByText(/^Last synced/)).toBeHidden()
    expect((await boxOf(header(page))).height).toBeCloseTo(HEADER_HEIGHT, 0)
    expect(await actionBoxes(page)).toEqual(before)

    releaseSync()
    await waitForSyncComplete(page, previousCount)
    await expect(lastSync.getByText(/^Last synced/)).toBeVisible()
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
})
