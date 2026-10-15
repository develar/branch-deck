// @spec spec/repository/zoom.spec.md
import type { Page } from "@playwright/test"
import { expect, test } from "../fixtures/test-fixtures"
import { boxOf, expectHeaderControlsOnOneLine, HEADER_CONTROLS, MIN_WINDOW, zoomPage } from "../helpers/header-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"

/** The zoom the page gave its webview, 1 if none */
function webviewZoom(page: Page): Promise<number> {
  return page.evaluate(() => (window as unknown as { __e2eWebviewZoom?: number }).__e2eWebviewZoom ?? 1)
}

function minWindowWidth(page: Page): Promise<number | undefined> {
  return page.evaluate(() => (window as unknown as { __e2eWindowState: { minSize?: { width: number } } }).__e2eWindowState.minSize?.width)
}

test.describe("Zoom", () => {
  test.beforeEach(async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await page.setViewportSize(MIN_WINDOW)
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
  })

  test("should step through the levels with Zoom In, Zoom Out and Actual Size", async ({ page }) => {
    // the menu items (src-tauri/src/menu.rs) send `zoom` to the main window
    await emitTauriEvent(page, "zoom", "in")
    await expect.poll(() => webviewZoom(page)).toBe(1.1)
    await emitTauriEvent(page, "zoom", "in")
    await expect.poll(() => webviewZoom(page)).toBe(1.25)
    await emitTauriEvent(page, "zoom", "out")
    await expect.poll(() => webviewZoom(page)).toBe(1.1)
    await emitTauriEvent(page, "zoom", "reset")
    await expect.poll(() => webviewZoom(page)).toBe(1)
  })

  test("should zoom in with Ctrl++ as well as with the Ctrl+= of the menu item", async ({ page }) => {
    await page.keyboard.press("Control+Shift+Equal")
    await expect.poll(() => webviewZoom(page)).toBe(1.1)
  })

  test("should zoom the islands but not the toolbar, the title bar of the window", async ({ page }) => {
    const row = page.getByTestId("branch-row").first()
    const rowHeight = (await boxOf(row)).height

    for (let step = 0; step < 4; step++) {
      await emitTauriEvent(page, "zoom", "in")
    }
    await expect.poll(() => webviewZoom(page)).toBe(1.5)
    // the islands keep 768 CSS px: a zoom above 125% widens the minimum window, 1152 px at 150%
    await expect.poll(() => minWindowWidth(page)).toBe(1152)
    await zoomPage(page, 1.5, { width: 1152, height: MIN_WINDOW.height })

    // a row keeps its CSS px, so it is 150% on the screen, while the toolbar keeps its 40 px and every control on its line
    expect((await boxOf(row)).height).toBeCloseTo(rowHeight, 0)
    await expectHeaderControlsOnOneLine(page, HEADER_CONTROLS, 1.5)

    await emitTauriEvent(page, "zoom", "reset")
    await expect.poll(() => minWindowWidth(page)).toBe(MIN_WINDOW.width)
  })
})

test.describe("Zoom on macOS", () => {
  // the page tells macOS by the user agent (`isMac`)
  test.use({ userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko)" })

  test("should zoom in with ⌘= as well as with the ⌘+ of the menu item", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    await page.keyboard.press("Meta+Equal")
    await expect.poll(() => webviewZoom(page)).toBe(1.1)
  })
})
