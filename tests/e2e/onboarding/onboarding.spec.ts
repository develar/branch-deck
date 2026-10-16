// @spec spec/repository/onboarding.spec.md
import { test, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { expandBranch, findBranchRow, getBranchDetailsRow } from "../helpers/branch-helpers"
import { emitTauriEvent, finishedSyncCount, syncBranches, syncOnWindowFocus } from "../helpers/sync-helpers"
import type { Page } from "@playwright/test"

/** The branch prefix as Settings… (⌘,) shows it; the header has no field of its own */
async function expectSettingsPrefix(page: Page, prefix: string): Promise<void> {
  await expect(page.getByTestId("configuration-header").getByRole("textbox", { name: "Branch prefix" })).toHaveCount(0)
  await emitTauriEvent(page, "open_settings")
  const dialog = page.getByTestId("settings-dialog")
  await expect(dialog.getByRole("textbox", { name: "Branch prefix", exact: true })).toHaveValue(prefix)
  await page.keyboard.press("Escape")
  await expect(dialog).toBeHidden()
}

/** Marks the injected project as synced an hour ago; `otherPath` adds a second recent project, never synced */
async function markSyncedBefore(page: Page, otherPath?: string): Promise<void> {
  await page.addInitScript((otherPath) => {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const projects = (window as any).__TAURI_STORE__.recentProjects
    projects[0].lastSyncTime = Date.now() - 60 * 60 * 1000
    if (otherPath !== undefined) {
      projects.push({ path: otherPath, cachedBranchPrefix: "user-name" })
    }
  }, otherPath)
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

  test("changes the prefix in Settings on Enter or when the field is left, not on every key", async ({ page, setupRepo, testStore }) => {
    await setupRepo("simple", { prepopulateStore: true })
    await emitTauriEvent(page, "open_settings")
    const field = page.getByTestId("settings-dialog").getByRole("textbox", { name: "Branch prefix", exact: true })
    await field.fill("jane doe")
    await expect(field).toHaveAttribute("aria-invalid", "true")
    await field.press("Enter")
    // refused: nothing saved
    expect((await testStore.get("appSettings") as { globalUserBranchPrefix?: string } | null)?.globalUserBranchPrefix).not.toBe("jane doe")
    await field.fill("jane/")
    await expect(page.getByTestId("setting-branch-prefix")).toContainText("Branches are named jane/virtual/<name>")
    await field.press("Enter")
    await expect.poll(async () => (await testStore.get("appSettings") as { globalUserBranchPrefix?: string } | null)?.globalUserBranchPrefix).toBe("jane")
    // Escape takes the typing back, the dialog stays
    await field.fill("john")
    await field.press("Escape")
    await expect(field).toHaveValue("jane")
    await expect(page.getByTestId("settings-dialog")).toBeVisible()
  })

  test("disables Sync with the reason while no branch prefix is set", async ({ page, setupRepo }) => {
    await setupRepo("simple_no_prefix", { prepopulateStore: true })

    const syncButton = page.getByTestId("sync-button")
    await expect(syncButton).toBeDisabled()
    await syncButton.hover()
    await expect(page.getByTestId("sync-tooltip")).toHaveText("Set the branch prefix first")

    // the menu item (⌘R) does what the button does: nothing
    const syncRequests: string[] = []
    page.on("request", (request) => {
      if (request.url().endsWith("/invoke/sync_branches")) {
        syncRequests.push(request.url())
      }
    })
    await emitTauriEvent(page, "sync-branches")
    await expect(syncButton).toHaveAttribute("data-sync-count", "0")
    expect(syncRequests).toEqual([])
  })

  test("says what Sync does before the first sync, then gives way to the branches", async ({ page, setupRepo }) => {
    await setupRepo("simple", { prepopulateStore: true })

    const card = page.getByTestId("ready-to-sync-card")
    await expect(card).toBeVisible()
    await expect(card).toContainText("user-name/virtual/<name>")
    await expect(card).toContainText("Your working tree and the checked-out branch stay as they are.")
    await captureHtmlSnapshot(card, "ready-to-sync-first-time")

    await card.getByTestId("ready-to-sync-button").click()

    await expect(page.locator("[data-testid=branch-row]").first()).toBeVisible()
    await expect(card).toHaveCount(0)
  })

  test("syncs a repository synced before when the window opens it", async ({ page, setupRepo }) => {
    await setupRepo("behind_upstream", { prepopulateStore: true })
    // a new window of a project synced an hour ago, Auto-sync on Focus off (the injected store runs first, this
    // script patches it)
    await markSyncedBefore(page)
    await page.reload()

    // no click: the branches and Update show by themselves
    await expect(findBranchRow(page, "feature-auth")).toBeVisible()
    await expect(page.getByTestId("update-project-button")).toHaveAttribute("data-status", "clean")
    await expect(page.getByTestId("ready-to-sync-card")).toHaveCount(0)
    await expect(page.getByTestId("sync-button")).toHaveAttribute("data-sync-count", "1")
  })

  test("forgets the branches of the previous repository on a switch", async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    await markSyncedBefore(page, "/nonexistent/branch-deck-e2e-other")
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

  test("explains the grouping in the unassigned commits while there is no branch", async ({ page, setupRepo }) => {
    await setupRepo("unassigned", { prepopulateStore: true })
    await syncBranches(page)

    await expect(page.getByTestId("unassigned-commits-hint")).toHaveText(
      "Start a commit subject with (name) or an issue key like ABC-123 to group it into a branch, or select commits and move them to one.",
    )
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
    await setupRepo("empty_git", { prepopulateStore: true })
    await page.getByTestId("ready-to-sync-button").click()

    await expect(page.getByTestId("error-alert")).toContainText("The repository has no commits yet")
  })
})

test.describe("Deferred sync", () => {
  test("says in an info toast that a sync waits for git, and leaves the branches as they were", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("simple")
    await syncAndWaitForBranches()
    const rows = await page.getByTestId("branch-row").count()
    // the backend refuses the sync while git is in the middle of a rebase (`SyncDeferred`)
    await page.route("**/invoke/sync_branches", route => route.fulfill({ json: { status: "error", error: { message: "Not synced: a rebase is in progress", kind: "operationInProgress" } } }))
    await page.getByTestId("sync-button").click()
    const toast = page.locator("[data-slot=toast]").filter({ hasText: "Not synced: a rebase is in progress" })
    await expect(toast).toContainText("The branches sync once git finished it")
    // a precondition, not a failure: no error on Sync, the rows stay
    await expect(page.getByTestId("sync-button")).toBeEnabled()
    await expect(page.getByTestId("sync-tooltip")).toHaveCount(0)
    await expect(page.getByTestId("branch-row")).toHaveCount(rows)
  })
})

test.describe("Sync on focus", () => {
  test("syncs on focus only when what a sync reads changed, and says when it checked", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("issue_links")
    await syncAndWaitForBranches()
    const syncs = await finishedSyncCount(page)
    const fingerprints = page.waitForResponse(response => response.url().endsWith("/invoke/get_sync_inputs_fingerprint"))
    // on by default: a focus with nothing changed reads the inputs, and syncs nothing
    await emitTauriEvent(page, "tauri://focus")
    await fingerprints
    await expect(page.getByTestId("inputs-checked")).toHaveText("Checked just now")
    expect(await finishedSyncCount(page)).toBe(syncs)

    // a commit reworded in a terminal: the next focus syncs it
    await expandBranch(page, findBranchRow(page, "feature-docs"))
    const hash = await getBranchDetailsRow(page, "feature-docs").locator("tr[data-row-id]").getAttribute("data-row-id")
    await repo.reword(hash!, "(feature-documentation) Update documentation")
    await syncOnWindowFocus(page)
    await expect(findBranchRow(page, "feature-documentation")).toBeVisible()
    await expect(page.getByTestId("inputs-checked")).toHaveCount(0)
  })
})
