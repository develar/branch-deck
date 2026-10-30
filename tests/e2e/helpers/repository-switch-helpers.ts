import type { Page } from "@playwright/test"
import { expect } from "../fixtures/base-test"

/**
 * A second recent project, which git cannot read: a switch to it shows another repository, so what an action of the
 * first one does after the switch must reach the first one, not the selected one
 */
export const OTHER_REPOSITORY = "/nonexistent/branch-deck-e2e-other"

function nameOf(path: string): string {
  return path.split("/").at(-1)!
}

/**
 * Adds `path` to the recent projects after the selected one and reloads the page, which shows the selected one as the
 * setup left it (a repository never synced shows the Ready to Sync card)
 */
export async function addRecentProject(page: Page, path: string = OTHER_REPOSITORY): Promise<void> {
  await page.addInitScript(other => {
    const store = (window as unknown as { __TAURI_STORE__: { recentProjects: { path: string }[] } }).__TAURI_STORE__
    if (!store.recentProjects.some(project => project.path === other)) {
      store.recentProjects.push({ path: other })
    }
  }, path)
  await page.reload()
  await expect(page.getByTestId("configuration-header")).toBeVisible()
}

/** Selects the recent project of `path` in the repository selector of the title bar, as the user does */
export async function switchRepository(page: Page, path: string): Promise<void> {
  await page.getByRole("combobox").click()
  await page.getByRole("option", { name: nameOf(path) }).click()
  await expect(page.getByRole("combobox")).toContainText(nameOf(path))
}
