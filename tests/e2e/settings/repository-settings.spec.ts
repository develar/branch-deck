// @spec openspec/specs/repository/settings/spec.md
import type { Locator, Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "../helpers/branch-helpers"
import { settledIpc } from "../helpers/ipc-helpers"
import { closeAsTheAnswerLands, expectSettingsOpened, openSettingsWindow, recordWrites, settingsTab } from "../helpers/settings-helpers"
import { getOpenedSubWindows, openLinkedSubWindow } from "../helpers/sub-window-helpers"
import { emitTauriEvent, expectTeamSessionSyncedOnOpen, finishedSyncCount, nextAnswer, pressSyncHeld, viewEpoch, waitForSyncComplete } from "../helpers/sync-helpers"
import { TEST_SERVER_URL } from "../helpers/test-server-url"
import { windowCalls } from "../helpers/title-bar-helpers"
import { holdNextAnswer, recordCommand, routeCommandError, openTooltip } from "../helpers/wait-helpers"

/** The repository's name, as the page and its errors say it */
function nameOf(path: string): string {
  return path.split("/").at(-1)!
}

/** Opens Settings → Branches on the tab of the repository and waits for its settings */
async function openRepositorySettings(page: Page, focus?: string): Promise<Page> {
  const window = await openSettingsWindow(page, focus === undefined ? { page: "branches", scope: "repository" } : { page: "branches", scope: "repository", focus })
  await expect(window.getByTestId("setting-symbols").getByRole("button", { name: "Off", exact: true })).toBeEnabled()
  return window
}

/** Opens Settings → Landing on the tab of the repository and waits for its settings */
async function openRepositoryLanding(page: Page): Promise<Page> {
  const window = await openSettingsWindow(page, { page: "landing", scope: "repository" })
  await expect(window.getByTestId("setting-githubRepository").getByRole("textbox")).toBeEnabled()
  return window
}

/** The line of a row of the repository's own value, with its Reset */
function ownLine(path: string): string {
  return `Only in ${nameOf(path)}Reset`
}

/** Selects the repository in the main window, whose recent projects reach the window as a change of the writer */
async function selectRepository(page: Page, window: Page, testStore: { get: (key: string) => Promise<unknown> }, path: string): Promise<void> {
  await page.getByRole("combobox").click()
  await page.getByRole("option", { name: nameOf(path) }).click()
  await expect.poll(async () => ((await testStore.get("recentProjects")) as Array<{ path: string }> | null)?.[0]?.path).toBe(path)
  await expect
    .poll(() => window.evaluate(() => (globalThis as unknown as { __TAURI_STORE__: { recentProjects?: { path: string }[] } }).__TAURI_STORE__.recentProjects?.[0]?.path))
    .toBe(path)
}

function row(window: Page, key: string): Locator {
  return window.getByTestId(`setting-${key}`)
}

/** The line under the row of `key`: where its value comes from */
function source(window: Page, key: string): Locator {
  return window.getByTestId(`setting-${key}-source`)
}

test.describe("Settings → Branches and Landing, the tab of a repository", () => {
  test("should turn the symbol dependencies off, and back to the default, the main window syncing each change", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("simple")
    await syncAndWaitForBranches()

    const window = await openRepositorySettings(page)
    await expect(window.getByTestId("settings-scope-line")).toHaveText(`Changes here apply only to ${nameOf(repo.path)}, saved in its .git/config`)
    const symbols = row(window, "symbols")
    const on = symbols.getByRole("button", { name: "On", exact: true })
    const off = symbols.getByRole("button", { name: "Off", exact: true })
    await expect(on).toHaveAttribute("aria-pressed", "true")
    await expect(source(window, "symbols")).toHaveCount(0)

    // off: the main window syncs
    let syncs = await finishedSyncCount(page)
    await off.click()
    await expect(off).toHaveAttribute("aria-pressed", "true")
    await expect(source(window, "symbols")).toHaveText(ownLine(repo.path))
    await waitForSyncComplete(page, syncs)

    // Reset removes the line of .git/config: On, the default; the main window syncs again
    const reset = symbols.getByRole("button", { name: "Reset" })
    await reset.hover()
    await expect(openTooltip(window)).toHaveText("Removes it from .git/config: back to On, the default")
    syncs = await finishedSyncCount(page)
    await reset.click()
    await expect(on).toHaveAttribute("aria-pressed", "true")
    await expect(reset).toHaveCount(0)
    await expect(source(window, "symbols")).toHaveCount(0)
    await waitForSyncComplete(page, syncs)
  })

  test("should write the symbol dependencies during a held sync at once, the main window syncing it once after that sync", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("simple")
    await syncAndWaitForBranches()
    const window = await openRepositorySettings(page)
    const off = row(window, "symbols").getByRole("button", { name: "Off", exact: true })
    await expect(off).toHaveAttribute("aria-pressed", "false")

    // the write waits for no sync: .git/config has it while the sync of the main window is held
    const syncs = await pressSyncHeld(page, repo)
    const written = nextAnswer(window, "set_repository_config")
    await off.click()
    await written
    await expect(off).toHaveAttribute("aria-pressed", "true")
    await expect(page.getByTestId("sync-button")).toHaveAttribute("aria-busy", "true")
    await expect(window.locator("[data-slot=toast]")).toHaveCount(0)

    // the held sync ends, then the one sync of the change
    await repo.holdRemoteAnswers({ release: ["sync"] })
    await expect.poll(() => finishedSyncCount(page)).toBe(syncs + 2)
  })

  test("should save Report Only, its source this repository, which the main window syncs, and read it again when the window gets the focus", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("simple")
    await syncAndWaitForBranches()
    const window = await openRepositorySettings(page)
    const symbols = row(window, "symbols")
    // the default: On, from all repositories
    await expect(symbols.getByRole("button", { name: "On", exact: true })).toHaveAttribute("aria-pressed", "true")
    await expect(source(window, "symbols")).toHaveCount(0)

    const syncs = await finishedSyncCount(page)
    await symbols.getByRole("button", { name: "Report Only" }).click()
    await expect(symbols.getByRole("button", { name: "Report Only" })).toHaveAttribute("aria-pressed", "true")
    // the mode decides how the branches stack: the main window syncs
    await waitForSyncComplete(page, syncs)
    await expect(symbols.getByRole("button", { name: "On", exact: true })).toHaveAttribute("aria-pressed", "false")
    await expect(source(window, "symbols")).toHaveText(ownLine(repo.path))
    await expect(symbols).toContainText("Shown by brd commits, never used for the branches")

    // read from git again: the write is in .git/config
    const read = window.waitForResponse(response => response.url().endsWith("/invoke/get_repository_config"))
    await emitTauriEvent(window, "tauri://focus")
    await read
    await expect(symbols.getByRole("button", { name: "Report Only" })).toHaveAttribute("aria-pressed", "true")
  })

  test("should refuse a GitHub repository that is not owner/name in place, and save one that is, which the main window asks GitHub about", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple")
    const window = await openRepositoryLanding(page)
    const writes = recordWrites(window)
    const github = row(window, "githubRepository")
    const field = github.getByRole("textbox", { name: "GitHub repository" })
    // unset: the row says why no repository was detected
    await expect(source(window, "githubRepository")).toHaveText("None detected: neither origin nor its Space mirror is on GitHub")

    await field.fill("acme/app/x")
    await expect(field).toHaveAttribute("aria-invalid", "true")
    await field.press("Enter")
    await expect(window.getByTestId("setting-githubRepository-typing")).toHaveText("acme/app/x is not a GitHub repository: use owner/name")

    const pullRequestCalls = await repo.getPullRequestCalls()
    await field.fill("acme/app")
    await expect(window.getByTestId("setting-githubRepository-typing")).toHaveCount(0)
    await field.press("Enter")
    await expect(source(window, "githubRepository")).toHaveText(ownLine(repo.path))
    // one write: the refused text never reached the backend
    expect(writes).toEqual([{ repositoryPath: repo.path, change: { key: "githubRepository", value: "acme/app" }, scope: "local" }])
    await expect(field).toHaveValue("acme/app")
    // the main window asks GitHub for the pull requests of the repository it names now
    await expect.poll(() => repo.getPullRequestCalls()).toBeGreaterThan(pullRequestCalls)
  })

  test("should tell a failed write in a toast", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple")
    const window = await openRepositorySettings(page)
    await routeCommandError(window, "set_repository_config", { code: "internal", message: "could not lock config file .git/config: File exists" })
    const symbols = row(window, "symbols")
    await symbols.getByRole("button", { name: "Report Only" }).click()
    const toast = window.locator("[data-slot=toast]").filter({ hasText: `Cannot change the symbol dependencies of ${nameOf(repo.path)}` })
    await expect(toast).toContainText("could not lock config file .git/config: File exists")
    await expect(symbols.getByRole("button", { name: "Report Only" })).toHaveAttribute("aria-pressed", "false")
    await expect(symbols.getByRole("button", { name: "On", exact: true })).toHaveAttribute("aria-pressed", "true")
  })

  test("should keep a prefix saved while a mode's write runs, when the mode's older answer comes last", async ({ page, setupRepo }) => {
    // the template sets branchdeck.branchPrefix user-name in .git/config, no symbol dependencies mode
    const repo = await setupRepo("simple")
    const window = await openRepositorySettings(page)
    const reportOnly = row(window, "symbols").getByRole("button", { name: "Report Only" })
    const field = row(window, "branchPrefix").getByRole("textbox", { name: "Branch prefix" })
    await expect(field).toHaveValue("user-name")
    await expect(reportOnly).toHaveAttribute("aria-pressed", "false")

    // the mode is written and read again on the server, with the old prefix; its answer waits
    const modeWrite = await holdNextAnswer(window, "set_repository_config")
    await reportOnly.click()
    await modeWrite.ran

    // the prefix is saved meanwhile, and answered first: read after both writes, it holds the mode too
    const prefixWrite = window.waitForResponse(response => response.url().endsWith("/invoke/set_repository_config"))
    await field.click()
    await expect(field).toBeFocused()
    await field.fill("team")
    await field.press("Enter")
    await prefixWrite
    await expect(field).toHaveValue("team")
    await expect(reportOnly).toHaveAttribute("aria-pressed", "true")

    // the mode's answer comes last, with the old prefix and one write fewer: the page has read it, and keeps the prefix
    modeWrite.release()
    await settledIpc(window)
    await expect(field).toHaveValue("team")
    await expect(reportOnly).toHaveAttribute("aria-pressed", "true")

    // a read again counts as many writes or more (the server writes for other tests too) and is taken: the focus finds
    // a prefix set in a terminal meanwhile, which no write of the app counts
    await repo.setBranchPrefixInTerminal("terminal")
    await emitTauriEvent(window, "tauri://focus")
    await expect(field).toHaveValue("terminal")
    await expect(reportOnly).toHaveAttribute("aria-pressed", "true")
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

    await expect(page.getByTestId("ready-to-sync-card")).toContainText("user-name/virtual/<name>")
    // the main window subscribes again after the write, holding the view it shows
    const resubscribed = page.waitForRequest(request => request.url().endsWith("/invoke/subscribe_view") && request.postDataJSON().have !== undefined)
    await field.fill("team/")
    await field.press("Enter")
    await expect(field).toHaveValue("team")
    await resubscribed
    // the session of the repository's new prefix answers it: the main window names the branches by it; the sync
    // resolves it too
    await expect(page.getByTestId("ready-to-sync-card")).toContainText("team/virtual/<name>")
    await page.getByTestId("ready-to-sync-button").click()
    await expect(findBranchRow(page, "test-branch")).toBeVisible()

    // All repositories says so, with the way back here
    await window.getByTestId("settings-scope-all").click()
    const general = window.getByTestId("setting-branch-prefix-repository")
    await expect(general).toContainText("This repository uses its own prefix team")
    await general.getByRole("button", { name: /^Open repo-\d+$/ }).click()
    await expectSettingsOpened(page, { page: "branches", scope: "repository", focus: "branchPrefix" }, 2)
  })

  test("should open and sync the session of a prefix written here, for a repository synced before", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the template sets branchdeck.branchPrefix user-name in .git/config
    const repo = await setupRepo("simple")
    await syncAndWaitForBranches()
    const synced = await viewEpoch(page)
    expect(synced).toBeDefined()
    const window = await openRepositorySettings(page, "branchPrefix")
    const field = row(window, "branchPrefix").getByRole("textbox", { name: "Branch prefix" })
    await expect(field).toBeFocused()
    await expect(field).toHaveValue("user-name")
    const syncs = await recordCommand(page, "sync")

    await field.fill("team")
    await field.press("Enter")
    await expect(field).toHaveValue("team")

    // another session names the branches: the main window subscribes to it and syncs it as an opened repository, once
    await expectTeamSessionSyncedOnOpen(page, repo, synced, syncs)
  })

  test("should give the prefix back to all repositories once the repository's own is unset, and say so", async ({ page, setupRepo }) => {
    // the template sets branchdeck.branchPrefix user-name in .git/config
    const repo = await setupRepo("simple", { prepopulateStore: true })
    const window = await openSettingsWindow(page, { page: "branches" })
    const general = window.getByRole("textbox", { name: "Branch prefix", exact: true })
    await general.fill("jane")
    await general.press("Enter")
    await expect(window.getByTestId("setting-branch-prefix-repository")).toContainText("This repository uses its own prefix user-name")

    await window.getByTestId("settings-scope-repository").click()
    const prefix = row(window, "branchPrefix")
    const field = prefix.getByRole("textbox", { name: "Branch prefix" })
    await expect(field).toHaveValue("user-name")
    await expect(source(window, "branchPrefix")).toHaveText(ownLine(repo.path))
    const reset = prefix.getByRole("button", { name: "Reset" })
    await reset.hover()
    await expect(openTooltip(window)).toHaveText("Removes it from .git/config: back to jane, the default")
    await reset.click()
    await expect(reset).toHaveCount(0)
    await expect(field).toHaveValue("")
    await expect(field).toHaveAttribute("placeholder", "jane")
    await expect(source(window, "branchPrefix")).toHaveCount(0)
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
    const window = await openRepositoryLanding(page)
    const writes = recordWrites(window)
    const field = row(window, "githubRepository").getByRole("textbox", { name: "GitHub repository" })
    await field.fill("acme/typed")

    await selectRepository(page, window, testStore, otherPath)
    // the page of the other repository: its read fails, with Retry
    const error = window.getByTestId("settings-config-error")
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

  /** Types "acme/app" as the GitHub repository, then a limit git cannot take, which stays typed, unsaved: the blur of the first writes it */
  async function typeTheRepositoryAndALimit(window: Page): Promise<void> {
    await row(window, "githubRepository").getByRole("textbox", { name: "GitHub repository" }).fill("acme/app")
    await row(window, "pushSizeLimit").getByRole("textbox", { name: "Push size limit" }).fill("lots")
  }

  /** The window was destroyed, having written the GitHub repository once, and nothing else */
  async function expectTheRepositoryWrittenOnce(window: Page, writes: Array<Record<string, unknown>>, path: string): Promise<void> {
    await expect.poll(() => windowCalls(window)).toContain("destroy")
    expect(writes).toEqual([{ repositoryPath: path, change: { key: "githubRepository", value: "acme/app" }, scope: "local" }])
  }

  test("should save valid typing when the window closes, and nothing else", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple")
    const window = await openRepositoryLanding(page)
    const writes = recordWrites(window)
    await typeTheRepositoryAndALimit(window)
    // ⌘W or the close button: the window asks before it closes, the page saves first, then the window goes
    await emitTauriEvent(window, "tauri://close-requested")
    await expectTheRepositoryWrittenOnce(window, writes, repo.path)
  })

  test("should write a change once when the window closes as the write of its blur answers, before the field shows it", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple")
    const window = await openRepositoryLanding(page)
    const writes = recordWrites(window)
    // the blur writes the change; the close comes as its answer lands, while the field still holds the typing
    await closeAsTheAnswerLands(window, "set_repository_config")
    await typeTheRepositoryAndALimit(window)
    await expectTheRepositoryWrittenOnce(window, writes, repo.path)
  })

  /**
   * Types "acme/one" as the GitHub repository and leaves the field, whose write runs on the server and whose answer is
   * held, then types "acme/two" meanwhile: the answer of the first write, to release
   */
  async function editDuringTheWriteOfABlur(window: Page): Promise<{ field: Locator; release: () => void }> {
    const field = row(window, "githubRepository").getByRole("textbox", { name: "GitHub repository" })
    await field.fill("acme/one")
    const write = await holdNextAnswer(window, "set_repository_config")
    await field.blur()
    await write.ran
    await field.fill("acme/two")
    return { field, release: write.release }
  }

  /** The two values written in order, each once */
  function writtenInOrder(path: string): Array<Record<string, unknown>> {
    return ["acme/one", "acme/two"].map(value => ({ repositoryPath: path, change: { key: "githubRepository", value }, scope: "local" }))
  }

  test("should write an edit typed during the write of a blur once that write answered, when the window closes", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple")
    const window = await openRepositoryLanding(page)
    const writes = recordWrites(window)
    const { release } = await editDuringTheWriteOfABlur(window)
    // ⌘W while the first write waits for its answer: the close waits for it, then writes the edit
    await emitTauriEvent(window, "tauri://close-requested")
    release()
    await expect.poll(() => windowCalls(window)).toContain("destroy")
    expect(writes).toEqual(writtenInOrder(repo.path))
  })

  test("should write an edit typed and entered during the write of a blur once that write answered, and keep it in the field", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple")
    const window = await openRepositoryLanding(page)
    const writes = recordWrites(window)
    const { field, release } = await editDuringTheWriteOfABlur(window)
    await field.press("Enter")
    release()
    await expect.poll(() => writes.length).toBe(2)
    expect(writes).toEqual(writtenInOrder(repo.path))
    await expect(field).toHaveValue("acme/two")
    await expect(source(window, "githubRepository")).toHaveText(ownLine(repo.path))
  })

  test("should show only the value of every repository without a repository, and say how to set the rest", async ({ page, setupRepo }) => {
    await setupRepo("NO_REPO", { prepopulateStore: false })
    const window = await openSettingsWindow(page, { page: "landing", scope: "repository" })
    await expect(settingsTab(window, "Landing")).toHaveAttribute("aria-selected", "true")
    await expect(window.getByTestId("settings-scope-all")).toHaveCount(0)
    await expect(window.getByTestId("settings-scope-line")).toHaveText("What every repository uses, in the app and brd")
    await expect(window.getByTestId("landing-per-repository")).toHaveText("Select a repository to set its route into master, its GitHub repository and its companions")
    // the global config alone
    await expect(row(window, "pushSizeLimit").getByRole("textbox", { name: "Push size limit" })).toBeEnabled()
  })
})

/** Settings → Branches, All repositories: the row of the switch of `key`, its switch, the line of where its value comes from, the writes */
async function openDiffsSwitch(page: Page, key: string) {
  const window = await openSettingsWindow(page, { page: "branches" })
  const writes = recordWrites(window)
  const setting = window.getByTestId(`setting-${key}`)
  return { window, writes, setting, toggle: setting.getByRole("switch"), source: window.getByTestId(`setting-${key}-source`) }
}

test.describe("Settings → Branches, a switch of every repository", () => {
  test("should resolve conflicts by words in the global config, and say where the repository's own value wins", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    await syncAndWaitForBranches()
    const { window, writes, setting, toggle, source } = await openDiffsSwitch(page, "resolveByWords")
    await expect(source).toHaveCount(0)
    await expect(setting.getByRole("button", { name: "Copy branchdeck.resolveByWords" })).toBeAttached()
    await expect(toggle).toBeChecked()

    // off for every repository: the global config of the test repository (its own file), and the main window syncs
    const syncs = await finishedSyncCount(page)
    await toggle.click()
    await expect(toggle).not.toBeChecked()
    await expect(source).toHaveCount(0)
    await waitForSyncComplete(page, syncs)
    expect(writes).toEqual([{ repositoryPath: repo.path, change: { key: "resolveByWords", value: false }, scope: "global" }])

    // `git config branchdeck.resolveByWords true` in the repository meanwhile: read again when the window gets the focus
    const local = await page.request.post(`${TEST_SERVER_URL}/invoke/set_repository_config`, {
      headers: { "x-repository-id": repo.id },
      data: { repositoryPath: repo.path, change: { key: "resolveByWords", value: true } },
    })
    expect(local.ok()).toBe(true)
    await emitTauriEvent(window, "tauri://focus")
    // the value of every repository stays off; the repository's own is on
    await expect(source).toHaveText(`${nameOf(repo.path)} has its own value: OnOpen ${nameOf(repo.path)}`)
    await expect(toggle).not.toBeChecked()

    // its tab: the switch changes the repository's own value, not the global one
    await source.getByRole("button", { name: `Open ${nameOf(repo.path)}` }).click()
    await expect(toggle).toBeChecked()
    await toggle.click()
    await expect(toggle).not.toBeChecked()
    await expect.poll(() => writes.at(-1)).toEqual({ repositoryPath: repo.path, change: { key: "resolveByWords", value: false }, scope: "local" })
    await expect(source).toHaveText(`Same as all repositoriesReset`)

    // Reset removes it from .git/config: the global value applies again
    const reset = source.getByRole("button", { name: "Reset" })
    await reset.hover()
    await expect(openTooltip(window)).toHaveText("Removes it from .git/config: back to Off, from all repositories")
    await reset.click()
    await expect(source).toHaveCount(0)
    await expect(toggle).not.toBeChecked()
    expect(writes.at(-1)).toEqual({ repositoryPath: repo.path, change: { key: "resolveByWords", value: null }, scope: "local" })
  })

  test("should turn the review plan on and off for every repository from Branches", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    await syncAndWaitForBranches()
    const { writes, setting, toggle, source } = await openDiffsSwitch(page, "reviewExperimental")
    await expect(setting).toContainText("Review plan (experimental)")
    await expect(setting.getByRole("button", { name: "Copy branchdeck.reviewExperimental" })).toBeAttached()
    // off by default
    await expect(source).toHaveCount(0)
    await expect(toggle).not.toBeChecked()

    // on for every repository: the global config, and the main window syncs, whose header the review windows read
    let syncs = await finishedSyncCount(page)
    await toggle.click()
    await expect(toggle).toBeChecked()
    await waitForSyncComplete(page, syncs)
    expect(writes).toEqual([{ repositoryPath: repo.path, change: { key: "reviewExperimental", value: true }, scope: "global" }])

    // Review Changes of a branch: its window, opened after Settings, shows the review plan
    const opened = (await getOpenedSubWindows(page)).length
    await findBranchRow(page, "test-branch").click({ button: "right" })
    await page.getByRole("menuitem", { name: "Review Changes" }).click()
    await expect.poll(async () => (await getOpenedSubWindows(page)).length).toBe(opened + 1)
    const review = await openLinkedSubWindow(page, (await getOpenedSubWindows(page)).at(-1)!)
    await expect(review.getByTestId("branch-review-mark-reviewed")).toBeVisible()
    await review.close()

    // off again, in the same global config
    syncs = await finishedSyncCount(page)
    await toggle.click()
    await expect(toggle).not.toBeChecked()
    await waitForSyncComplete(page, syncs)
    expect(writes.at(-1)).toEqual({ repositoryPath: repo.path, change: { key: "reviewExperimental", value: false }, scope: "global" })
  })
})
