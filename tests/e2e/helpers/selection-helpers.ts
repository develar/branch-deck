import type { Page, Locator } from "@playwright/test"
import { expect } from "@playwright/test"

/**
 * Select a single commit
 */
export async function selectCommit(page: Page, commitLocator: Locator): Promise<void> {
  // Ensure the commit row is visible before clicking
  await commitLocator.scrollIntoViewIfNeeded()
  await commitLocator.click()

  // Wait for the selection to register in the UI
  await expect(commitLocator).toHaveAttribute("data-selected", "true", { timeout: 5000 })

  // Additional small wait to ensure all watchers have executed
  await page.waitForTimeout(100)
}

/**
 * Multi-select a commit (adds to existing selection)
 */
export async function multiSelectCommit(page: Page, commitLocator: Locator): Promise<void> {
  // Ensure the commit row is visible before clicking
  await commitLocator.scrollIntoViewIfNeeded()
  await commitLocator.click({ modifiers: ["ControlOrMeta"] })

  // Wait for the selection to register in the UI
  await expect(commitLocator).toHaveAttribute("data-selected", "true", { timeout: 5000 })

  // Additional small wait to ensure all watchers have executed
  await page.waitForTimeout(100)
}

/**
 * Open context menu on an element
 */
export async function openContextMenu(page: Page, element: Locator): Promise<void> {
  await element.click({ button: "right" })
}

/**
 * Click a context menu item by name
 */
export async function clickContextMenuItem(page: Page, itemName: string): Promise<void> {
  const menuItem = page.getByRole("menuitem", { name: itemName })
  await menuItem.waitFor({ state: "visible" })
  await menuItem.click()
}

/**
 * Wait for and click the "Group into Branch" button
 */
export async function clickGroupIntoBranchButton(page: Page): Promise<void> {
  // The floating toolbar has a 150ms delay before showing, plus we need time for
  // the selection state to update and the target element to be computed
  await page.waitForTimeout(300)

  // Wait for the floating selection bar to appear
  // It's rendered in a PopoverContent which might be in a portal
  const createBranchButton = page.locator("button:has-text(\"Group into Branch\")")

  // Debug: Check if any commits are selected (wrap in try-catch to handle navigation)
  try {
    const selectedCommits = await page.locator("[data-selected=\"true\"]").count()
    if (selectedCommits === 0) {
      console.error("No commits are selected - floating toolbar won't appear")
    }
  }
  catch {
    // Ignore errors from navigation
  }

  await expect(createBranchButton).toBeVisible({ timeout: 10000 })
  await createBranchButton.click()
}

/**
 * Check if an element has the selected attribute
 */
export async function isSelected(element: Locator): Promise<boolean> {
  const selected = await element.getAttribute("data-selected")
  return selected === "true"
}

/**
 * Get all selected commits
 */
export function getSelectedCommits(page: Page): Locator {
  return page.locator("[data-row-id][data-selected=\"true\"]")
}