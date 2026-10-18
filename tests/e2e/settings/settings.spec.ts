// @spec spec/repository/settings.spec.md
import type { Page } from "@playwright/test"
import type { GitHubAuthStatus, RepositoryConfig } from "../../../src/lib/bindings"
import { test, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { expectSettingsOpened, openSettingsWindow, settingsTab } from "../helpers/settings-helpers"
import { openLinkedSubWindow, reopenLinkedSubWindow } from "../helpers/sub-window-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"
import { windowCalls } from "../helpers/title-bar-helpers"
import { routeCommand } from "../helpers/wait-helpers"

const WINDOW_SIZE = { width: 720, height: 540 }
const SIGNED_OUT: GitHubAuthStatus = { signInAvailable: true, signedInAs: null, tokenSource: null, manageUrl: null }

/**
 * Every line a row of Settings → Repository can show at once: a source, Use Default or Use Global, the reason that
 * stacked branches can't be turned off, a value of the worktree config, an invalid value
 */
const TALLEST_REPOSITORY_CONFIG: RepositoryConfig = {
  stacks: { value: true, scope: "local", localSet: true, invalid: null, fallbackValue: false, fallbackScope: "global" },
  symbols: { value: "report", scope: "global", localSet: false, invalid: null, fallbackValue: "report", fallbackScope: "global" },
  githubRepository: { value: "acme/app-mirror", scope: "worktree", localSet: true, invalid: null, fallbackValue: null, fallbackScope: null },
  branchPrefix: {
    value: "jane doe",
    scope: "local",
    localSet: true,
    invalid: 'A prefix cannot contain spaces, "..", "@{" or any of ~ ^ : ? * [ \\',
    fallbackValue: null,
    fallbackScope: null,
  },
  detectedGithubRepository: { fullName: "acme/app", source: "spaceMirror" },
  stacksOffClashes: [
    { branch: "auth/login", beside: "auth" },
    { branch: "auth/session", beside: "auth" },
  ],
}

/** The main window opens Settings again (the native menu, a button) at `target`, the window shows the new open */
async function reopenSettings(page: Page, window: Page, target: object, count: number): Promise<void> {
  await emitTauriEvent(page, "open_settings", target)
  await reopenLinkedSubWindow(window, await expectSettingsOpened(page, target, count))
}

/** The page of the window, as the zoom the main window applied (`__e2eWebviewZoom` of its webview mock) */
function webviewZoom(page: Page): Promise<number | undefined> {
  return page.evaluate(() => (window as unknown as { __e2eWebviewZoom?: number }).__e2eWebviewZoom)
}

/**
 * The window at 720 × 540 with the tallest state of each page: the code of the GitHub sign-in, the warning that the OS
 * refused the notifications, every line of the rows of Repository. The window's own commands are answered in its page.
 */
async function openTallestSettings(page: Page): Promise<Page> {
  const window = await openSettingsWindow(
    page,
    {},
    {
      beforeLoad: async window => {
        await window.setViewportSize(WINDOW_SIZE)
        await window.addInitScript(() => {
          Object.defineProperty(Notification, "permission", { get: () => "denied" })
        })
        await routeCommand(window, "get_github_auth_status", () => ({ status: "ok", data: SIGNED_OUT }))
        await routeCommand(window, "get_repository_config", () => ({ status: "ok", data: TALLEST_REPOSITORY_CONFIG }))
        await routeCommand(window, "start_github_sign_in", () => ({
          status: "ok",
          data: { userCode: "WDJB-MJHT", verificationUri: "https://github.com/login/device", expiresIn: 900 },
        }))
        // GitHub never answers: the code stays
        await window.route("**/invoke/wait_github_sign_in", () => {})
      },
    },
  )
  await window.getByTestId("settings-tab-github").click()
  await window.getByRole("button", { name: "Sign In to GitHub" }).click()
  await expect(window.getByTestId("github-user-code")).toHaveText("WDJB-MJHT")
  return window
}

/** Each page of the sidebar in its tallest state fits the island: nothing to scroll at 720 × 540 */
async function expectEveryPageFits(window: Page, pages: string[]): Promise<void> {
  const island = window.getByTestId("settings-island")
  for (const name of pages) {
    await settingsTab(window, name).click()
    await expect(settingsTab(window, name)).toHaveAttribute("aria-selected", "true")
    if (name === "Notifications") {
      await expect(window.getByTestId("notification-permission")).toHaveAttribute("data-status", "denied")
    }
    if (name === "Repository") {
      // the reason that stacked branches can't be turned off, a value from the global config with its button
      await expect(window.getByTestId("repository-setting-reason")).toHaveCount(2)
    }
    const { scrollHeight, clientHeight } = await island.evaluate(element => ({ scrollHeight: element.scrollHeight, clientHeight: element.clientHeight }))
    expect(scrollHeight, `${name} fits`).toBeLessThanOrEqual(clientHeight)
  }
}

test.describe("Settings window", () => {
  test.beforeEach(async ({ setupRepo }) => {
    await setupRepo("simple", { prepopulateStore: true })
  })

  test("should open at the page an open names, else at the page shown last, its tab focused", async ({ page, testStore }) => {
    // the menu item: no page, the one shown last (none yet: General)
    const window = await openSettingsWindow(page)
    const tabs = window.getByRole("tablist", { name: "Settings pages" })
    await expect(tabs).toHaveAttribute("aria-orientation", "vertical")
    // the page of the selected repository last, its name beside it
    await expect(tabs.getByRole("tab")).toHaveText(["General", "Appearance", "Notifications", "GitHub", /^Repositoryrepo-\d+$/])
    await expect(settingsTab(window, "General")).toBeFocused()
    await expect(window.getByTestId("settings-page-general")).toBeVisible()

    // ↑ and ↓ switch the page
    await window.keyboard.press("ArrowDown")
    await expect(settingsTab(window, "Appearance")).toHaveAttribute("aria-selected", "true")
    await expect(window.getByTestId("settings-page-appearance")).toBeVisible()
    await window.keyboard.press("ArrowDown")
    await expect(window.getByTestId("settings-page-notifications")).toBeVisible()
    await window.keyboard.press("ArrowUp")
    await expect(window.getByTestId("settings-page-appearance")).toBeVisible()
    await expect(window.getByTestId("settings-page-general")).toHaveCount(0)
    // saved by the main window, where the next window opens
    await expect.poll(async () => ((await testStore.get("appSettings")) as { settingsPage?: string } | null)?.settingsPage).toBe("appearance")

    // a button that names a page: the open window shows it, its tab focused
    await reopenSettings(page, window, { page: "github" }, 2)
    await expect(settingsTab(window, "GitHub")).toHaveAttribute("aria-selected", "true")
    await expect(settingsTab(window, "GitHub")).toBeFocused()
    // the menu item again: the page stays
    await settingsTab(window, "Notifications").click()
    await reopenSettings(page, window, {}, 3)
    await expect(settingsTab(window, "Notifications")).toHaveAttribute("aria-selected", "true")

    // a new window starts at the page shown last
    await window.close()
    await emitTauriEvent(page, "open_settings", null)
    const next = await openLinkedSubWindow(page, await expectSettingsOpened(page, {}, 4))
    await expect(settingsTab(next, "Notifications")).toHaveAttribute("aria-selected", "true")
    await expect(settingsTab(next, "Notifications")).toBeFocused()
  })

  test("should focus the field an open names", async ({ page }) => {
    const window = await openSettingsWindow(page, { page: "general", focus: "branchPrefix" })
    await expect(window.getByRole("textbox", { name: "Branch prefix", exact: true })).toBeFocused()
  })

  test("should open the guide of the branch prefix without a button that opens Settings", async ({ page }) => {
    const window = await openSettingsWindow(page, { page: "general" })
    await window.getByRole("button", { name: "View Guide" }).click()
    const guide = window.getByTestId("branch-prefix-help-dialog")
    await expect(guide).toContainText("Settings → General, Branches")
    await expect(guide.getByRole("button", { name: "Open Settings" })).toHaveCount(0)
  })

  test("should close on Escape, which takes the typing of a field back first", async ({ page }) => {
    const window = await openSettingsWindow(page, { page: "general" })
    const field = window.getByRole("textbox", { name: "Branch prefix", exact: true })
    const saved = await field.inputValue()
    await field.fill("jane")
    await window.keyboard.press("Escape")
    await expect(field).toHaveValue(saved)
    expect(await windowCalls(window)).not.toContain("close")
    await window.keyboard.press("Escape")
    await expect.poll(() => windowCalls(window)).toContain("close")
  })

  test("should zoom every window from Appearance with the items of the View menu", async ({ page, testStore }) => {
    const window = await openSettingsWindow(page, { page: "appearance" })
    const level = window.getByTestId("setting-zoom-level")
    await expect(level).toHaveText("100%")
    await window.getByRole("button", { name: "Zoom In" }).click()
    await expect(level).toHaveText("110%")
    // the main window zooms with it, and saves the level
    await expect.poll(() => webviewZoom(page)).toBe(1.1)
    await expect.poll(async () => ((await testStore.get("appSettings")) as { zoom?: number } | null)?.zoom).toBe(1.1)
    await window.getByRole("button", { name: "Zoom In" }).click()
    await expect(level).toHaveText("125%")
    await window.getByRole("button", { name: "Zoom Out" }).click()
    await expect(level).toHaveText("110%")
    await window.getByRole("button", { name: "Actual Size" }).click()
    await expect(level).toHaveText("100%")
    await expect.poll(() => webviewZoom(page)).toBe(1)
  })

  test("should ask the OS for the permission to notify again when the window gets the focus", async ({ page }) => {
    const window = await openSettingsWindow(
      page,
      { page: "notifications" },
      {
        beforeLoad: settings =>
          settings.addInitScript(() => {
            Object.defineProperty(Notification, "permission", {
              get: () => (window as unknown as { __e2eNotificationPermission?: string }).__e2eNotificationPermission ?? "denied",
            })
          }),
      },
    )
    const permission = window.getByTestId("notification-permission")
    await expect(permission).toHaveAttribute("data-status", "denied")
    await expect(permission).toContainText("Notifications are off in System Settings")
    // allowed in System Settings meanwhile (the browser has no "granted" without a prompt: back to never asked)
    await window.evaluate(() => {
      ;(window as unknown as { __e2eNotificationPermission: string }).__e2eNotificationPermission = "default"
    })
    await emitTauriEvent(window, "tauri://focus")
    await expect(permission).toHaveAttribute("data-status", "notAsked")
  })

  test("should fit every page at 720 × 540 without scrolling", async ({ page }) => {
    const window = await openTallestSettings(page)
    // Windows: the title bar on Appearance, no Dock badge
    await expectEveryPageFits(window, ["General", "Appearance", "Notifications", "GitHub", "Repository"])
    for (const id of ["general", "appearance", "notifications", "repository"]) {
      await window.getByTestId(`settings-tab-${id}`).click()
      await captureHtmlSnapshot(window.getByTestId("settings-window"), `settings-${id}`)
    }
  })
})

test.describe("Settings window on macOS", () => {
  test.use({ userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36" })

  test("should fit every page at 720 × 540 without scrolling, the Dock badge too", async ({ page, setupRepo }) => {
    await setupRepo("simple", { prepopulateStore: true })
    const window = await openTallestSettings(page)
    await expect(window.getByTestId("settings-page-github")).toBeVisible()
    await settingsTab(window, "Notifications").click()
    await expect(window.getByTestId("setting-reviewBadge")).toBeVisible()
    await expectEveryPageFits(window, ["General", "Appearance", "Notifications", "GitHub", "Repository"])
    // no title bar setting on macOS
    await settingsTab(window, "Appearance").click()
    await expect(window.getByTestId("setting-zoom")).toBeVisible()
    await expect(window.getByTestId("setting-systemTitleBar")).toHaveCount(0)
  })
})
