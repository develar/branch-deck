import type { Page, Locator } from "@playwright/test"
import { expect } from "@playwright/test"

/**
 * Capture ARIA snapshot with standardized naming
 */
export async function captureAriaSnapshot(
  element: Locator,
  snapshotName: string,
  options?: Record<string, unknown>,
): Promise<void> {
  await expect(element).toMatchAriaSnapshot({
    name: snapshotName.endsWith(".yml") ? snapshotName : `${snapshotName}.aria.yml`,
    ...options,
  })
}

/**
 * Capture HTML snapshot with normalization and pretty formatting
 */
export async function captureHtmlSnapshot(
  element: Locator,
  snapshotName: string,
): Promise<void> {
  // Use the global HTML formatter function
  const formattedHtml = await element.evaluate((el) => {
    // Check if the formatter is available
    const win = window as Window & { __htmlFormatter?: { normalizeAndFormat: (el: Element) => string } }
    if (typeof win.__htmlFormatter === "undefined") {
      throw new Error("HTML formatter not loaded. Make sure html-formatter.ts is imported.")
    }
    return win.__htmlFormatter.normalizeAndFormat(el)
  })

  const finalName = snapshotName.endsWith(".html") ? snapshotName : `${snapshotName}.html`
  await expect(formattedHtml).toMatchSnapshot(finalName)
}

/**
 * Capture table HTML snapshot
 */
export async function captureTableSnapshot(
  page: Page,
  tableSelector: string | Locator,
  snapshotName: string,
): Promise<void> {
  const table = typeof tableSelector === "string"
    ? page.locator(tableSelector)
    : tableSelector

  await captureHtmlSnapshot(table, snapshotName)
}

/**
 * Capture modal/dialog HTML snapshot
 */
export async function captureModalSnapshot(
  page: Page,
  snapshotName: string,
): Promise<void> {
  const modal = page.getByRole("dialog")
  await expect(modal).toBeVisible()
  await captureHtmlSnapshot(modal, snapshotName)
}

/**
 * Capture context menu HTML snapshot
 */
export async function captureContextMenuSnapshot(
  page: Page,
  snapshotName: string,
): Promise<void> {
  const menu = page.getByRole("menu")
  await expect(menu).toBeVisible()
  await captureHtmlSnapshot(menu, snapshotName)
}

/**
 * Capture floating element HTML snapshot (tooltips, popovers)
 */
export async function captureFloatingElementSnapshot(
  element: Locator,
  snapshotName: string,
): Promise<void> {
  await expect(element).toBeVisible()
  await captureHtmlSnapshot(element, snapshotName)
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
    await captureHtmlSnapshot(element, snapshotName)
  }
}
