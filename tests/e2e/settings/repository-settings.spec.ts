// @spec spec/repository/settings.spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "../helpers/branch-helpers"
import { expectSettingsOpened, openSettingsWindow, recordWrites, settingsTab } from "../helpers/settings-helpers"
import { emitTauriEvent, finishedSyncCount, waitForSyncComplete } from "../helpers/sync-helpers"
import { TEST_SERVER_URL } from "../helpers/test-server-url"
import { windowCalls } from "../helpers/title-bar-helpers"
import { holdNextAnswer, routeCommandError } from "../helpers/wait-helpers"

/** docs stacks on ABC-1 (a level below it), or is a branch of its own */
async function expectDocsStacked(page: Page, stacked: boolean): Promise<void> {
  const docs = findBranchRow(page, "docs")
  if (stacked) {
    await expect(docs).toHaveAttribute("data-depth", "1")
  } else {
    await expect(docs).toBeVisible()
    await expect(docs).not.toHaveAttribute("data-depth", "1")
  }
}

/** The repository's name, as the page and its errors say it */
function nameOf(path: string): string {
  return path.split("/").at(-1)!
}

/** Opens Settings → Repository and waits for the settings of the repository */
async function openRepositorySettings(page: Page, focus?: string): Promise<Page> {
  const window = await openSettingsWindow(page, focus === undefined ? { page: "repository" } : { page: "repository", focus })
  await expect(window.getByTestId("repository-setting-stacks")).toBeVisible()
  return window
}

/**
 * Selects the repository in the main window, whose saved recent projects reach the window as the main window
 * broadcasts them in the app (`store-snapshot-update`: the mock of the store plugin reports no change to broadcast)
 */
async function selectRepository(page: Page, window: Page, testStore: { get: (key: string) => Promise<unknown> }, path: string): Promise<void> {
  await page.getByRole("combobox").click()
  await page.getByRole("option", { name: nameOf(path) }).click()
  let saved: unknown = null
  await expect
    .poll(async () => {
      saved = await testStore.get("recentProjects")
      return (saved as Array<{ path: string }> | null)?.[0]?.path
    })
    .toBe(path)
  await emitTauriEvent(window, "store-snapshot-update", { key: "recentProjects", value: saved })
}

function row(window: Page, key: string): Locator {
  return window.getByTestId(`repository-setting-${key}`)
}

test.describe("Settings → Repository", () => {
  test("should turn stacked branches off and on, and back to the default, the main window syncing each change", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // stacked branches on in .git/config: docs stacks on ABC-1 by the stack parents
    const repo = await setupRepo("stacked_issues")
    await syncAndWaitForBranches()
    await expectDocsStacked(page, true)

    const window = await openRepositorySettings(page)
    await expect(window.getByTestId("repository-settings")).toContainText(`Saved in .git/config of ${nameOf(repo.path)}, which bd reads too`)
    const stacks = row(window, "stacks")
    const toggle = stacks.getByRole("switch", { name: "Stacked branches" })
    await expect(toggle).toBeChecked()
    await expect(stacks.getByTestId("repository-setting-source")).toHaveText("Set in this repository")

    // off: the main window syncs, docs is a branch of its own
    let syncs = await finishedSyncCount(page)
    await toggle.click()
    await expect(toggle).not.toBeChecked()
    await waitForSyncComplete(page, syncs)
    await expectDocsStacked(page, false)

    // on again: stacked again
    syncs = await finishedSyncCount(page)
    await toggle.click()
    await expect(toggle).toBeChecked()
    await waitForSyncComplete(page, syncs)
    await expectDocsStacked(page, true)

    // Use Default removes the line of .git/config: off, the default, and no source to name
    const reset = stacks.getByRole("button", { name: "Use Default" })
    await reset.hover()
    await expect(window.getByRole("tooltip")).toHaveText("Removes it from .git/config: back to Off, the default")
    syncs = await finishedSyncCount(page)
    await reset.click()
    await expect(toggle).not.toBeChecked()
    await expect(reset).toHaveCount(0)
    await expect(stacks.getByTestId("repository-setting-source")).toHaveCount(0)
    await waitForSyncComplete(page, syncs)
    await expectDocsStacked(page, false)
  })

  test("should not offer to turn stacked branches off while their flat names would clash, and say what to do", async ({ page, setupRepo }) => {
    await setupRepo("stacked_branches")
    const window = await openRepositorySettings(page)
    const stacks = row(window, "stacks")
    await expect(stacks.getByRole("switch", { name: "Stacked branches" })).toBeChecked()
    await expect(stacks.getByRole("switch", { name: "Stacked branches" })).toBeDisabled()
    await expect(stacks.getByTestId("repository-setting-reason")).toHaveText(
      "Turning this off renames login and session to names like auth/login, which git cannot keep next to auth. Unstack them first",
    )
  })

  test("should save Report Only, its source this repository, which the main window syncs, and read it again when the window gets the focus", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await setupRepo("simple")
    await syncAndWaitForBranches()
    const window = await openRepositorySettings(page)
    const symbols = row(window, "symbols")
    // the default: On, no source
    await expect(symbols.getByRole("button", { name: "On", exact: true })).toHaveAttribute("aria-pressed", "true")
    await expect(symbols.getByTestId("repository-setting-source")).toHaveCount(0)

    const syncs = await finishedSyncCount(page)
    await symbols.getByRole("button", { name: "Report Only" }).click()
    await expect(symbols.getByRole("button", { name: "Report Only" })).toHaveAttribute("aria-pressed", "true")
    // the mode decides how the branches stack: the main window syncs
    await waitForSyncComplete(page, syncs)
    await expect(symbols.getByRole("button", { name: "On", exact: true })).toHaveAttribute("aria-pressed", "false")
    await expect(symbols.getByTestId("repository-setting-source")).toHaveText("Set in this repository")
    await expect(symbols).toContainText("Shown by bd commits, never used for the branches")

    // read from git again: the write is in .git/config
    const read = window.waitForResponse(response => response.url().endsWith("/invoke/get_repository_config"))
    await emitTauriEvent(window, "tauri://focus")
    await read
    await expect(symbols.getByRole("button", { name: "Report Only" })).toHaveAttribute("aria-pressed", "true")
  })

  test("should refuse a GitHub repository that is not owner/name in place, and save one that is, which the main window asks GitHub about", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple")
    const window = await openRepositorySettings(page)
    const writes = recordWrites(window)
    const github = row(window, "githubRepository")
    const field = github.getByRole("textbox", { name: "GitHub repository" })
    // unset: the row says why no repository was detected
    await expect(github.getByTestId("repository-setting-source")).toHaveText("None detected: neither origin nor its Space mirror is on GitHub")

    await field.fill("acme/app/x")
    await expect(field).toHaveAttribute("aria-invalid", "true")
    await field.press("Enter")
    await expect(github.getByTestId("repository-setting-error")).toHaveText("acme/app/x is not a GitHub repository: use owner/name")

    const pullRequestCalls = await repo.getPullRequestCalls()
    await field.fill("acme/app")
    await expect(github.getByTestId("repository-setting-error")).toHaveCount(0)
    await field.press("Enter")
    await expect(github.getByTestId("repository-setting-source")).toHaveText("Set in this repository")
    // one write: the refused text never reached the backend
    expect(writes).toEqual([{ repositoryPath: repo.path, change: { key: "githubRepository", value: "acme/app" } }])
    await expect(field).toHaveValue("acme/app")
    // the main window asks GitHub for the pull requests of the repository it names now
    await expect.poll(() => repo.getPullRequestCalls()).toBeGreaterThan(pullRequestCalls)
  })

  test("should tell a failed write in a toast", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple")
    const window = await openRepositorySettings(page)
    await routeCommandError(window, "set_repository_config", { code: "internal", message: "could not lock config file .git/config: File exists" })
    const toggle = row(window, "stacks").getByRole("switch", { name: "Stacked branches" })
    await toggle.click()
    const toast = window.locator("[data-slot=toast]").filter({ hasText: `Cannot turn on stacked branches in ${nameOf(repo.path)}` })
    await expect(toast).toContainText("could not lock config file .git/config: File exists")
    await expect(toggle).not.toBeChecked()
  })

  test("should keep a prefix saved while a switch's write runs, when the switch's older answer comes last", async ({ page, setupRepo }) => {
    // the template sets branchdeck.branchPrefix user-name in .git/config, stacked branches off
    await setupRepo("simple")
    const window = await openRepositorySettings(page)
    const toggle = row(window, "stacks").getByRole("switch", { name: "Stacked branches" })
    const field = row(window, "branchPrefix").getByRole("textbox", { name: "Branch prefix" })
    await expect(field).toHaveValue("user-name")
    await expect(toggle).not.toBeChecked()

    // the switch is written and read again on the server, with the old prefix; its answer waits
    const switchWrite = await holdNextAnswer(window, "set_repository_config")
    await toggle.click()
    await switchWrite.ran

    // the prefix is saved meanwhile, and answered first
    const prefixWrite = window.waitForResponse(response => response.url().endsWith("/invoke/set_repository_config"))
    await field.click()
    await expect(field).toBeFocused()
    await field.fill("team")
    await field.press("Enter")
    await prefixWrite
    await expect(field).toHaveValue("team")

    // the switch's answer comes last: it never takes the prefix back, and the settings are read again
    const readAgain = window.waitForResponse(response => response.url().endsWith("/invoke/get_repository_config"))
    switchWrite.release()
    await readAgain
    await expect(toggle).toBeChecked()
    await expect(field).toHaveValue("team")
  })

  test("should name the branches by the repository's own prefix over the app's, in the main window too", async ({ page, setupRepo }) => {
    // the template sets branchdeck.branchPrefix user-name in .git/config
    await setupRepo("simple", { prepopulateStore: true })
    const window = await openRepositorySettings(page, "branchPrefix")
    const prefix = row(window, "branchPrefix")
    const field = prefix.getByRole("textbox", { name: "Branch prefix" })
    // the open focuses the field once the settings are read
    await expect(field).toBeFocused()
    await expect(field).toHaveValue("user-name")

    await field.fill("team/")
    await field.press("Enter")
    await expect(field).toHaveValue("team")
    // the main window loads the prefix again and names the branches by it
    await expect(page.getByTestId("ready-to-sync-card")).toContainText("team/virtual/<name>")
    const sync = page.waitForRequest(request => request.url().endsWith("/invoke/sync_branches"))
    await page.getByTestId("ready-to-sync-button").click()
    expect((await sync).postDataJSON()).toMatchObject({ branchPrefix: "team" })
    await expect(findBranchRow(page, "test-branch")).toBeVisible()

    // General says so, with the way back here
    await settingsTab(window, "General").click()
    const general = window.getByTestId("setting-branch-prefix-repository")
    await expect(general).toContainText("This repository uses its own prefix team")
    await general.getByRole("button", { name: "Open Repository Settings" }).click()
    await expectSettingsOpened(page, { page: "repository", focus: "branchPrefix" }, 2)
  })

  test("should give the prefix back to General once the repository's own is unset, and say so", async ({ page, setupRepo }) => {
    // the template sets branchdeck.branchPrefix user-name in .git/config
    await setupRepo("simple", { prepopulateStore: true })
    const window = await openSettingsWindow(page, { page: "general" })
    const general = window.getByRole("textbox", { name: "Branch prefix", exact: true })
    await general.fill("jane")
    await general.press("Enter")
    await expect(window.getByTestId("setting-branch-prefix-repository")).toContainText("This repository uses its own prefix user-name")

    await settingsTab(window, "Repository").click()
    const prefix = row(window, "branchPrefix")
    const field = prefix.getByRole("textbox", { name: "Branch prefix" })
    await expect(field).toHaveValue("user-name")
    await expect(prefix.getByTestId("repository-setting-source")).toHaveText("Set in this repository")
    const reset = prefix.getByRole("button", { name: "Use Default" })
    await reset.hover()
    await expect(window.getByRole("tooltip")).toHaveText("Removes it from .git/config: back to jane, the prefix of General")
    await reset.click()
    await expect(reset).toHaveCount(0)
    await expect(field).toHaveValue("")
    await expect(field).toHaveAttribute("placeholder", "jane")
    await expect(prefix.getByTestId("repository-setting-source")).toHaveText("Set in General")
    // the main window names the branches by General's prefix now
    await expect(page.getByTestId("ready-to-sync-card")).toContainText("jane/virtual/<name>")
  })

  test("should write nothing when the repository is switched while a field holds typing", async ({ page, setupRepo, testStore }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    // a second recent project, which git cannot read
    const otherPath = "/nonexistent/branch-deck-e2e-other"
    await page.addInitScript(path => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      ;(window as any).__TAURI_STORE__.recentProjects.push({ path })
    }, otherPath)
    await page.reload()
    await expect(page.getByTestId("ready-to-sync-card")).toBeVisible()
    const window = await openRepositorySettings(page)
    const writes = recordWrites(window)
    const field = row(window, "githubRepository").getByRole("textbox", { name: "GitHub repository" })
    await field.fill("acme/typed")

    await selectRepository(page, window, testStore, otherPath)
    // the page of the other repository: its read fails, with Retry
    const error = window.getByTestId("repository-settings-error")
    await expect(error).toContainText(`Cannot read the settings of ${nameOf(otherPath)}`)
    await expect(error.getByRole("button", { name: "Retry" })).toBeVisible()

    // back to the first one: the typing is gone, and nothing was written
    await selectRepository(page, window, testStore, repo.path)
    await expect(field).toHaveValue("")
    expect(writes).toEqual([])
    // a write still works: the empty list above is no listener that never listened
    await field.fill("acme/app")
    await field.press("Enter")
    await expect.poll(() => writes.length).toBe(1)
  })

  test("should save valid typing when the window closes, and nothing else", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple")
    const window = await openRepositorySettings(page)
    const writes = recordWrites(window)
    await row(window, "githubRepository").getByRole("textbox", { name: "GitHub repository" }).fill("acme/app")
    // a prefix git cannot take stays typed, unsaved
    await row(window, "branchPrefix").getByRole("textbox", { name: "Branch prefix" }).fill("jane doe")

    // ⌘W or the close button: the window asks before it closes, the page saves first, then the window goes
    await emitTauriEvent(window, "tauri://close-requested")
    await expect.poll(() => windowCalls(window)).toContain("destroy")
    expect(writes).toEqual([{ repositoryPath: repo.path, change: { key: "githubRepository", value: "acme/app" } }])
  })

  test("should say what to do without a repository", async ({ page, setupRepo }) => {
    await setupRepo("NO_REPO", { prepopulateStore: false })
    const window = await openSettingsWindow(page, { page: "repository" })
    await expect(window.getByTestId("repository-settings-empty")).toHaveText("Select a repository to see its settings")
    await expect(settingsTab(window, "Repository")).toHaveAttribute("aria-selected", "true")
  })
})

test.describe("Settings → Diffs & Merge", () => {
  test("should resolve conflicts by words in the global config, and say where the repository's own value wins", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    await syncAndWaitForBranches()
    const window = await openSettingsWindow(page, { page: "diffs" })
    const writes = recordWrites(window)
    const setting = window.getByTestId("setting-resolveByWords")
    const toggle = setting.getByRole("switch")
    const source = window.getByTestId("setting-resolveByWords-source")
    await expect(source).toHaveText("On by default. A change goes to your global Git config, which bd reads too")
    await expect(setting).toContainText("branchdeck.resolveByWords")
    await expect(toggle).toBeChecked()

    // off for every repository: the global config of the test repository (its own file), and the main window syncs
    const syncs = await finishedSyncCount(page)
    await setting.click()
    await expect(toggle).not.toBeChecked()
    await expect(source).toHaveText("Set in your global Git config, which bd reads too")
    await waitForSyncComplete(page, syncs)
    expect(writes).toEqual([{ repositoryPath: repo.path, change: { key: "resolveByWords", value: false }, scope: "global" }])

    // `git config branchdeck.resolveByWords true` in the repository meanwhile: read again when the window gets the focus
    const local = await page.request.post(`${TEST_SERVER_URL}/invoke/set_repository_config`, {
      headers: { "x-repository-id": repo.id },
      data: { repositoryPath: repo.path, change: { key: "resolveByWords", value: true } },
    })
    expect(local.ok()).toBe(true)
    await emitTauriEvent(window, "tauri://focus")
    await expect(source).toHaveText(`Set in .git/config of ${nameOf(repo.path)}, which overrides your global Git configUse Global`)
    await expect(toggle).toBeChecked()

    // the switch changes the repository's own value, not the global one
    await setting.click()
    await expect(toggle).not.toBeChecked()
    await expect.poll(() => writes.at(-1)).toEqual({ repositoryPath: repo.path, change: { key: "resolveByWords", value: false }, scope: "local" })

    // Use Global removes it from .git/config: the global value applies again
    const reset = source.getByRole("button", { name: "Use Global" })
    await reset.hover()
    await expect(window.getByRole("tooltip")).toHaveText("Removes it from .git/config: back to Off, set in your global Git config")
    await reset.click()
    await expect(source).toHaveText("Set in your global Git config, which bd reads too")
    await expect(toggle).not.toBeChecked()
    expect(writes.at(-1)).toEqual({ repositoryPath: repo.path, change: { key: "resolveByWords", value: null }, scope: "local" })
  })
})
