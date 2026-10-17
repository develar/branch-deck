// @spec spec/repository/title-bar.spec.md
import type { Page } from "@playwright/test"
import { expect, test, testWithConflicts } from "../fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchDetailsRow } from "../helpers/branch-helpers"
import { boxOf, expectHeaderControlsOnOneLine, header, HEADER_CONTROLS, MIN_WINDOW } from "../helpers/header-helpers"
import { getEmittedEvents, getOpenedSubWindows, openSubWindowPage } from "../helpers/sub-window-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"
import { TITLE_BARS, useTitleBar, windowCalls } from "../helpers/title-bar-helpers"
import { waitForConflictViewer } from "../helpers/wait-helpers"

/** The empty part of the main window's row, between the repository and the actions: it drags the window */
async function emptyPartOfRow(page: Page): Promise<{ x: number; y: number }> {
  const browse = await boxOf(page.getByTestId("browse-repository-button"))
  const update = await boxOf(page.getByTestId("update-project-button"))
  return { x: (browse.x + browse.width + update.x) / 2, y: 20 }
}

/** The window enters fullscreen, as the mock tells the page */
async function enterFullscreen(page: Page): Promise<void> {
  await page.evaluate(() => {
    ;(window as unknown as { __e2eWindowState: { fullscreen: boolean } }).__e2eWindowState.fullscreen = true
  })
  await emitTauriEvent(page, "tauri://resize", { width: MIN_WINDOW.width, height: MIN_WINDOW.height })
}

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

test.describe("Title bar", () => {
  test.beforeEach(async ({ page }) => {
    await page.setViewportSize(MIN_WINDOW)
  })

  test("should start the toolbar after the traffic lights of an overlay title bar, and not in fullscreen", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await useTitleBar(page, TITLE_BARS.macos)
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
    await enterFullscreen(page)
    await expect.poll(async () => (await boxOf(selector)).x).toBeLessThan(16)
    await expect(page.getByTestId("window-controls")).toHaveCount(0)
  })

  test("should end a custom title bar with the window buttons", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await useTitleBar(page, TITLE_BARS.windows)
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()

    await expect(header(page)).toHaveAttribute("data-title-bar", "custom")
    await expect(header(page)).toHaveAttribute("data-tauri-drag-region", "deep")
    // the main menu replaces the native menu bar
    const headerBox = await expectHeaderControlsOnOneLine(page, ["main-menu-button", ...HEADER_CONTROLS, "window-controls"])
    const controls = page.getByTestId("window-controls")
    // flush with the right edge of the window, after the sync settings
    const controlsBox = await boxOf(controls)
    expect(controlsBox.x + controlsBox.width).toBeCloseTo(headerBox.x + headerBox.width, 0)

    // the Segoe Fluent glyphs of the caption buttons: ChromeMinimize, ChromeMaximize, ChromeClose
    expect(await controls.getByRole("button").evaluateAll(buttons => buttons.map(it => it.textContent?.codePointAt(0)?.toString(16)))).toEqual(["e921", "e922", "e8bb"])
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
  test("should start the row of a review window after the traffic lights, its empty parts dragging the window", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("stacked_ownership")
    await syncAndWaitForBranches()
    await useTitleBar(page, TITLE_BARS.macos)
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
    await useTitleBar(page, TITLE_BARS.windows)
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
    await useTitleBar(page, TITLE_BARS.macos)
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

test.describe("Custom title bar on Linux", () => {
  test.beforeEach(async ({ page }) => {
    await page.setViewportSize(MIN_WINDOW)
  })

  test("should put the window buttons where the desktop's layout does, in its look", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // GNOME's default `appmenu:close`: Close alone at the end
    await useTitleBar(page, TITLE_BARS.gnomeClose)
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    const end = header(page).locator("[data-testid=window-controls][data-side=end]")
    await expect(end).toHaveAttribute("data-style", "gnome")
    expect(await end.getByRole("button").evaluateAll(buttons => buttons.map(it => it.getAttribute("aria-label")))).toEqual(["Close"])
    await expect(header(page).locator("[data-testid=window-controls][data-side=start]")).toHaveCount(0)
    // flush with the right edge, a 47px cell; out of the tab order, as native window buttons are
    const headerBox = await boxOf(header(page))
    const closeBox = await boxOf(end.getByRole("button", { name: "Close" }))
    expect(closeBox.x + closeBox.width).toBeCloseTo(headerBox.x + headerBox.width, 0)
    expect(closeBox.width).toBe(47)
    await expect(end.getByRole("button", { name: "Close" })).toHaveAttribute("tabindex", "-1")
    await expectHeaderControlsOnOneLine(page, ["main-menu-button", ...HEADER_CONTROLS, "window-controls"])
  })

  test("should split the buttons of a split layout, the start ones before the main menu", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await useTitleBar(page, TITLE_BARS.gnomeSplit)
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    const start = header(page).locator("[data-testid=window-controls][data-side=start]")
    expect(await start.getByRole("button").evaluateAll(buttons => buttons.map(it => it.getAttribute("aria-label")))).toEqual(["Close"])
    expect(
      await header(page)
        .locator("[data-testid=window-controls][data-side=end]")
        .getByRole("button")
        .evaluateAll(buttons => buttons.map(it => it.getAttribute("aria-label"))),
    ).toEqual(["Minimize", "Maximize"])
    const closeBox = await boxOf(start)
    expect(closeBox.x).toBe(0)
    expect((await boxOf(page.getByTestId("main-menu-button"))).x).toBeGreaterThan(closeBox.x + closeBox.width)
  })

  test("should draw KDE's buttons, dim them in an inactive window, and leave fullscreen with Restore", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await useTitleBar(page, TITLE_BARS.kde)
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    const end = header(page).locator("[data-testid=window-controls][data-side=end]")
    await expect(end).toHaveAttribute("data-style", "kde")

    await emitTauriEvent(page, "tauri://blur", null)
    await expect(header(page)).toHaveAttribute("data-window-inactive", "")
    await emitTauriEvent(page, "tauri://focus", null)
    await expect(header(page)).not.toHaveAttribute("data-window-inactive")

    await enterFullscreen(page)
    await end.getByRole("button", { name: "Restore" }).click()
    expect(await windowCalls(page)).toContain("set_fullscreen")
    expect(await windowCalls(page)).not.toContain("toggle_maximize")
  })

  test("should do what the system's double click does, and open the window menu on a right click of the empty row", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await useTitleBar(page, { ...TITLE_BARS.gnome, doubleClick: "minimize" })
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    const empty = await emptyPartOfRow(page)
    await header(page).dblclick({ position: empty })
    await expect.poll(() => windowCalls(page)).toContain("minimize")
    expect(await windowCalls(page)).not.toContain("toggle_maximize")

    await header(page).click({ button: "right", position: empty })
    await expect.poll(() => windowCalls(page)).toContain("show_window_menu")
    // a control keeps its own right click
    const calls = (await windowCalls(page)).length
    await page.getByTestId("sync-button").click({ button: "right" })
    expect(await windowCalls(page)).toHaveLength(calls)
  })
})

test.describe("Main menu", () => {
  test.beforeEach(async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await page.setViewportSize(MIN_WINDOW)
    await useTitleBar(page, TITLE_BARS.gnome)
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
  })

  test("should expand the menu bar in place of ☰, go through the menus, and collapse on Escape", async ({ page }) => {
    const button = page.getByTestId("main-menu-button")
    await button.click()
    const menu = page.getByTestId("main-menu")
    await expect(menu.getByTestId("main-menu-trigger")).toHaveText(["File", "View", "Help"])
    await expect(page.getByRole("menuitem", { name: /Sync Branches/ })).toBeVisible()
    await expect(page.getByRole("menuitemcheckbox", { name: "Auto-sync on Focus" })).toHaveAttribute("aria-checked", "true")
    // the repository selector under the expanded menu hides
    await expect(header(page).getByRole("combobox").first()).toBeHidden()

    // the open menu takes the focus, so the arrows go through the menus
    await expect.poll(() => page.evaluate(() => document.activeElement?.closest("[role=menu]") !== null)).toBe(true)
    await page.keyboard.press("ArrowRight")
    await expect(page.getByRole("menuitem", { name: /Zoom In/ })).toBeVisible()
    await page.keyboard.press("Escape")
    await expect(menu).toHaveCount(0)
    await expect(button).toBeVisible()
    await expect(header(page).getByRole("combobox").first()).toBeVisible()
  })

  test("should open from the keyboard and run the items", async ({ page }) => {
    await page.keyboard.press("Alt+Backslash")
    await expect(page.getByRole("menuitem", { name: /Sync Branches/ })).toBeVisible()
    await page.keyboard.press("Escape")
    await page.keyboard.press("Alt+KeyV")
    await expect(page.getByRole("menuitem", { name: /Actual Size/ })).toBeVisible()
    await page.keyboard.press("Escape")
    await page.keyboard.press("F10")
    await expect(page.getByRole("menuitem", { name: /Settings/ })).toBeVisible()
    await page.getByRole("menuitem", { name: /Settings/ }).click()
    await expect(page.getByTestId("settings-dialog")).toBeVisible()
    await expect(page.getByTestId("main-menu")).toHaveCount(0)
  })

  test("should highlight File on a lone Alt tap on Windows, not after another key", async ({ page }) => {
    // the E2E browser says it is Windows
    await page.keyboard.press("Alt")
    await expect(page.getByTestId("main-menu-trigger").first()).toBeFocused()
    await page.keyboard.press("Escape")
    await expect(page.getByTestId("main-menu")).toHaveCount(0)
    // Alt with another key is no tap
    await page.keyboard.down("Alt")
    await page.keyboard.press("KeyQ")
    await page.keyboard.up("Alt")
    await expect(page.getByTestId("main-menu")).toHaveCount(0)
  })

  test("should take the keys of the native menu: Ctrl+R syncs, Ctrl+, opens the settings", async ({ page }) => {
    const sync = page.getByTestId("sync-button")
    const count = Number(await sync.getAttribute("data-sync-count"))
    await page.keyboard.press("Control+KeyR")
    await expect.poll(async () => Number(await sync.getAttribute("data-sync-count"))).toBeGreaterThan(count)
    await page.keyboard.press("Control+Comma")
    await expect(page.getByTestId("settings-dialog")).toBeVisible()
  })

  test("should offer the system title bar in the settings, applied after a restart", async ({ page }) => {
    await page.keyboard.press("Control+Comma")
    const row = page.getByTestId("setting-systemTitleBar")
    await expect(row.getByRole("switch")).not.toBeChecked()
    await row.getByRole("switch").click()
    await expect(page.getByTestId("setting-systemTitleBar-restart")).toContainText("Applies after a restart")
  })
})

test.describe("Keys of the native menu", () => {
  test("should leave the keys to the native menu where the window has one", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await useTitleBar(page, TITLE_BARS.macos)
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    await expect(page.getByTestId("main-menu-button")).toHaveCount(0)
    const sync = page.getByTestId("sync-button")
    const count = await sync.getAttribute("data-sync-count")
    await page.keyboard.press("Control+KeyR")
    await page.waitForTimeout(300)
    await expect(sync).toHaveAttribute("data-sync-count", count!)
  })

  test("should send Sync from a sub-window to the main window", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await useTitleBar(page, TITLE_BARS.gnome)
    await setupRepo("stacked_ownership")
    await syncAndWaitForBranches()
    await showBranchReviewWindow(page)
    await page.keyboard.press("Control+KeyR")
    await expect.poll(async () => (await getEmittedEvents(page, "sync-branches")).length).toBe(1)
  })

  test("should show a fixed system title bar in the settings with its reason", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await useTitleBar(page, { kind: "native", reason: "tiling-wm" })
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    await page.evaluate(() => {
      ;(window as unknown as { __e2eEmitTauriEvent: (event: string, payload: unknown) => void }).__e2eEmitTauriEvent("open_settings", null)
    })
    const row = page.getByTestId("setting-systemTitleBar")
    await expect(row.getByRole("switch")).toBeChecked()
    await expect(row.getByRole("switch")).toBeDisabled()
    await expect(row).toContainText("A tiling window manager draws the titles")
  })
})

test.describe("Title bar on Linux", () => {
  test.use({ userAgent: "Mozilla/5.0 (X11; Linux aarch64) AppleWebKit/605.1.15 (KHTML, like Gecko)" })

  test("should drag only once the mouse moved, so a double click reaches the page and maximizes", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await page.setViewportSize(MIN_WINDOW)
    await useTitleBar(page, TITLE_BARS.gnome)
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    const empty = await emptyPartOfRow(page)

    // a press without a move drags nothing
    await page.mouse.click(empty.x, empty.y)
    expect(await windowCalls(page)).not.toContain("start_dragging")
    // a double click does what the system's does
    await page.mouse.dblclick(empty.x, empty.y)
    await expect.poll(() => windowCalls(page)).toContain("toggle_maximize")
    expect(await windowCalls(page)).not.toContain("start_dragging")
    // a press that moves past the threshold drags the window
    await page.mouse.move(empty.x, empty.y)
    await page.mouse.down()
    await page.mouse.move(empty.x + 10, empty.y + 2, { steps: 3 })
    await page.mouse.up()
    await expect.poll(() => windowCalls(page)).toContain("start_dragging")
  })
})

test.describe("Rounded window on Linux", () => {
  test("should round the window as the desktop does, square while maximized or tiled", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await page.setViewportSize(MIN_WINDOW)
    await useTitleBar(page, TITLE_BARS.gnome)
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    const rounded = () => page.evaluate(() => document.documentElement.hasAttribute("data-window-rounded"))
    await expect.poll(rounded).toBe(true)
    expect(await page.evaluate(() => getComputedStyle(document.body).borderTopLeftRadius)).toBe("12px")

    // GTK tells the page that the window is tiled: square, as GNOME's tiled windows are
    await emitTauriEvent(page, "window-tiled", true)
    await expect.poll(rounded).toBe(false)
    await emitTauriEvent(page, "window-tiled", false)
    await expect.poll(rounded).toBe(true)

    await header(page).locator("[data-testid=window-controls][data-side=end]").getByRole("button", { name: "Maximize" }).click()
    await expect.poll(rounded).toBe(false)
  })

  test("should not round a window where the system does it or can't", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await useTitleBar(page, TITLE_BARS.windows)
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    await expect(header(page)).toHaveAttribute("data-title-bar", "custom")
    expect(await page.evaluate(() => document.documentElement.hasAttribute("data-window-rounded"))).toBe(false)
  })
})
