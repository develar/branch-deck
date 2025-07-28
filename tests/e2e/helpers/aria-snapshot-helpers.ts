import type { Page, Locator } from "@playwright/test"
import { expect } from "@playwright/test"

/**
 * Capture ARIA snapshot with standardized naming
 */
export async function captureAriaSnapshot(
  element: Locator,
  snapshotName: string,
  options?: Parameters<typeof expect>[1],
): Promise<void> {
  await expect(element).toMatchAriaSnapshot({
    name: snapshotName.endsWith(".yml") ? snapshotName : `${snapshotName}.aria.yml`,
    ...options,
  })
}

/**
 * Capture table ARIA snapshot
 */
export async function captureTableSnapshot(
  page: Page,
  tableSelector: string | Locator,
  snapshotName: string,
): Promise<void> {
  const table = typeof tableSelector === "string"
    ? page.locator(tableSelector)
    : tableSelector

  await captureAriaSnapshot(table, snapshotName)
}

/**
 * Capture modal/dialog ARIA snapshot
 */
export async function captureModalSnapshot(
  page: Page,
  snapshotName: string,
): Promise<void> {
  const modal = page.getByRole("dialog")
  await expect(modal).toBeVisible()
  await captureAriaSnapshot(modal, snapshotName)
}

/**
 * Capture context menu ARIA snapshot
 */
export async function captureContextMenuSnapshot(
  page: Page,
  snapshotName: string,
): Promise<void> {
  const menu = page.getByRole("menu")
  await expect(menu).toBeVisible()
  await captureAriaSnapshot(menu, snapshotName)
}

/**
 * Capture floating element ARIA snapshot (tooltips, popovers)
 */
export async function captureFloatingElementSnapshot(
  element: Locator,
  snapshotName: string,
): Promise<void> {
  await expect(element).toBeVisible()
  await captureAriaSnapshot(element, snapshotName)
}

/**
 * Capture branch row table with specific branch
 */
export async function captureBranchTableSnapshot(
  page: Page,
  branchRow: Locator,
  snapshotName: string,
): Promise<void> {
  const table = page.locator("table").filter({ has: branchRow })
  await captureTableSnapshot(page, table, snapshotName)
}

/**
 * Helper to capture multiple states of the same element
 */
export async function captureElementStates(
  element: Locator,
  states: Array<{ action: () => Promise<void>, snapshotName: string }>,
): Promise<void> {
  for (const { action, snapshotName } of states) {
    await action()
    await captureAriaSnapshot(element, snapshotName)
  }
}