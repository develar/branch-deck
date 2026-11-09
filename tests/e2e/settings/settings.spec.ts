// @spec openspec/specs/repository/settings/spec.md
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
import { windowCalls, windowTheme } from "../helpers/title-bar-helpers"
import { routeCommand } from "../helpers/wait-helpers"

const WINDOW_SIZE = { width: 880, height: 640 }

/**
 * Every line a row of the pages of the git config can show at once: the repository's own value with Reset, a value of
 * the worktree config, an invalid value, the system config; on Agents the pull request off by the route
 */
const TALLEST_REPOSITORY_CONFIG: RepositoryConfig = {
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
  // Landing with Robots: the route Patronus, its source, Use Default and an invalid global push under it
  landing: {
    value: "patronus",
    scope: "local",
    localSet: true,
    invalid: "a push is taken only from .git/config of the repository, never from the global config",
    fallbackValue: null,
    fallbackScope: null,
  },
  agentLease: { value: false, scope: "worktree", localSet: true, invalid: null, fallbackValue: true, fallbackScope: "system" },
  agentSubjectBranch: { value: false, scope: "local", localSet: true, invalid: "maybe is not a boolean (true or false)", fallbackValue: true, fallbackScope: "global" },
  agentPullRequest: { value: true, scope: "local", localSet: true, invalid: null, fallbackValue: false, fallbackScope: null },
  reviewExperimental: { value: true, scope: "system", localSet: false, invalid: null, fallbackValue: true, fallbackScope: "system" },
  agentPullRequestOff: "The route into master is Push (branchdeck.landing): the work lands without a pull request",
  companions: [
    { name: "ci-config", path: "/Users/jane/src/ci-config", problem: null },
    { name: "deploy-scripts", path: "/Users/jane/src/deploy-scripts", problem: "/Users/jane/src/deploy-scripts is gone" },
  ],
  primary: null,
  detectedGithubRepository: { fullName: "acme/app", source: "spaceMirror" },
  landingDefault: { route: "pullRequest", onSpace: false },
  seq: 0,
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
 * The window at 880 × 640 with the tallest state of each page: the code of the GitHub sign-in, the warning that the OS
 * refused the notifications, every line of the rows of the pages of the git config. The sign-in runs against the fake GitHub, which
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
          bd: "/Users/test/.local/bin/brd",
          skills: ["/Users/test/.claude/skills/branch-deck/SKILL.md", "/Users/test/.agents/skills/branch-deck/SKILL.md"],
        }))
      },
    },
  )
  await window.getByTestId("settings-tab-accounts").click()
  await window.getByRole("button", { name: "Sign In to GitHub" }).click()
  await expect(window.getByTestId("github-user-code")).toHaveText("WDJB-MJHT")
  return window
}

/** The pages of the sidebar, in its order */
const PAGES = ["General", "Appearance", "Diffs", "Notifications", "Accounts", "Tools", "Branches", "Landing", "Agents"]

/** The pages that may scroll in their tallest state at 880 × 640 (openspec/specs/repository/settings/spec.md#the-window) */
const SCROLLING_PAGES = new Set(["Tools", "Landing"])

/** The pages of the git config, each shown on both scope tabs */
const SCOPED_PAGES = new Set(["Branches", "Landing", "Agents"])

/**
 * Each page of the sidebar in its tallest state, a page of the git config on both tabs, fits the island: nothing to
 * scroll at 880 × 640, but `SCROLLING_PAGES`; nothing wider than the island at any size
 */
async function expectEveryPageFits(window: Page, pages: string[]): Promise<void> {
  const island = window.getByTestId("settings-island")
  for (const name of pages) {
    await settingsTab(window, name).click()
    await expect(settingsTab(window, name)).toHaveAttribute("aria-selected", "true")
    for (const scope of SCOPED_PAGES.has(name) ? ["all", "repository"] : ["all"]) {
      if (SCOPED_PAGES.has(name)) {
        await window.getByTestId(`settings-scope-${scope}`).click()
        await expect(window.getByTestId(`settings-scope-${scope}`)).toHaveAttribute("aria-selected", "true")
      }
      await expectTallestState(window, name, scope)
      const { scrollHeight, clientHeight, scrollWidth, clientWidth } = await island.evaluate(element => ({
        scrollHeight: element.scrollHeight,
        clientHeight: element.clientHeight,
        scrollWidth: element.scrollWidth,
        clientWidth: element.clientWidth,
      }))
      expect(scrollWidth, `${name} (${scope}) is no wider than the island`).toBeLessThanOrEqual(clientWidth)
      if (!SCROLLING_PAGES.has(name)) {
        expect(scrollHeight, `${name} (${scope}) fits`).toBeLessThanOrEqual(clientHeight)
      }
    }
  }
}

/** What the tallest state of the page shows, before its height is measured */
async function expectTallestState(window: Page, name: string, scope: string): Promise<void> {
  if (name === "Notifications") {
    await expect(window.getByTestId("notification-permission")).toHaveAttribute("data-status", "denied")
  }
  if (name === "Branches" && scope === "repository") {
    // its own prefix, invalid; resolve by words its own, invalid; the review plan of the system config
    await expect(window.getByTestId("setting-resolveByWords-source").getByRole("button", { name: "Reset" })).toBeVisible()
    await expect(window.getByText("maybe is not a boolean (true or false)")).toBeVisible()
  }
  if (name === "Branches" && scope === "all") {
    await expect(window.getByTestId("setting-reviewExperimental-source")).toContainText("Set in the system Git config")
  }
  if (name === "Landing" && scope === "repository") {
    // the route of the repository with Reset and its invalid value, the companions, the push size limit of its own
    await expect(window.getByTestId("setting-landing-source").getByRole("button", { name: "Reset" })).toBeVisible()
    await expect(window.getByTestId("setting-githubRepository-reason")).toBeVisible()
    await expect(window.getByTestId("companion-row")).toHaveCount(2)
    await expect(window.getByTestId("setting-pushSizeLimit-source")).toBeVisible()
  }
  if (name === "Agents" && scope === "repository") {
    await expect(window.getByTestId("setting-agentLease-reason")).toBeVisible()
    await expect(window.getByTestId("setting-agentPullRequest-not-applied")).toBeVisible()
  }
  if (name === "Tools") {
    await expect(window.getByTestId("agent-skill")).toContainText("/Users/test/.local/bin/brd")
    await expect(window.getByTestId("cli-tool-teamcity").getByTestId("cli-tool-server")).toHaveCount(2)
    await expect(window.getByTestId("cli-tool-youtrack").locator("pre")).toBeVisible()
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
    // the pages of the app, then those of the git config of the repositories
    await expect(tabs.getByRole("tab")).toHaveText(["General", "Appearance", "Diffs", "Notifications", "Accounts", "Tools", "Branches", "Landing", "Agents"])
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
    await reopenSettings(page, window, { page: "accounts" }, 2)
    await expect(settingsTab(window, "Accounts")).toHaveAttribute("aria-selected", "true")
    await expect(settingsTab(window, "Accounts")).toBeFocused()
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

  test("should open a page of the git config at the scope an open names, else at All repositories, never the one of before", async ({ page }) => {
    const window = await openSettingsWindow(page, { page: "agents", scope: "repository" })
    const repository = window.getByTestId("settings-scope-repository")
    await expect(repository).toHaveAttribute("aria-selected", "true")
    await expect(repository).toHaveText(/^repo-\d+$/)
    await expect(window.getByTestId("settings-scope-line")).toHaveText(/^Changes here apply only to repo-\d+, saved in its \.git\/config$/)
    // ⌘, again: every repository, where a change can't go to the repository unseen
    await reopenSettings(page, window, {}, 2)
    await expect(window.getByTestId("settings-scope-all")).toHaveAttribute("aria-selected", "true")
    await expect(window.getByTestId("settings-scope-line")).toHaveText("What every repository uses, in the app and brd")
    // ← and → switch the scope
    await window.getByTestId("settings-scope-all").focus()
    await window.keyboard.press("ArrowRight")
    await expect(repository).toHaveAttribute("aria-selected", "true")
  })

  test("should show the page that has the settings of a page of an older build", async ({ page }) => {
    const window = await openSettingsWindow(page, { page: "repository" as never })
    await expect(settingsTab(window, "Branches")).toHaveAttribute("aria-selected", "true")
  })

  test("should focus the field an open names", async ({ page }) => {
    const window = await openSettingsWindow(page, { page: "branches", focus: "branchPrefix" })
    await expect(window.getByRole("textbox", { name: "Branch prefix", exact: true })).toBeFocused()
    // a tool, an account: their section
    await reopenSettings(page, window, { page: "tools", focus: "cli-teamcity" }, 2)
    await expect(window.getByTestId("cli-tool-teamcity")).toBeFocused()
    await reopenSettings(page, window, { page: "accounts", focus: "account-youtrack" }, 3)
    await expect(window.locator('[data-settings-focus="account-youtrack"]')).toBeFocused()
  })

  test("should open the guide of the branch prefix without a button that opens Settings", async ({ page }) => {
    const window = await openSettingsWindow(page, { page: "branches" })
    await window.getByRole("button", { name: "View Guide" }).click()
    const guide = window.getByTestId("branch-prefix-help-dialog")
    await expect(guide).toContainText("Settings → Branches, All repositories")
    await expect(guide.getByRole("button", { name: "Open Settings" })).toHaveCount(0)
  })

  test("should close on Escape, which takes the typing of a field back first", async ({ page }) => {
    const window = await openSettingsWindow(page, { page: "branches" })
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

  test("should set the theme of every window from Appearance, System following the system", async ({ page, testStore }) => {
    await page.emulateMedia({ colorScheme: "light" })
    const settings = await openSettingsWindow(page, { page: "appearance" })
    await settings.emulateMedia({ colorScheme: "light" })
    const picker = settings.getByTestId("setting-theme")
    await expect(picker.getByRole("button", { name: "System" })).toHaveAttribute("aria-pressed", "true")
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light")

    await picker.getByRole("button", { name: "Dark" }).click()
    await expect(picker.getByRole("button", { name: "Dark" })).toHaveAttribute("aria-pressed", "true")
    // both windows show the dark tokens, and the main window saves the choice and sets the theme of its frame
    await expect(settings.locator("html")).toHaveAttribute("data-theme", "dark")
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark")
    await expect.poll(() => page.evaluate(() => getComputedStyle(document.body).colorScheme)).toBe("dark")
    await expect.poll(async () => ((await testStore.get("appSettings")) as { theme?: string } | null)?.theme).toBe("dark")
    await expect.poll(() => windowTheme(page)).toBe("dark")

    await picker.getByRole("button", { name: "Light" }).click()
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light")
    await page.emulateMedia({ colorScheme: "dark" })
    // Light stays light whatever the system shows
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light")

    await picker.getByRole("button", { name: "System" }).click()
    await expect(page.locator("html")).toHaveAttribute("data-theme", "dark")
    await expect.poll(() => windowTheme(page)).toBeNull()
    await page.emulateMedia({ colorScheme: "light" })
    await expect(page.locator("html")).toHaveAttribute("data-theme", "light")
  })

  test("should set every diff from Diffs, the Whitespace menu naming its choice", async ({ page, testStore }) => {
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

  test("should fit every page at 880 × 640 without scrolling", async ({ page }) => {
    const window = await openTallestSettings(page, repo)
    // Windows: the title bar on Appearance, no Dock badge
    await expectEveryPageFits(window, PAGES)
    for (const id of ["general", "appearance", "diffs", "notifications", "tools"]) {
      await window.getByTestId(`settings-tab-${id}`).click()
      await captureHtmlSnapshot(window.getByTestId("settings-window"), `settings-${id}`)
    }
    // Accounts without the GitHub sign-in, whose code counts down
    await window.getByTestId("settings-tab-accounts").click()
    await captureHtmlSnapshot(window.locator('[data-settings-focus="account-youtrack"]'), "settings-accounts-youtrack")
    for (const id of ["branches", "landing", "agents"]) {
      await window.getByTestId(`settings-tab-${id}`).click()
      for (const scope of ["all", "repository"]) {
        await window.getByTestId(`settings-scope-${scope}`).click()
        await captureHtmlSnapshot(window.getByTestId("settings-window"), `settings-${id}-${scope}`)
      }
    }
  })

  test("should keep every page within the island at the smallest window", async ({ page }) => {
    const window = await openSettingsWindow(page, {}, { beforeLoad: window => window.setViewportSize({ width: 640, height: 440 }) })
    for (const name of PAGES) {
      await settingsTab(window, name).click()
      const { scrollWidth, clientWidth } = await window.getByTestId("settings-island").evaluate(element => ({ scrollWidth: element.scrollWidth, clientWidth: element.clientWidth }))
      expect(scrollWidth, `${name} at 640 × 440`).toBeLessThanOrEqual(clientWidth)
    }
  })
})

test.describe("Settings window on macOS", () => {
  test.use({ userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36" })

  test("should fit every page at 880 × 640 without scrolling, the Dock badge too", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    const window = await openTallestSettings(page, repo)
    await expect(window.getByTestId("settings-page-accounts")).toBeVisible()
    await settingsTab(window, "Notifications").click()
    await expect(window.getByTestId("setting-reviewBadge")).toBeVisible()
    await expectEveryPageFits(window, PAGES)
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
    await expect(window.getByTestId("review-window")).toBeVisible()
    await expect.poll(() => getSubWindowReadyOpen(window)).toBe(1)
    await expect.poll(() => webviewZoom(window)).toBe(1)
  })
})
