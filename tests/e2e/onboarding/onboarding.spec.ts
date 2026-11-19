import type { Locator, Page } from "@playwright/test"
import { CLOSE_SAVE_LIMIT_MS } from "../../../src/lib/settings-close"
// @spec openspec/specs/repository/onboarding/spec.md
import { test, expect, expectErrorLog } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { expandBranch, findBranchRow, getBranchDetailsRow, pushFromRowMenu } from "../helpers/branch-helpers"
import { boxOf } from "../helpers/header-helpers"
import { expectPushedToast } from "../helpers/push-helpers"
import { pressAndExpectSettingsOpened, openSettingsWindow } from "../helpers/settings-helpers"
import {
  emitTauriEvent,
  expectTeamSessionSyncedOnOpen,
  finishedSyncCount,
  focusWindow,
  focusWindowRefused,
  runAndWaitForAnswer,
  runAndWaitForSync,
  syncBranches,
  syncOnWindowFocus,
  viewEpoch,
  waitForSyncComplete,
} from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { windowCalls } from "../helpers/title-bar-helpers"
import { openToast } from "../helpers/toast-helpers"
import { holdCommand, holdNextAnswer, recordCommand, routeCommandError, waitForStorePersistence, expectClosed, openTooltip } from "../helpers/wait-helpers"

/** The branch prefix as Settings… (⌘,) shows it on General; the header has no field of its own */
async function expectSettingsPrefix(page: Page, prefix: string): Promise<void> {
  await expect(page.getByTestId("configuration-header").getByRole("textbox", { name: "Branch prefix" })).toHaveCount(0)
  const settings = await openSettingsWindow(page, { page: "branches" })
  await expect(settings.getByRole("textbox", { name: "Branch prefix", exact: true })).toHaveValue(prefix)
  await settings.close()
}

/** Settings → Branches of the repository "simple", "jane" typed as the app's prefix and not saved: the window */
async function typePrefixInSettings(page: Page, setupRepo: (templateName: string, options: { prepopulateStore: boolean }) => Promise<TestRepositoryBuilder>): Promise<Page> {
  await setupRepo("simple", { prepopulateStore: true })
  const settings = await openSettingsWindow(page, { page: "branches" })
  await settings.getByRole("textbox", { name: "Branch prefix", exact: true }).fill("jane")
  return settings
}

/** Types the app's prefix into Settings → Branches and saves it with Enter; the window stays open */
async function saveAppPrefix(page: Page, prefix: string): Promise<Page> {
  const settings = await openSettingsWindow(page, { page: "branches" })
  const field = settings.getByRole("textbox", { name: "Branch prefix", exact: true })
  await field.fill(prefix)
  await field.press("Enter")
  return settings
}

/** The repository was synced an hour ago (`repositoryState`, set through the test server before the page opens) */
const SYNCED_AN_HOUR_AGO = 60 * 60 * 1000

/**
 * The first sync of a repository whose git identity git cannot form, whatever the global config says: the notes of the
 * sync fail with `no_identity`, and the git alert says it
 */
async function syncWithoutIdentity(
  page: Page,
  setupRepo: (templateName: string, options: { prepopulateStore: boolean }) => Promise<TestRepositoryBuilder>,
): Promise<{ repo: TestRepositoryBuilder; alert: Locator }> {
  expectErrorLog(page, /^Cannot sync the branches: Git user identity is not configured/)
  const repo = await setupRepo("simple", { prepopulateStore: true })
  await repo.setUserNameInTerminal("")
  await page.getByTestId("ready-to-sync-button").click()
  return { repo, alert: page.getByTestId("git-setup-alert") }
}

/** Adds a second recent project, never synced, to the injected store */
async function addRecentProject(page: Page, path: string): Promise<void> {
  await page.addInitScript(path => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const projects = (window as any).__TAURI_STORE__.recentProjects
    projects.push({ path })
  }, path)
}

test.describe("Onboarding", () => {
  test("pre-fills the prefix from the git identity and saves it only on Save", async ({ page, setupRepo }) => {
    await setupRepo("simple_no_prefix", { prepopulateStore: true })

    const welcomeCard = page.getByTestId("welcome-card")
    const prefixInput = welcomeCard.getByRole("textbox", { name: "Branch prefix to save" })
    // the template's committer is test@example.com
    await expect(prefixInput).toHaveValue("test")
    await expect(welcomeCard).toContainText("Branches will be named test/virtual/<name>")
    // nothing is applied before Save: Sync still waits for a prefix
    await expect(page.getByTestId("sync-button")).toBeDisabled()
    await expect(page.getByTestId("ready-to-sync-card")).toHaveCount(0)

    await welcomeCard.getByRole("button", { name: "Save" }).click()

    await expect(welcomeCard).toHaveCount(0)
    await expect(page.getByTestId("sync-button")).toBeEnabled()
    await expect(page.getByTestId("ready-to-sync-card")).toBeVisible()
    await expectSettingsPrefix(page, "test")
  })

  test("saves a typed prefix without its trailing slash and refuses one git cannot take", async ({ page, setupRepo }) => {
    await setupRepo("simple_no_prefix", { prepopulateStore: true })

    const welcomeCard = page.getByTestId("welcome-card")
    const prefixInput = welcomeCard.getByRole("textbox", { name: "Branch prefix to save" })
    const save = welcomeCard.getByRole("button", { name: "Save" })

    await prefixInput.fill("jane doe")
    await expect(prefixInput).toHaveAttribute("aria-invalid", "true")
    await expect(save).toBeDisabled()

    await prefixInput.fill("jane/")
    await expect(welcomeCard).toContainText("Branches will be named jane/virtual/<name>")
    await prefixInput.press("Enter")

    await expect(welcomeCard).toHaveCount(0)
    await expectSettingsPrefix(page, "jane")
  })

  test("saves the prefix typed in Settings when the window closes, and the window opened again shows it", async ({ page, setupRepo, testStore }) => {
    const settings = await typePrefixInSettings(page, setupRepo)
    // ⌘W with the typing in the field: the window saves it first, waits for the writer, then goes
    await emitTauriEvent(settings, "tauri://close-requested")
    await expect.poll(() => windowCalls(settings)).toContain("destroy")
    expect(((await testStore.get("appSettings")) as { globalUserBranchPrefix?: string } | null)?.globalUserBranchPrefix).toBe("jane")
    await expectSettingsPrefix(page, "jane")
  })

  test("closes Settings after a while when the save of the prefix typed gets no answer, and the main window says it is still saving, its Open Settings on the field", async ({
    page,
    setupRepo,
  }) => {
    const settings = await typePrefixInSettings(page, setupRepo)
    // the writer takes the change but its answer never comes
    const write = await holdNextAnswer(settings, "set_setting")
    await emitTauriEvent(settings, "tauri://close-requested")
    await write.ran
    expect(await windowCalls(settings)).not.toContain("destroy")
    await settings.clock.fastForward(CLOSE_SAVE_LIMIT_MS)
    await expect.poll(() => windowCalls(settings)).toContain("destroy")
    // no failure: Rust may still write it
    const toast = openToast(page, "Changing the branch prefix of every repository…")
    await expect(toast).toContainText("Branch Deck has not answered in 3 seconds and may still save the change")
    await pressAndExpectSettingsOpened(page, () => toast.getByRole("button", { name: "Open Settings" }).click(), { page: "branches", scope: "all", focus: "branchPrefix" }, 2)
  })

  test("changes the prefix in Settings on Enter or when the field is left, not on every key", async ({ page, setupRepo, testStore }) => {
    await setupRepo("simple", { prepopulateStore: true })
    const settings = await openSettingsWindow(page, { page: "branches" })
    const field = settings.getByRole("textbox", { name: "Branch prefix", exact: true })
    await field.fill("jane doe")
    await expect(field).toHaveAttribute("aria-invalid", "true")
    await field.press("Enter")
    // refused: nothing saved
    expect(((await testStore.get("appSettings")) as { globalUserBranchPrefix?: string } | null)?.globalUserBranchPrefix).not.toBe("jane doe")
    await field.fill("jane/")
    await expect(settings.getByTestId("setting-branch-prefix")).toContainText("Branches are named jane/virtual/<name>")
    await field.press("Enter")
    await expect.poll(async () => ((await testStore.get("appSettings")) as { globalUserBranchPrefix?: string } | null)?.globalUserBranchPrefix).toBe("jane")
    // Escape takes the typing back, the window stays
    await field.fill("john")
    await field.press("Escape")
    await expect(field).toHaveValue("jane")
    expect(await windowCalls(settings)).not.toContain("close")
  })

  test("names the branches by the repository's own prefix over the app's, and says so in Settings", async ({ page, setupRepo, testStore }) => {
    // the template sets branchdeck.branchPrefix user-name in .git/config
    const repo = await setupRepo("simple", { prepopulateStore: true })
    await expect(page.getByTestId("ready-to-sync-card")).toContainText("user-name/virtual/<name>")
    // the main window subscribes again once the writer has the app's prefix, holding the view it shows
    const resubscribed = page.waitForRequest(request => request.url().endsWith("/invoke/subscribe_view") && request.postDataJSON().have !== undefined)
    const settings = await saveAppPrefix(page, "jane")
    await expect.poll(async () => ((await testStore.get("appSettings")) as { globalUserBranchPrefix?: string } | null)?.globalUserBranchPrefix).toBe("jane")
    expect((await resubscribed).postDataJSON()).toMatchObject({ repositoryPath: repo.path })
    const setting = settings.getByTestId("setting-branch-prefix")
    await expect(setting).toContainText("Branches are named jane/virtual/<name>")
    await expect(settings.getByTestId("setting-branch-prefix-repository")).toContainText("This repository uses its own prefix user-name")

    // the session of the subscription is the one of the repository's own prefix, which Rust resolves over the app's: the
    // main window names the branches by it; the sync names only the repository, Rust resolves the same prefix for it
    await expect(page.getByTestId("ready-to-sync-card")).toContainText("user-name/virtual/<name>")
    const sync = page.waitForRequest(request => request.url().endsWith("/invoke/sync"))
    await page.getByTestId("ready-to-sync-button").click()
    expect((await sync).postDataJSON()).toEqual({ repositoryPath: repo.path, reason: "manual" })
    await expect(findBranchRow(page, "test-branch")).toBeVisible()
  })

  test("names the branches by the app's prefix where the repository has none", async ({ page, setupRepo }) => {
    await setupRepo("simple_no_prefix", { prepopulateStore: true })
    const settings = await saveAppPrefix(page, "jane")
    await expect(page.getByTestId("ready-to-sync-card")).toContainText("jane/virtual/<name>")
    await expect(settings.getByTestId("setting-branch-prefix")).toContainText("Branches are named jane/virtual/<name>")
    await expect(settings.getByTestId("setting-branch-prefix-repository")).toHaveCount(0)
  })

  test("disables Sync with the reason while no branch prefix is set", async ({ page, setupRepo }) => {
    await setupRepo("simple_no_prefix", { prepopulateStore: true })

    const syncButton = page.getByTestId("sync-button")
    await expect(syncButton).toBeDisabled()
    await syncButton.hover()
    await expect(page.getByTestId("sync-tooltip")).toHaveText("Set the branch prefix first")

    // the menu item (⌘R) does what the button does: nothing
    const syncRequests: string[] = []
    page.on("request", request => {
      if (request.url().endsWith("/invoke/sync")) {
        syncRequests.push(request.url())
      }
    })
    await emitTauriEvent(page, "sync_branches")
    await expect(syncButton).toHaveAttribute("data-sync-count", "0")
    expect(syncRequests).toEqual([])
  })

  test("says what Sync does before the first sync, then gives way to the branches", async ({ page, setupRepo }) => {
    await setupRepo("simple", { prepopulateStore: true })

    const card = page.getByTestId("ready-to-sync-card")
    await expect(card).toBeVisible()
    await expect(card).toContainText("user-name/virtual/<name>")
    await expect(card).toContainText("Your working tree and the checked-out branch stay as they are.")
    await expect(card.getByTestId("agents-hint")).toBeVisible()
    await captureHtmlSnapshot(card, "ready-to-sync-first-time")

    await card.getByTestId("ready-to-sync-button").click()

    await expect(page.locator("[data-testid=branch-row]").first()).toBeVisible()
    await expect(card).toHaveCount(0)
  })

  test("syncs under a prefix set in a terminal behind the window's back when Sync of the Ready to Sync card is pressed", async ({ page, setupRepo }) => {
    // the template sets branchdeck.branchPrefix user-name in .git/config; the window keeps the focus, so no resolve of the
    // prefix finds the change before the click
    const repo = await setupRepo("simple", { prepopulateStore: true })
    const card = page.getByTestId("ready-to-sync-card")
    await expect(card).toContainText("user-name/virtual/<name>")
    await repo.setBranchPrefixInTerminal("team")
    const syncs = await recordCommand(page, "sync")

    await card.getByTestId("ready-to-sync-button").click()

    // the session of the old prefix refuses the run, and the click is asked again of the session of the new one
    await expect(findBranchRow(page, "test-branch")).toBeVisible()
    await expect(card).toHaveCount(0)
    await expect(page.getByTestId("sync-button")).toBeEnabled()
    expect(syncs).toEqual([
      { repositoryPath: repo.path, reason: "manual" },
      { repositoryPath: repo.path, reason: "manual" },
    ])
    expect(await repo.revision("team/virtual/test-branch")).toMatch(/^[0-9a-f]{40}$/)
    // nothing was built under the prefix the config left
    await expect(repo.revision("user-name/virtual/test-branch")).rejects.toThrow("404")
  })

  test("syncs a repository synced before when the window opens it", async ({ page, setupRepo }) => {
    // a new window of a project the app synced an hour ago, Auto-sync on Focus off; the open sync of its session is held
    const repo = await setupRepo("behind_upstream", { prepopulateStore: true, syncedAgoMs: SYNCED_AN_HOUR_AGO, remoteAnswers: { hold: ["sync"] }, waitForSync: false })

    // the session syncs as it opens, which the view says (`synced`): Sync spins, the table loads, no card asks for a sync
    await expect(page.getByTestId("sync-button")).toHaveAttribute("aria-busy", "true")
    await expect(page.getByTestId("branch-table-loading")).toBeVisible()
    await expect(page.getByTestId("ready-to-sync-card")).toHaveCount(0)
    await repo.holdRemoteAnswers({ release: ["sync"] })

    // no click: the branches and Update show by themselves
    await expect(findBranchRow(page, "feature-auth")).toBeVisible()
    await expect(page.getByTestId("update-project-button")).toHaveAttribute("data-status", "clean")
    await expect(page.getByTestId("ready-to-sync-card")).toHaveCount(0)
    await expect(page.getByTestId("sync-button")).toHaveAttribute("data-sync-count", "1")
  })

  test("syncs a worktree that brd synced when the window opens it, though the app never synced it: no Ready to Sync card", async ({ page, setupRepo }) => {
    // an agent's `brd sync` in a terminal synced the worktree; the app never did (no Last synced), and the first sync of
    // the window's session is held
    const repo = await setupRepo("behind_upstream", { prepopulateStore: true, syncedByBrd: true, remoteAnswers: { hold: ["sync"] }, waitForSync: false })
    const sync = page.getByTestId("sync-button")
    const card = page.getByTestId("ready-to-sync-card")

    // the session syncs as it opens: Sync spins in the header, the table loads where its rows come, no card asks for
    // the first sync
    await expect(sync).toHaveAttribute("aria-busy", "true")
    await expect(page.getByTestId("branch-table-loading")).toBeVisible()
    await expect(card).toHaveCount(0)

    await repo.holdRemoteAnswers({ release: ["sync"] })
    await expect(findBranchRow(page, "feature-auth")).toBeVisible()
    await expect(sync).toHaveAttribute("data-sync-count", "1")
    await expect(card).toHaveCount(0)
  })

  test("syncs a slot of the lease pool the app synced under its lease now when the window opens it, and says when", async ({ page, setupRepo }) => {
    // an agent leased the slot, and the app synced it under that lease an hour ago; the open sync of the session is held
    const repo = await setupRepo("simple", { leaseSlot: "now", syncedAgoMs: SYNCED_AN_HOUR_AGO, remoteAnswers: { hold: ["sync"] }, waitForSync: false })
    await expect(page.getByTestId("sync-button")).toHaveAttribute("aria-busy", "true")
    // the sync of an hour ago, while the one of the open is held
    await expect(page.getByTestId("last-sync-time")).toHaveText(/^Last synced /)
    await expect(page.getByTestId("ready-to-sync-card")).toHaveCount(0)

    await repo.holdRemoteAnswers({ release: ["sync"] })
    await expect(findBranchRow(page, "test-branch")).toBeVisible()
  })

  test("shows the Ready to Sync card for a slot of the lease pool the app synced under an earlier lease, and no Last synced", async ({ page, setupRepo }) => {
    // the app synced the slot an hour ago, then the slot went back and an agent leased it again: the same path, another
    // worktree, whose first sync is the user's
    await setupRepo("simple", { leaseSlot: "again", syncedAgoMs: SYNCED_AN_HOUR_AGO })
    await expectNothingRunsBeforeTheFirstSync(page, async () => (await focusWindow(page))?.outcome)
    await expect(page.getByTestId("last-sync-time")).toHaveCount(0)
  })

  test("shows the Ready to Sync card in the window of a slot once an agent leased the slot again, without the rows and Last synced of the lease before", async ({
    page,
    setupRepo,
  }) => {
    const repo = await setupRepo("simple", { leaseSlot: "now", syncedAgoMs: SYNCED_AN_HOUR_AGO })
    await expect(findBranchRow(page, "test-branch")).toBeVisible()
    await expect(page.getByTestId("last-sync-time")).toBeVisible()
    // the slot goes back and an agent leases it again, the window still on it: brd touches the stamp after each
    await repo.leaseSlot(true)
    await repo.touchStamp()
    await expect(page.getByTestId("ready-to-sync-card")).toBeVisible()
    await expect(findBranchRow(page, "test-branch")).toHaveCount(0)
    await expect(page.getByTestId("last-sync-time")).toHaveCount(0)
    // nor room for the rows of the lease before (the loading placeholders of the table and of the landing indicator, by
    // the count of the branches of its sync), whatever the window of the lease before wrote of it late: the fields of a
    // lease are Rust's (`LEASE_FIELDS`)
    await expect(page.getByTestId("landing-indicator-loading")).toHaveCount(0)
    await expect(page.getByTestId("branch-table-loading")).toHaveCount(0)
  })

  test("shows the rows of the new lease of a slot an agent leased again under the window, none of them selected, expanded or focused as the row of the same name of the lease before was", async ({
    page,
    setupRepo,
  }) => {
    const repo = await setupRepo("simple", { leaseSlot: "now", syncedAgoMs: SYNCED_AN_HOUR_AGO })
    const row = findBranchRow(page, "test-branch")
    await expandBranch(page, row)
    await row.click({ modifiers: ["ControlOrMeta"] })
    await expect(row).toHaveAttribute("data-selected", "true")
    await expect(row).toBeFocused()
    const epoch = await viewEpoch(page)

    // the slot goes back and an agent leases it again, and its brd sync builds the same branch: brd touches the stamp
    await repo.leaseSlot(true)
    await repo.syncOutsideTheApp()
    await repo.touchStamp()
    await expect.poll(() => viewEpoch(page)).not.toBe(epoch)
    // the session of the new lease syncs itself, as its worktree was synced before
    await expect(row).toBeVisible()
    await expect(row).toHaveAttribute("data-selected", "false")
    await expect(row).toHaveAttribute("data-state", "closed")
    await expect(row).not.toBeFocused()
    await expect(page.locator(":focus")).toHaveCount(0)
  })

  test("forgets the branches of the previous repository on a switch", async ({ page, setupRepo }) => {
    expectErrorLog(page, /^Cannot subscribe to the branches of the repository: Repository not accessible: \/nonexistent\/branch-deck-e2e-other$/)
    const repo = await setupRepo("simple", { prepopulateStore: true, syncedAgoMs: SYNCED_AN_HOUR_AGO })
    // the injected store runs first, this script adds to it
    await addRecentProject(page, "/nonexistent/branch-deck-e2e-other")
    await page.reload()
    const branchRows = page.getByTestId("branch-row")
    await expect(branchRows.first()).toBeVisible()
    const repositorySelector = page.getByRole("combobox")

    await repositorySelector.click()
    await page.getByRole("option", { name: "branch-deck-e2e-other" }).click()
    await expect(page.getByTestId("error-alert")).toBeVisible()
    await expect(branchRows).toHaveCount(0)

    // back to the first one: it syncs by itself again
    await repositorySelector.click()
    await page.getByRole("option", { name: repo.path.split("/").pop() }).click()
    await expect(branchRows.first()).toBeVisible()
    await expect(page.getByTestId("ready-to-sync-card")).toHaveCount(0)
  })

  test("shows the loading table in the body during the first sync of another session of a repository synced before", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("simple")
    await syncAndWaitForBranches()
    const synced = await viewEpoch(page)
    expect(synced).toBeDefined()

    // a prefix set in a terminal is another session, which has no table yet; the window saw a sync end already. The
    // check of the focus finds it (`prefix_changed`), the window subscribes again, and the sync of the new session is
    // the held open
    await repo.setBranchPrefixInTerminal("team")
    await repo.holdRemoteAnswers({ hold: ["sync"] })
    await emitTauriEvent(page, "tauri://focus")
    await expect.poll(() => viewEpoch(page)).not.toBe(synced)
    const loading = page.getByTestId("branch-table-loading")
    // the session's sync waits in its first step
    await expect(loading).toContainText("Grouping the commits…")
    await expect(page.getByTestId("branch-row")).toHaveCount(0)
    await expect(page.getByTestId("error-alert")).toHaveCount(0)

    await repo.holdRemoteAnswers({ release: ["sync"] })
    await expect(findBranchRow(page, "test-branch")).toBeVisible()
    await expect(loading).toHaveCount(0)
  })

  test("explains the grouping in the unassigned commits while there is no branch", async ({ page, setupRepo }) => {
    await setupRepo("unassigned", { prepopulateStore: true })
    await syncBranches(page)

    await expect(page.getByTestId("unassigned-commits-hint")).toHaveText(
      "Start a commit subject with (name) or an issue key like ABC-123 to group it into a branch, or select commits and move them to one.",
    )
  })

  test("says how agents work once the Agent CLI is installed, and opens their settings", async ({ page, setupRepo }) => {
    await setupRepo("simple", { prepopulateStore: true })
    // Help → Install Agent CLI… of the native menu
    await emitTauriEvent(page, "install_agent_cli")
    const toast = page.locator("[data-slot=toast]", { hasText: "Agent CLI installed" })
    await expect(toast.locator("[data-slot=toast-description]")).toHaveText(
      "Coding agents find brd through the skills in /Users/test/.claude/skills/branch-deck/SKILL.md, /Users/test/.agents/skills/branch-deck/SKILL.md. In a terminal, run brd. Agents work in their own worktree and land each commit on your branch, and name the branch in each commit subject.",
    )
    await pressAndExpectSettingsOpened(page, () => toast.getByRole("button", { name: "Open Agent Settings" }).click(), { page: "agents" })
  })

  test("says in Ready to Sync where coding agents work, installs the Agent CLI there and opens Agent Settings", async ({ page, setupRepo }) => {
    await setupRepo("simple", { prepopulateStore: true })
    const hint = page.getByTestId("ready-to-sync-card").getByTestId("agents-hint")
    await expect(hint).toContainText(
      "You work in this checkout. With the Agent CLI, coding agents such as Claude Code lease worktrees of their own and land each commit on your branch.",
    )
    await captureHtmlSnapshot(page.getByTestId("ready-to-sync-card"), "ready-to-sync-agents")

    await hint.getByRole("button", { name: "Install Agent CLI" }).click()
    await expect(page.locator("[data-slot=toast]", { hasText: "Agent CLI installed" })).toBeVisible()
    await expect(hint).toContainText("You work in this checkout. Coding agents such as Claude Code lease worktrees of their own through brd and land each commit on your branch.")
    await expect(hint.getByRole("button", { name: "Install Agent CLI" })).toHaveCount(0)
    await pressAndExpectSettingsOpened(page, () => hint.getByRole("button", { name: "Open Agent Settings" }).click(), { page: "agents", scope: "repository", focus: "agentLease" })
  })

  test("names the baseline and where coding agents work in the empty state, and installs the Agent CLI there", async ({ page, setupRepo }) => {
    await setupRepo("archived_branches")
    await syncBranches(page, ['[data-testid="archived-branches-card"]'])
    const emptyState = page.getByTestId("empty-state-alert")
    await expect(emptyState.getByTestId("empty-state-baseline")).toHaveText(/^No commits ahead of \S+\.\s*$/)
    await expect(emptyState).toContainText("Commit to your branch using prefixes like:")
    const hint = emptyState.getByTestId("agents-hint")
    await hint.getByRole("button", { name: "Install Agent CLI" }).click()
    await expect(page.locator("[data-slot=toast]", { hasText: "Agent CLI installed" })).toBeVisible()
    await pressAndExpectSettingsOpened(page, () => hint.getByRole("button", { name: "Open Agent Settings" }).click(), { page: "agents", scope: "repository", focus: "agentLease" })
  })

  test("says in the help of the unassigned commits whether agents write the prefix here, and opens their settings", async ({ page, setupRepo }) => {
    await setupRepo("unassigned", { prepopulateStore: true })
    await syncBranches(page)
    await page.getByTestId("unassigned-commits-help-icon").hover()
    const line = page.getByTestId("unassigned-commits-agent-subjects")
    await expect(line).toContainText("Coding agents write (name) when they commit here.")
    await pressAndExpectSettingsOpened(page, () => line.getByRole("button", { name: "Open Agent Settings" }).click(), {
      page: "agents",
      scope: "repository",
      focus: "agentSubjectBranch",
    })
  })

  test("refuses a bare repository with the reason", async ({ page, setupRepo }) => {
    await setupRepo("bare", { prepopulateStore: false })
    await page.getByTestId("browse-repository-button").click()

    const alert = page.getByTestId("error-alert")
    await expect(alert).toContainText("Bare repositories are not supported: Branch Deck needs a working tree")
    // the alert explains the problem: the welcome card does not repeat the steps next to it
    await expect(page.getByTestId("welcome-card")).toHaveCount(0)
  })

  test("says that a repository without commits has nothing to sync yet", async ({ page, setupRepo }) => {
    expectErrorLog(page, /^Cannot sync the branches: The repository has no commits yet/)
    await setupRepo("empty_git", { prepopulateStore: true })
    // the alert comes with the answer of the sync, which takes seconds under load: the look at it waits for the answer
    await runAndWaitForAnswer(page, "sync", () => page.getByTestId("ready-to-sync-button").click())

    await expect(page.getByTestId("error-alert")).toContainText("The repository has no commits yet")
  })

  // the alert stood only while no sync ran: each sync of a repository whose syncs fail took it away, the rows moving up
  // under a still pointer, and brought it back as it failed again
  test("keeps the alert of a failed sync above the rows while the next sync runs and fails again, and drops it once one syncs", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await setupRepo("simple")
    await syncAndWaitForBranches()
    // the next two requests fail as a disk would; the one after them syncs
    await routeCommandError(page, "sync", { code: "internal", message: "The disk is full" }, { times: 2 })
    const alert = page.getByTestId("error-alert")
    const row = findBranchRow(page, "test-branch")
    const syncs = await finishedSyncCount(page)
    await page.getByTestId("sync-button").click()
    await expect(alert).toContainText("The disk is full")
    await expect(page.getByTestId("sync-button")).not.toHaveAttribute("aria-busy", "true")
    // the pointer rests on the row; ⌘R syncs again, its request held, and it fails again
    await row.hover()
    const before = await boxOf(row)
    const release = await holdCommand(page, "sync")
    await emitTauriEvent(page, "sync_branches")
    await expect(page.getByTestId("sync-button")).toHaveAttribute("aria-busy", "true")
    await expect(alert).toContainText("The disk is full")
    expect(await boxOf(row)).toEqual(before)
    release()
    await expect(page.getByTestId("sync-button")).not.toHaveAttribute("aria-busy", "true")
    await expect(alert).toContainText("The disk is full")
    expect(await boxOf(row)).toEqual(before)

    // a sync that syncs: the alert goes as it ends
    await page.getByTestId("sync-button").click()
    await waitForSyncComplete(page, syncs)
    await expect(alert).toHaveCount(0)
  })
})

test.describe("Deferred sync", () => {
  test("says in an info toast that a sync waits for git, and leaves the branches as they were", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("simple")
    await syncAndWaitForBranches()
    const rows = await page.getByTestId("branch-row").count()
    // the backend refuses the sync while git is in the middle of a rebase (`SyncDeferred`)
    await routeCommandError(
      page,
      "sync",
      { code: "operation_in_progress", message: "Not synced: a rebase is in progress" },
      { unlogged: "a deferred sync is a precondition, told in an info toast" },
    )
    await page.getByTestId("sync-button").click()
    const toast = page.locator("[data-slot=toast]").filter({ hasText: "Not synced: a rebase is in progress" })
    await expect(toast).toContainText("The branches sync once git finished it")
    // a precondition, not a failure: no error on Sync, the rows stay
    await expect(page.getByTestId("sync-button")).toBeEnabled()
    await expectClosed(page.getByTestId("sync-tooltip"))
    await expect(page.getByTestId("branch-row")).toHaveCount(rows)
  })
})

test.describe("A history unrelated to the baseline", () => {
  test("says above the table that a sync waits while the branch has history unrelated to the baseline, and syncs once the baseline is back", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("simple")
    await syncAndWaitForBranches()
    const rows = await page.getByTestId("branch-row").count()
    const master = await repo.revision("origin/master")
    // the server serves another repository as master, and a fetch takes it (a forced update)
    await repo.commitOnRemote("master", {
      subject: "Initial commit",
      author: "JitPack <bot@example.com>",
      timestamp: 1_700_000_000,
      files: { "README.md": "library\n" },
      root: true,
    })
    await repo.fetchInTerminal()

    // the session's own sync: deferred, the table of the sync before stays, the warning says which side lacks what
    expect(await focusWindowRefused(page)).toMatchObject({ code: "unrelated_history" })
    const alert = page.getByTestId("sync-deferred-alert")
    await expect(alert).toContainText("Not synced: master has history unrelated to origin/master")
    await expect(alert).toContainText("origin/master lacks")
    await expect(alert).toContainText("a root commit of master")
    await expect(page.getByTestId("branch-row")).toHaveCount(rows)
    await expect(page.getByTestId("error-alert")).toHaveCount(0)

    // the user's Sync says it in a warning toast too, and Sync shows no failure
    await page.getByTestId("sync-button").click()
    const toast = page.locator("[data-slot=toast]").filter({ hasText: "Not synced: master has history unrelated to origin/master" })
    await expect(toast).toContainText("origin/master lacks")
    await expect(page.getByTestId("sync-button")).toBeEnabled()
    await expectClosed(page.getByTestId("sync-tooltip"))

    // the real master is back on the remote and fetched: the next sync syncs, and the warning goes
    await repo.pushInTerminal("master", { revision: master })
    await syncOnWindowFocus(page)
    await expect(alert).toHaveCount(0)
    await expect(page.getByTestId("branch-row")).toHaveCount(rows)
  })
})

test.describe("A sync without a table", () => {
  test("says in the body that a sync waits for git while there is no table, and syncs once git finished", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true, syncedAgoMs: SYNCED_AN_HOUR_AGO, rebaseInProgress: true })
    const alert = page.getByTestId("sync-deferred-alert")
    await expect(alert).toContainText("Not synced: a rebase is in progress")
    await expect(alert).toContainText("The branches sync once git finished it")
    await expect(page.getByTestId("branch-table-loading")).toHaveCount(0)
    await expect(page.getByTestId("error-alert")).toHaveCount(0)

    // a focus while git is still at it finds what the deferred sync read: no run
    const syncs = await finishedSyncCount(page)
    expect(await focusWindowRefused(page)).toMatchObject({ code: "operation_in_progress" })
    expect(await finishedSyncCount(page)).toBe(syncs)
    await expect(alert).toBeVisible()

    // the rebase aborted moves no ref, and still is a change
    await repo.setRebaseInProgress(false)
    await syncOnWindowFocus(page)
    await expect(findBranchRow(page, "test-branch")).toBeVisible()
    await expect(alert).toHaveCount(0)
  })

  test("does not sync again on a focus after a failed sync while nothing changed, and syncs again on Sync", async ({ page, setupRepo }) => {
    expectErrorLog(page, /^Cannot sync the branches: The repository has no commits yet/)
    await setupRepo("empty_git", { prepopulateStore: true, syncedAgoMs: SYNCED_AN_HOUR_AGO })
    const alert = page.getByTestId("error-alert")
    await expect(alert).toContainText("The repository has no commits yet")
    const syncs = await finishedSyncCount(page)

    expect(await focusWindowRefused(page)).toMatchObject({ message: expect.stringContaining("The repository has no commits yet") })
    expect(await finishedSyncCount(page)).toBe(syncs)
    await expect(alert).toBeVisible()

    await page.getByTestId("sync-button").click()
    await waitForSyncComplete(page, syncs)
    await expect(alert).toContainText("The repository has no commits yet")
  })
})

test.describe("Git setup", () => {
  test("says above everything else that git is missing, and opens the repository once Check Again finds it", async ({ page, setupRepo }) => {
    // the first command that runs git, the subscription of the window, finds none: its words are the summary, then the hint
    const missing = "Cannot find Git: git is not on PATH\nInstall or upgrade Git with: brew install git"
    await routeCommandError(page, "subscribe_view", { code: "git_not_found", message: missing }, { times: 1 })
    await setupRepo("simple", { prepopulateStore: true, waitForSync: false })
    const alert = page.getByTestId("git-setup-alert")
    await expect(alert.getByText("Cannot find Git: git is not on PATH")).toBeVisible()
    await expect(alert.getByText("brew install git", { exact: true })).toBeVisible()
    // it replaces the errors that would say the same, and the welcome card
    await expect(page.getByTestId("error-alert")).toHaveCount(0)
    await expect(page.getByTestId("welcome-card")).toHaveCount(0)

    const resubscribed = page.waitForRequest(request => request.url().endsWith("/invoke/subscribe_view"))
    await alert.getByRole("button", { name: "Check Again" }).click()
    await resubscribed
    await expect(alert).toHaveCount(0)
    await expect(page.getByTestId("ready-to-sync-card")).toBeVisible()
  })

  // the alert stood only while no sync ran: Sync Again took it away, its button from under the pointer that pressed it,
  // and a sync that failed again brought it back
  test("keeps the git alert and its Sync Again under the pointer while the sync it asked for runs and fails again", async ({ page, setupRepo }) => {
    const { repo, alert } = await syncWithoutIdentity(page, setupRepo)
    const again = alert.getByRole("button", { name: "Sync Again" })
    const before = await boxOf(again)
    // the sync Sync Again asks for waits in its first step
    await repo.holdRemoteAnswers({ hold: ["sync"] })
    const syncs = await finishedSyncCount(page)
    const failed = await page.getByTestId("sync-button").getAttribute("data-view-version")
    await again.click()
    // its run runs, held in its first step, and the view says it runs: the patch of its start is the next one. Busy
    // alone is the window's count of its requests, which comes before the view, still failed then
    await repo.arrival("sync")
    await expect(page.getByTestId("sync-button")).not.toHaveAttribute("data-view-version", failed!)
    await expect(alert).toContainText("Git does not know your name and email")
    expect(await boxOf(again)).toEqual(before)
    await repo.holdRemoteAnswers({ release: ["sync"] })
    await waitForSyncComplete(page, syncs)
    await expect(alert).toContainText("Git does not know your name and email")
    expect(await boxOf(again)).toEqual(before)
  })

  test("warns that git does not know the user's name and email when a sync fails on it, and syncs on Sync Again", async ({ page, setupRepo }) => {
    const { repo, alert } = await syncWithoutIdentity(page, setupRepo)
    await expect(alert).toContainText("Git does not know your name and email")
    await expect(alert.getByText('git config --global user.name "Your Name"')).toBeVisible()
    await expect(alert.getByText("git config --global user.email you@example.com")).toBeVisible()
    // instead of the error alert, which would say the same
    await expect(page.getByTestId("error-alert")).toHaveCount(0)

    await repo.setUserNameInTerminal("Test User")
    const syncs = await finishedSyncCount(page)
    await alert.getByRole("button", { name: "Sync Again" }).click()
    await waitForSyncComplete(page, syncs)
    await expect(findBranchRow(page, "test-branch")).toBeVisible()
    await expect(alert).toHaveCount(0)
  })
})

/**
 * The Ready to Sync card of a repository never synced stays over `ask`, a focus or a stamp of `brd`, which the session
 * answers `skipped`: no sync ran before, none after
 */
async function expectNothingRunsBeforeTheFirstSync(page: Page, ask: () => Promise<string | undefined>): Promise<void> {
  const card = page.getByTestId("ready-to-sync-card")
  const sync = page.getByTestId("sync-button")
  await expect(card).toBeVisible()
  await expect(sync).toHaveAttribute("data-sync-count", "0")
  expect(await ask()).toBe("skipped")
  await expect(card).toBeVisible()
  await expect(sync).toHaveAttribute("data-sync-count", "0")
}

test.describe("Sync on focus", () => {
  /** Rewords the commit of feature-docs in a terminal, into feature-documentation: a change the next sync shows */
  async function rewordDocsInTerminal(page: Page, repo: TestRepositoryBuilder): Promise<void> {
    await expandBranch(page, findBranchRow(page, "feature-docs"))
    const hash = await getBranchDetailsRow(page, "feature-docs").locator("tr[data-row-id]").getAttribute("data-row-id")
    await repo.reword(hash!, "(feature-documentation) Update documentation")
  }

  test("syncs nothing on a focus before the first sync, which is the user's: the Ready to Sync card stays", async ({ page, setupRepo }) => {
    await setupRepo("simple", { prepopulateStore: true })
    // Auto-sync on Focus is on: the window tells its session the focus, and the session answers that it ran nothing,
    // as the first sync writes the branches and the notes
    await expectNothingRunsBeforeTheFirstSync(page, async () => (await focusWindow(page))?.outcome)
  })

  test("syncs on a focus after the user's first Sync once what a sync reads changed", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    const card = page.getByTestId("ready-to-sync-card")
    await card.getByTestId("ready-to-sync-button").click()
    await waitForSyncComplete(page, 0)
    await expect(findBranchRow(page, "test-branch")).toBeVisible()
    await expect(card).toHaveCount(0)

    // a commit in a terminal: the next focus syncs it
    await repo.commit("(focus-docs) Add the docs", { "focus-docs.md": "docs\n" }, 1_704_200_000)
    await syncOnWindowFocus(page)
    await expect(findBranchRow(page, "focus-docs")).toBeVisible()
  })

  test("syncs nothing on a stamp of brd before the first sync, as on a focus, and what a mutation of brd changed once the user synced", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    // a mutation of brd in a terminal (an agent's `brd reword`, a `brd lease get` beside): the session runs nothing
    await expectNothingRunsBeforeTheFirstSync(page, () => repo.touchStampAndAnswer())

    // once the user synced, a stamp syncs what a mutation of brd changed
    await page.getByTestId("ready-to-sync-card").getByTestId("ready-to-sync-button").click()
    await waitForSyncComplete(page, 0)
    await repo.commit("(stamp-docs) Add the docs", { "stamp-docs.md": "docs\n" }, 1_704_200_000)
    await runAndWaitForSync(page, () => repo.touchStamp())
    await expect(findBranchRow(page, "stamp-docs")).toBeVisible()
  })

  test("syncs on focus only when what a sync reads changed, and says when it checked", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("issue_links")
    await syncAndWaitForBranches()
    const syncs = await finishedSyncCount(page)
    const lastSync = page.getByTestId("last-sync-time")
    // on by default: the window tells its session, which reads the inputs, and syncs nothing when none changed; a
    // check in the minute of the sync says nothing "Last synced just now" does not, also once both aged
    expect((await focusWindow(page))?.outcome).toBe("unchanged")
    await page.clock.fastForward("01:15")
    await expect(lastSync).toHaveText("Last synced 1 minute ago")

    // a check a minute after the sync says that the branches are current, the age of the sync in its tooltip
    expect((await focusWindow(page))?.outcome).toBe("unchanged")
    await expect(lastSync).toHaveText("Checked just now")
    await page.mouse.move(0, 0)
    await lastSync.hover()
    await expect(openTooltip(page)).toHaveText("Nothing a sync reads changed since the last sync, 1 minute ago")
    expect(await finishedSyncCount(page)).toBe(syncs)

    // a commit reworded in a terminal: the next focus syncs it
    await rewordDocsInTerminal(page, repo)
    await syncOnWindowFocus(page)
    await expect(findBranchRow(page, "feature-documentation")).toBeVisible()
    await expect(lastSync).toHaveText("Last synced just now")
  })

  test("syncs nothing on the focus after its own push, the remote branch it moved known to the session", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the upstream of the template is a bare repository: the push is real and moves the remote branch a sync reads
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    await runAndWaitForAnswer(page, "execute", () => pushFromRowMenu(page, "feature-auth"))
    await expectPushedToast(page, "feature-auth")
    const syncs = await finishedSyncCount(page)

    // a minute after the sync, so the header says the check
    await page.clock.fastForward("01:15")
    expect((await focusWindow(page))?.outcome).toBe("unchanged")
    await expect(page.getByTestId("last-sync-time")).toHaveText("Checked just now")
    await expect(page.getByTestId("sync-button")).toHaveAttribute("data-sync-count", String(syncs))
  })

  test("syncs the session of a prefix set in a terminal once the focus finds it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the template sets branchdeck.branchPrefix user-name in .git/config
    const repo = await setupRepo("simple")
    await syncAndWaitForBranches()
    const synced = await viewEpoch(page)
    expect(synced).toBeDefined()
    await repo.setBranchPrefixInTerminal("team")
    const syncs = await recordCommand(page, "sync")
    // the check of the focus reads the inputs of the old session, the prefix among them: its run answers
    // `prefix_changed`, and the window subscribes again, holding the view it shows
    const resubscribed = page.waitForRequest(request => request.url().endsWith("/invoke/subscribe_view") && request.postDataJSON().have !== undefined)
    await emitTauriEvent(page, "tauri://focus")
    await resubscribed

    await expectTeamSessionSyncedOnOpen(page, repo, synced, syncs)
  })

  test("tells the session the focus, which syncs nothing once Auto-sync on Focus is off", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("issue_links")
    await syncAndWaitForBranches()
    // File → Auto-sync on Focus unchecked: the setting the backend reads, once its writer has it
    await emitTauriEvent(page, "auto_sync_on_focus_toggled", false)
    await waitForStorePersistence(page, "appSettings", settings => (settings as { autoSyncOnFocus?: boolean } | undefined)?.autoSyncOnFocus === false)
    await rewordDocsInTerminal(page, repo)
    // the window still tells its session; the session asks nothing, and the rows stay as the last sync left them
    expect(await focusWindow(page)).toBeNull()
    await expect(findBranchRow(page, "feature-docs")).toBeVisible()
    // the change was there to sync: Sync syncs it
    await syncBranches(page)
    await expect(findBranchRow(page, "feature-documentation")).toBeVisible()
  })

  test("checks Auto-sync on Focus in the native menu as the setting is, whichever window changed it", async ({ page, setupRepo }) => {
    // what the main window checks in the native menu, in the order it asked
    const checks: boolean[] = []
    page.on("request", request => {
      if (request.url().endsWith("/invoke/set_auto_sync_menu_checked")) {
        checks.push((request.postDataJSON() as { checked: boolean }).checked)
      }
    })
    await setupRepo("issue_links")
    // on by default
    await expect.poll(() => checks.at(-1)).toBe(true)
    // the user unchecks the menu item: the setting follows, as Settings shows it
    await emitTauriEvent(page, "auto_sync_on_focus_toggled", false)
    await expect.poll(() => checks.at(-1)).toBe(false)
    const settings = await openSettingsWindow(page, { page: "general" })
    const settingsSwitch = settings.getByTestId("setting-autoSyncOnFocus").getByRole("switch")
    await expect(settingsSwitch).not.toBeChecked()

    // the switch of the Settings window: its write reaches the main window through the settings sync, which checks the item
    await settingsSwitch.click()
    await expect.poll(() => checks.at(-1)).toBe(true)
    await settingsSwitch.click()
    await expect.poll(() => checks.at(-1)).toBe(false)
  })
})
