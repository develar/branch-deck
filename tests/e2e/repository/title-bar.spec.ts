// @spec spec/repository/title-bar.spec.md
import type { Page } from "@playwright/test"
import { expect, test, testWithConflicts } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchDetailsRow } from "../helpers/branch-helpers"
import { boxOf, expectHeaderControlsOnOneLine, header, HEADER_CONTROLS, MIN_WINDOW } from "../helpers/header-helpers"
import { getOpenedSubWindows, openSubWindowPage } from "../helpers/sub-window-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"
import { waitForConflictViewer } from "../helpers/wait-helpers"

/** What the window builder injects before the page loads (`title_bar_script` in src-tauri/src/title_bar.rs) */
async function useTitleBar(page: Page, kind: "overlay" | "custom"): Promise<void> {
  await page.addInitScript(kind => {
    ;(window as unknown as { __BD_TITLE_BAR__: string }).__BD_TITLE_BAR__ = kind
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
      ;(window as unknown as { __e2eWindowState: { fullscreen: boolean } }).__e2eWindowState.fullscreen = true
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

test.describe("Title bar of a sub-window", () => {
  /** The review window of `login`, opened by Review Changes of its menu, shown in the page */
  async function showBranchReviewWindow(page: Page) {
    await findBranchRow(page, "login").click({ button: "right" })
    await page.getByRole("menuitem", { name: "Review Changes" }).click()
    await expect.poll(async () => (await getOpenedSubWindows(page)).length).toBe(1)
    const [opened] = await getOpenedSubWindows(page)
    await openSubWindowPage(page, opened!)
    const window = page.getByTestId("branch-review")
    await expect(window.getByTestId("review-file-header").first()).toBeVisible()
    return window
  }

  test("should start the row of a review window after the traffic lights, its empty parts dragging the window", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_ownership")
    await syncAndWaitForBranches()
    await useTitleBar(page, "overlay")
    const window = await showBranchReviewWindow(page)

    const titleBar = window.getByRole("banner")
    await expect(titleBar).toHaveAttribute("data-title-bar", "overlay")
    await expect(titleBar).toHaveAttribute("data-tauri-drag-region", "deep")
    // the stack where the title would be, after the traffic lights; the diff view and the actions at the other end
    const stack = titleBar.getByTestId("branch-review-stack")
    expect((await boxOf(stack)).x).toBe(88)
    await expect(titleBar.getByRole("button", { name: "Split" })).toBeVisible()
    await expect(titleBar.getByTestId("branch-review-push")).toBeVisible()
    // one 40px line
    expect((await boxOf(titleBar)).height).toBe(40)
  })

  test("should end the row of a review window with the window buttons of a custom title bar", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_ownership")
    await syncAndWaitForBranches()
    await useTitleBar(page, "custom")
    const window = await showBranchReviewWindow(page)

    const controls = window.getByRole("banner").getByTestId("window-controls")
    await expect(controls).toBeVisible()
    await controls.getByRole("button", { name: "Close" }).click()
    expect(await windowCalls(page)).toContain("close")
  })

  testWithConflicts("should keep the row of a conflict window above the islands, which scroll under it", async ({ page, setupConflictRepo }) => {
    await setupConflictRepo("conflict_branches")
    await expandBranch(page, findBranchRow(page, "bug-fix"))
    const details = getBranchDetailsRow(page, "bug-fix")
    await waitForConflictViewer(page, details)
    await details.getByTestId("conflicting-files").getByRole("button", { name: "Open in Window" }).click()
    const [opened] = await getOpenedSubWindows(page)
    await useTitleBar(page, "overlay")
    await page.setViewportSize({ width: 960, height: 400 })
    await openSubWindowPage(page, opened!)

    const window = page.getByTestId("conflicting-files-window")
    const titleBar = window.getByRole("banner")
    await expect(titleBar.getByTestId("window-title")).toHaveText("bug-fix›Conflicting files")
    await expect(window.getByTestId("conflicting-files")).toBeVisible()
    // the islands scroll in their own container: the title bar stays at the top
    await window.getByRole("main").evaluate(main => main.scrollTo({ top: main.scrollHeight }))
    expect((await boxOf(titleBar)).y).toBe(0)
  })
})
