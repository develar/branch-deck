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
  await expect(commitLocator).toHaveAttribute("data-selected", "true")

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
  await expect(commitLocator).toHaveAttribute("data-selected", "true")

  // Additional small wait to ensure all watchers have executed
  await page.waitForTimeout(100)
}

/**
 * Shift-click a commit to select the range from the last clicked row to it
 */
export async function shiftSelectCommit(commitLocator: Locator): Promise<void> {
  await commitLocator.scrollIntoViewIfNeeded()
  await commitLocator.click({ modifiers: ["Shift"] })
  await expect(commitLocator).toHaveAttribute("data-selected", "true")
}

/**
 * Select all rows of a selectable commit table with Cmd/Ctrl+A
 */
export async function selectAllCommits(page: Page, table: Locator): Promise<void> {
  await expect(table).toHaveAttribute("tabindex", "0")
  await table.focus()
  await page.keyboard.press("ControlOrMeta+a")
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
  // the button is in the header of the unassigned commits card while commits are selected
  const createBranchButton = page.getByTestId("selection-actions").getByRole("button", { name: "Group into Branch" })
  await expect(createBranchButton).toBeVisible()
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
