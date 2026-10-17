import type { Page, Locator } from "@playwright/test"
import { expect } from "@playwright/test"

/**
 * Select a single commit
 */
export async function selectCommit(commitLocator: Locator): Promise<void> {
  // Ensure the commit row is visible before clicking
  await commitLocator.scrollIntoViewIfNeeded()
  await commitLocator.click()

  // the selection is there once the row says it: what follows waits for the element it acts on
  await expect(commitLocator).toHaveAttribute("data-selected", "true")
}

/**
 * Multi-select a commit (adds to existing selection)
 */
export async function multiSelectCommit(commitLocator: Locator): Promise<void> {
  // Ensure the commit row is visible before clicking
  await commitLocator.scrollIntoViewIfNeeded()
  await commitLocator.click({ modifiers: ["ControlOrMeta"] })

  await expect(commitLocator).toHaveAttribute("data-selected", "true")
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
 * Wait for and click the "Move to…" button of the selection
 */
export async function clickGroupIntoBranchButton(page: Page): Promise<void> {
  // the button is in the header of the unassigned commits card (or above the branch commits) while commits are selected
  const createBranchButton = page.getByTestId("selection-actions").getByRole("button", { name: "Move to…" })
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
  return page.locator('[data-row-id][data-selected="true"]')
}

/** Starts a native drag of a commit or branch row and keeps the button pressed; finish it with `dropOn` */
export async function startDrag(page: Page, row: Locator): Promise<void> {
  await row.scrollIntoViewIfNeeded()
  await row.hover()
  await page.mouse.down()
  const box = (await row.boundingBox())!
  // the drag starts on the first move
  await page.mouse.move(box.x + box.width / 2 + 10, box.y + box.height / 2 + 10, { steps: 4 })
}

export async function dropOn(page: Page, target: Locator): Promise<void> {
  await target.hover()
  await page.mouse.up()
}
