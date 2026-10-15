// @spec spec/repository/title-bar.spec.md
import type { Page } from "@playwright/test"
import { expect, test } from "../fixtures/test-fixtures"
import { boxOf, expectHeaderControlsOnOneLine, header, HEADER_CONTROLS, MIN_WINDOW } from "../helpers/header-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"

/** What the window builder injects before the page loads (`MAIN_TITLE_BAR` in src-tauri/src/lib.rs) */
async function useTitleBar(page: Page, kind: "overlay" | "custom"): Promise<void> {
  await page.addInitScript((kind) => {
    (window as unknown as { __BD_TITLE_BAR__: string }).__BD_TITLE_BAR__ = kind
  }, kind)
}

function windowCalls(page: Page): Promise<string[]> {
  return page.evaluate(() => (window as unknown as { __e2eWindowCalls?: string[] }).__e2eWindowCalls ?? [])
}

test.describe("Title bar", () => {
  test.beforeEach(async ({ page }) => {
    await page.setViewportSize(MIN_WINDOW)
  })

  test("should start the toolbar after the traffic lights of an overlay title bar, and not in fullscreen", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await useTitleBar(page, "overlay")
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()

    await expect(header(page)).toHaveAttribute("data-title-bar", "overlay")
    // the empty parts of the toolbar drag the window
    await expect(header(page)).toHaveAttribute("data-tauri-drag-region", "deep")
    await expectHeaderControlsOnOneLine(page, HEADER_CONTROLS)
    // the traffic lights take the first 73px (14px inset, three 12px buttons), the toolbar starts 15px after them
    const selector = page.getByTestId("configuration-header").getByRole("combobox").first()
    expect((await boxOf(selector)).x).toBe(88)

    // fullscreen hides the traffic lights: the toolbar starts at the edge
    await page.evaluate(() => {
      (window as unknown as { __e2eWindowState: { fullscreen: boolean } }).__e2eWindowState.fullscreen = true
    })
    await emitTauriEvent(page, "tauri://resize", { width: MIN_WINDOW.width, height: MIN_WINDOW.height })
    await expect.poll(async () => (await boxOf(selector)).x).toBeLessThan(16)
    await expect(page.getByTestId("window-controls")).toHaveCount(0)
  })

  test("should end a custom title bar with the window buttons", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await useTitleBar(page, "custom")
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()

    await expect(header(page)).toHaveAttribute("data-title-bar", "custom")
    await expect(header(page)).toHaveAttribute("data-tauri-drag-region", "deep")
    const headerBox = await expectHeaderControlsOnOneLine(page, [...HEADER_CONTROLS, "window-controls"])
    const controls = page.getByTestId("window-controls")
    // flush with the right edge of the window, after the sync settings
    const controlsBox = await boxOf(controls)
    expect(controlsBox.x + controlsBox.width).toBeCloseTo(headerBox.x + headerBox.width, 0)

    await controls.getByRole("button", { name: "Minimize" }).click()
    await controls.getByRole("button", { name: "Maximize" }).click()
    // a maximized window offers Restore
    await expect(controls.getByRole("button", { name: "Restore" })).toBeVisible()
    await controls.getByRole("button", { name: "Restore" }).click()
    await expect(controls.getByRole("button", { name: "Maximize" })).toBeVisible()
    await controls.getByRole("button", { name: "Close" }).click()
    expect(await windowCalls(page)).toEqual(["minimize", "toggle_maximize", "toggle_maximize", "close"])
  })

  test("should keep the native title bar where the page doesn't draw one", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()

    await expect(header(page)).toHaveAttribute("data-title-bar", "native")
    await expect(header(page)).not.toHaveAttribute("data-tauri-drag-region")
    await expect(page.getByTestId("window-controls")).toHaveCount(0)
    expect((await boxOf(page.getByTestId("configuration-header").getByRole("combobox").first())).x).toBeLessThan(16)
    // the page asks the window nothing
    expect(await windowCalls(page)).toEqual([])
  })
})
