import type { Page } from "@playwright/test"
import { expect } from "@playwright/test"

/**
 * Sync branches and wait for specific elements to appear
 * @param waitForSelectors - Optional selectors to wait for after starting sync
 */
export async function syncBranches(page: Page, waitForSelectors?: string[]): Promise<void> {
  // Click sync button to load branches
  const syncButton = page.locator("[data-testid=\"sync-button\"]")

  // Wait for the button to be enabled
  await expect(syncButton).toBeEnabled()

  await syncButton.click()

  // If specific selectors are provided, wait for them
  if (waitForSelectors && waitForSelectors.length > 0) {
    console.log(`[Sync] Waiting for selectors: ${waitForSelectors.join(", ")}`)

    // Wait for all specified selectors in parallel
    await Promise.all(
      waitForSelectors.map(selector =>
        page.waitForSelector(selector, { timeout: 15000 })
          .then(() => console.log(`[Sync] Found selector: ${selector}`))
          .catch((err) => {
            console.error(`[Sync] Failed to find selector: ${selector}`, err.message)
            throw err
          }),
      ),
    )
  }
  else {
    // Default behavior: wait for sync to complete
    // Wait for sync to complete (button becomes enabled again)
    await expect(syncButton).toBeEnabled({ timeout: 15000 })

    // Wait for sync results - look for either branch rows or unassigned commits
    // This replaces the need for an arbitrary timeout as it waits for actual content
    await page.waitForFunction(() => {
      const branchRows = document.querySelectorAll("[data-testid=\"branch-row\"]").length
      const unassignedSection = Array.from(document.querySelectorAll("h2")).find(h => h.textContent?.includes("Unassigned Commits"))
      const emptyState = document.querySelector("[title=\"No branches found\"]")
      console.log(`[Sync waitForFunction] Branch rows: ${branchRows}, Unassigned section: ${unassignedSection ? "found" : "not found"}, Empty state: ${emptyState ? "found" : "not found"}`)
      return branchRows > 0 || unassignedSection !== null || emptyState !== null
    }, { timeout: 10000 })
  }
}

/**
 * Wait for branches to finish loading
 */
export async function waitForBranchesLoaded(page: Page, timeout = 10000): Promise<void> {
  // Wait for branch rows to appear
  await page.waitForSelector("[data-testid=\"branch-row\"]", { timeout })
}

/**
 * Wait for unassigned commits section to appear
 */
export async function waitForUnassignedCommits(page: Page, timeout = 10000): Promise<void> {
  // Wait for unassigned commits section
  await page.waitForSelector("[data-testid=\"unassigned-commits-section\"]", { timeout })

  // Wait for commit rows to load - they have data-row-id attribute
  await page.waitForSelector("[data-row-id]", { timeout })
}

/**
 * Wait for a specific branch to finish syncing (no progress bar)
 */
export async function waitForBranchSyncComplete(page: Page, branchName: string): Promise<void> {
  const branchRow = page.locator(`[data-testid="branch-row"][data-branch-name="${branchName}"]`)

  // Wait for the branch row to be visible
  await expect(branchRow).toBeVisible()

  // Wait for the progress bar to disappear (sync complete)
  const statusCell = branchRow.locator("td").nth(2) // Third cell is status
  await expect(statusCell.locator("[role=\"progressbar\"]")).not.toBeVisible()

  // Now wait for the status badge to appear and verify it's not "syncing"
  const statusBadge = statusCell.locator(".lowercase")
  await expect(statusBadge).toBeVisible()

  // Verify the status is not "syncing" - it should be "created", "updated", etc.
  const statusText = await statusBadge.textContent()
  expect(statusText?.toLowerCase()).not.toBe("syncing")
}

/**
 * Get unassigned commits within the unassigned card
 */
export function getUnassignedCommits(page: Page) {
  // Find the unassigned commits card - it contains "Unassigned Commits" text
  const unassignedCard = page.locator(".overflow-hidden").filter({ hasText: "Unassigned Commits" })

  // Find commits within the unassigned card
  return unassignedCard.locator("[data-row-id]")
}

/**
 * Wait for sync operation to complete by monitoring the sync button state
 */
export async function waitForSyncComplete(page: Page, timeout = 10000): Promise<void> {
  const syncButton = page.locator("[data-testid=\"sync-button\"]")

  // Wait for sync to start (button becomes disabled)
  await expect(syncButton).toBeDisabled({ timeout: 5000 })

  // Wait for sync to complete (button becomes enabled again)
  await expect(syncButton).toBeEnabled({ timeout })
}