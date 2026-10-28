// @spec spec/repository/settings.spec.md
import type { Page } from "@playwright/test"
import type { RepositoryConfig, SettingsChange } from "../../../src/lib/bindings"
import { test, testWithIssueLinks, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { expandBranch, findBranchRow, getBranchCommitRow } from "../helpers/branch-helpers"
import { openCommitPreview } from "../helpers/preview-helpers"
import { expectSettingsOpened, openSettingsWindow, settingsTab } from "../helpers/settings-helpers"
import { getEmittedEvents, getOpenedSubWindows, getSubWindowReadyOpen, openLinkedSubWindow, reopenLinkedSubWindow } from "../helpers/sub-window-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { windowCalls } from "../helpers/title-bar-helpers"
import { routeCommand } from "../helpers/wait-helpers"

const WINDOW_SIZE = { width: 720, height: 540 }

/**
 * Every line a row of Settings → Repository can show at once: a source, Use Default or Use Global, the reason that
 * stacked branches can't be turned off, a value of the worktree config, an invalid value; on Diffs & Merge a value of
 * the repository with Use Global and an invalid one; on Agents a value of the worktree config and an invalid one of the
 * repository
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
  pushSizeLimit: {
    value: "lots",
    scope: "local",
    localSet: true,
    invalid: "lots is not a size such as 50M or 1G, nor 0 or off",
    fallbackValue: "1G",
    fallbackScope: "global",
  },
  resolveByWords: { value: true, scope: "local", localSet: true, invalid: "maybe is not a boolean (true or false)", fallbackValue: false, fallbackScope: "global" },
  landing: { value: null, scope: null, localSet: false, invalid: null, fallbackValue: null, fallbackScope: null },
  agentLease: { value: false, scope: "worktree", localSet: true, invalid: null, fallbackValue: true, fallbackScope: "system" },
  agentSubjectBranch: { value: false, scope: "local", localSet: true, invalid: "maybe is not a boolean (true or false)", fallbackValue: true, fallbackScope: "global" },
  detectedGithubRepository: { fullName: "acme/app", source: "spaceMirror" },
  landingDefault: { route: "pullRequest", onSpace: false },
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
 * refused the notifications, every line of the rows of Repository. The sign-in runs against the fake GitHub, which
 * never gets the code; the window's other commands are answered in its page.
 */
async function openTallestSettings(page: Page, repo: TestRepositoryBuilder): Promise<Page> {
  await repo.setGitHubSignIn({ clientId: "client-1", signedIn: false })
  // teamcity by its install script on two servers, one of them refused; youtrack missing, no Homebrew: its command
  await repo.setCliTools({
    installed: [{ tool: "teamcity", version: "1.6.0", unmanaged: true }],
    latest: { teamcity: "1.7.0" },
    files: {
      "teamcity.auth": JSON.stringify([
        { server: "https://buildserver.example", status: "authenticated", user: { username: "jane.doe" }, token_expiry: "2026-11-21T16:58:19Z" },
        { server: "https://ci.example", status: "error", error: "token missing or could not be retrieved" },
      ]),
    },
  })
  const window = await openSettingsWindow(
    page,
    {},
    {
      beforeLoad: async window => {
        await window.setViewportSize(WINDOW_SIZE)
        await window.addInitScript(() => {
          Object.defineProperty(Notification, "permission", { get: () => "denied" })
        })
        await routeCommand(window, "get_repository_config", () => ({ status: "ok", data: TALLEST_REPOSITORY_CONFIG }))
        await routeCommand(window, "agent_cli_status", () => ({
          bd: "/Users/test/.local/bin/bd",
          skills: ["/Users/test/.claude/skills/branch-deck/SKILL.md", "/Users/test/.agents/skills/branch-deck/SKILL.md"],
        }))
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
    if (name === "Diffs & Merge") {
      await expect(window.getByTestId("setting-resolveByWords-source").getByRole("button", { name: "Use Global" })).toBeVisible()
      await expect(window.getByText("maybe is not a boolean (true or false)")).toBeVisible()
    }
    if (name === "Agents") {
      await expect(window.getByTestId("setting-agentLease-reason")).toBeVisible()
      await expect(window.getByTestId("agent-skill")).toContainText("Installed")
    }
    if (name === "Tools") {
      await expect(window.getByTestId("cli-tool-teamcity").getByTestId("cli-tool-server")).toHaveCount(2)
      await expect(window.getByTestId("cli-tool-youtrack").locator("pre")).toBeVisible()
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
  let repo: TestRepositoryBuilder
  test.beforeEach(async ({ setupRepo }) => {
    repo = await setupRepo("simple", { prepopulateStore: true })
  })

  test("should open at the page an open names, else at the page shown last, its tab focused", async ({ page, testStore }) => {
    // the menu item: no page, the one shown last (none yet: General)
    const window = await openSettingsWindow(page)
    const tabs = window.getByRole("tablist", { name: "Settings pages" })
    await expect(tabs).toHaveAttribute("aria-orientation", "vertical")
    // the page of the selected repository last, its name beside it
    await expect(tabs.getByRole("tab")).toHaveText([
      "General",
      "Appearance",
      "Diffs & Merge",
      "Notifications",
      "GitHub",
      "YouTrack",
      "Patronus",
      "Agents",
      "Tools",
      /^Repositoryrepo-\d+$/,
    ])
    await expect(settingsTab(window, "General")).toBeFocused()
    await expect(window.getByTestId("settings-page-general")).toBeVisible()

    // ↑ and ↓ switch the page
    await window.keyboard.press("ArrowDown")
    await expect(settingsTab(window, "Appearance")).toHaveAttribute("aria-selected", "true")
    await expect(window.getByTestId("settings-page-appearance")).toBeVisible()
    await window.keyboard.press("ArrowDown")
    await expect(window.getByTestId("settings-page-diffs")).toBeVisible()
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
    await expect.poll(async () => ((await testStore.get("appSettings")) as { settingsPage?: string } | null)?.settingsPage).toBe("notifications")

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

  test("should set every diff from Diffs & Merge, the Whitespace menu naming its choice", async ({ page, testStore }) => {
    const window = await openSettingsWindow(page, { page: "diffs" })
    // the main window saves what differs from the defaults
    await window.getByTestId("setting-diffViewMode").getByRole("button", { name: "Unified" }).click()
    await expect.poll(() => testStore.get("diffView")).toEqual({ mode: "unified" })

    const whitespace = window.getByTestId("setting-whitespacePolicy").getByRole("button")
    await expect(whitespace).toHaveText("Do Not Ignore")
    await whitespace.click()
    await window.getByRole("menuitemradio", { name: "Trim Whitespace" }).click()
    await expect(whitespace).toHaveText("Trim Whitespace")
    // a radio item keeps the menu open; Escape closes the menu only, not the window
    await window.keyboard.press("Escape")
    await expect(window.getByRole("menu")).toHaveCount(0)
    await expect.poll(() => testStore.get("diffView")).toEqual({ mode: "unified", whitespacePolicy: "trim" })

    const ligatures = window.getByTestId("setting-ligatures")
    await expect(ligatures).toContainText("keeps the width of its characters")
    await expect(ligatures.getByRole("switch")).toBeChecked()
    await ligatures.click()
    await expect(ligatures.getByRole("switch")).not.toBeChecked()
    await expect.poll(() => testStore.get("diffView")).toEqual({ mode: "unified", whitespacePolicy: "trim", ligatures: false })
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
    const window = await openTallestSettings(page, repo)
    // Windows: the title bar on Appearance, no Dock badge
    await expectEveryPageFits(window, ["General", "Appearance", "Diffs & Merge", "Notifications", "GitHub", "YouTrack", "Patronus", "Agents", "Tools", "Repository"])
    for (const id of ["general", "appearance", "diffs", "notifications", "youtrack", "patronus", "agents", "tools", "repository"]) {
      await window.getByTestId(`settings-tab-${id}`).click()
      await captureHtmlSnapshot(window.getByTestId("settings-window"), `settings-${id}`)
    }
  })
})

test.describe("Settings window on macOS", () => {
  test.use({ userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36" })

  test("should fit every page at 720 × 540 without scrolling, the Dock badge too", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    const window = await openTallestSettings(page, repo)
    await expect(window.getByTestId("settings-page-github")).toBeVisible()
    await settingsTab(window, "Notifications").click()
    await expect(window.getByTestId("setting-reviewBadge")).toBeVisible()
    await expectEveryPageFits(window, ["General", "Appearance", "Diffs & Merge", "Notifications", "GitHub", "YouTrack", "Patronus", "Agents", "Tools", "Repository"])
    // no title bar setting on macOS
    await settingsTab(window, "Appearance").click()
    await expect(window.getByTestId("setting-zoom")).toBeVisible()
    await expect(window.getByTestId("setting-systemTitleBar")).toHaveCount(0)
  })
})

testWithIssueLinks.describe("Settings of a window whose page loads", () => {
  testWithIssueLinks("zooms a review window whose page loaded after Actual Size", async ({ page }) => {
    // the main window zooms in, then opens the review of a commit: the window is built at 110%
    await emitTauriEvent(page, "zoom", "in")
    await expect.poll(() => webviewZoom(page)).toBe(1.1)
    await expandBranch(page, findBranchRow(page, "JIRA-456"))
    await openCommitPreview(page, getBranchCommitRow(page, "JIRA-456", "perf db: increase pool size to 50"))
    await page.keyboard.press("d")
    await expect.poll(async () => (await getOpenedSubWindows(page)).length).toBe(1)
    const [opened] = await getOpenedSubWindows(page)
    expect(opened!.store).toMatchObject({ appSettings: { zoom: 1.1 } })

    // Actual Size while the window's page still loads: no listener of it hears the change
    await emitTauriEvent(page, "zoom", "reset")
    await expect.poll(() => webviewZoom(page)).toBe(1)
    // the writer's change of it, answered to the main window
    const lastChange = async () => (await getEmittedEvents<SettingsChange>(page, "settings_changed")).at(-1)
    await expect.poll(async () => ((await lastChange())?.value as { zoom?: number } | undefined)?.zoom).toBe(1)
    const actualSize = await lastChange()
    expect(actualSize).toMatchObject({ key: "appSettings", origin: "main" })
    expect(actualSize!.version).toBeGreaterThan(opened!.storeVersion ?? 0)

    // the page starts from the snapshot at 110%, its load takes Actual Size, and the window shows at 100%
    const window = await openLinkedSubWindow(page, opened!)
    await expect(window.getByTestId("branch-review")).toBeVisible()
    await expect.poll(() => getSubWindowReadyOpen(window)).toBe(1)
    await expect.poll(() => webviewZoom(window)).toBe(1)
  })
})
