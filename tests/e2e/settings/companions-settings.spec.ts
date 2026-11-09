// @spec openspec/specs/branch/declared-needs/spec.md
import { test, expect } from "../fixtures/test-fixtures"
import { openSettingsWindow } from "../helpers/settings-helpers"
import { finishedSyncCount, waitForSyncComplete } from "../helpers/sync-helpers"

test.describe("Settings → Landing, Companions", () => {
  test("should add a companion repository from the folder dialog, refuse a folder that is no repository, and remove it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("issue_links")
    await syncAndWaitForBranches()
    const window = await openSettingsWindow(page, { page: "landing", scope: "repository" })
    const companions = window.getByRole("region", { name: "Companions" })
    const empty = companions.getByTestId("companions-empty")
    await expect(empty).toHaveText("No companion repositories. A need can name a branch of another repository once you add one here.")
    const add = companions.getByRole("button", { name: "Add Companion…" })

    // the user cancels the dialog: nothing is written
    await repo.setDialogPick(null)
    await add.click()
    await expect(add).toBeEnabled()
    await expect(empty).toBeVisible()

    // a folder that is no repository is refused with the reason
    const ciConfig = await repo.addCompanionRepository("ci-config")
    const folder = ciConfig.slice(0, ciConfig.lastIndexOf("/"))
    await repo.setDialogPick(folder)
    await add.click()
    const refused = window.locator("[data-slot=toast]").filter({ hasText: "Cannot add the companion" })
    await expect(refused).toContainText("is no git repository")
    await expect(empty).toBeVisible()

    // the repository is added under the name of its directory, and the main window syncs to read its branches
    await repo.setDialogPick(ciConfig)
    const syncs = await finishedSyncCount(page)
    await add.click()
    const row = companions.getByTestId("companion-row")
    await expect(row).toHaveCount(1)
    await expect(row).toContainText("ci-config")
    // its main worktree, as git names it
    await expect(row).toContainText(".companions/ci-config")
    await waitForSyncComplete(page, syncs)

    await companions.getByRole("button", { name: "Remove ci-config" }).click()
    await expect(empty).toBeVisible()
    await expect(row).toHaveCount(0)
  })
})
