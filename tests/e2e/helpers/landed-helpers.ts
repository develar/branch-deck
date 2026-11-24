import type { Locator, Page } from "@playwright/test"
import { expect } from "../fixtures/test-fixtures"
import { runAndWaitForUpdate } from "./sync-helpers"
import type { TestRepositoryBuilder } from "./test-repository"

/**
 * The repository of the template `landed_upstream` synced, then updated by Update Project, which brings what landed of
 * it in master: the Landed island of the main window, shown
 */
export async function updateIntoLandedIsland(
  page: Page,
  setupRepo: (template: string) => Promise<TestRepositoryBuilder>,
  syncAndWaitForBranches: () => Promise<unknown>,
): Promise<{ repo: TestRepositoryBuilder; card: Locator }> {
  const repo = await setupRepo("landed_upstream")
  await syncAndWaitForBranches()
  await runAndWaitForUpdate(page, () => page.getByTestId("update-project-button").click())
  const card = page.getByTestId("landed-card")
  await expect(card).toBeVisible()
  return { repo, card }
}
