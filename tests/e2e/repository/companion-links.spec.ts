// @spec openspec/specs/branch/declared-needs/spec.md
import { realpath, rm } from "node:fs/promises"
import { join } from "node:path"
import { expect, test } from "../fixtures/test-fixtures"
import { header } from "../helpers/header-helpers"
import { switchRepository } from "../helpers/repository-switch-helpers"
import { openSettingsWindow } from "../helpers/settings-helpers"
import { finishedSyncCount, waitForSyncComplete } from "../helpers/sync-helpers"

test("should show and switch to a companion that has never been opened @webkit", async ({ page, setupRepo, syncAndWaitForBranches }) => {
  const primary = await setupRepo("issue_links")
  await syncAndWaitForBranches()
  const primaryRows = page.getByTestId("branch-row")
  await expect(primaryRows.first()).toBeVisible()

  const companionPath = await primary.addCompanionRepository("ci-config")
  await primary.setDialogPick(companionPath)
  const settings = await openSettingsWindow(page, { page: "landing", scope: "repository" })
  const companions = settings.getByRole("region", { name: "Companions" })
  const syncs = await finishedSyncCount(page)
  await companions.getByRole("button", { name: "Add Companion…" }).click()
  await expect(companions.getByTestId("companion-row")).toHaveCount(1)
  await waitForSyncComplete(page, syncs)
  await settings.close()

  const selector = header(page).getByRole("combobox")
  await selector.click()
  const list = page.getByRole("listbox")
  const primaryName = primary.path.split(/[\\/]/u).filter(Boolean).at(-1)!
  await expect(list.getByText(`Companions of ${primaryName}`)).toBeVisible()
  const option = list.getByRole("option", { name: "ci-config" })
  await expect(option).toBeVisible()
  const canonicalCompanionPath = await realpath(companionPath)
  await expect(option).toContainText(canonicalCompanionPath)

  await page.keyboard.press("Escape")
  await switchRepository(page, companionPath)
  await expect(header(page).getByTestId("repository-path")).toContainText("ci-config")
  await expect(page.getByTestId("welcome-card")).toBeVisible()
  await expect(primaryRows).toHaveCount(0)

  await rm(join(canonicalCompanionPath, ".git"), { recursive: true, force: true })
  await switchRepository(page, primary.path)
  const syncsBeforeUnreadableCheck = await finishedSyncCount(page)
  await page.getByTestId("sync-button").click()
  await waitForSyncComplete(page, syncsBeforeUnreadableCheck)
  await header(page).getByRole("combobox").click()
  const unreadable = page.getByRole("listbox").getByRole("option", { name: "ci-config" })
  await expect(unreadable).toBeDisabled()
  await expect(unreadable).toContainText("Not readable:")
  await expect(header(page).getByTestId("repository-path")).toContainText(primaryName)
})
