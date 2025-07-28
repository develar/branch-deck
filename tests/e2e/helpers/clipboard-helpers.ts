import type { Page, BrowserContext, Locator } from "@playwright/test"
import { expect } from "@playwright/test"
import { captureFloatingElementSnapshot } from "./aria-snapshot-helpers"

/**
 * Setup clipboard permissions for the test context
 */
export async function setupClipboardPermissions(context: BrowserContext): Promise<void> {
  await context.grantPermissions(["clipboard-read", "clipboard-write"])
}

/**
 * Read the current clipboard content
 */
export async function readClipboard(page: Page): Promise<string> {
  return await page.evaluate(() => navigator.clipboard.readText())
}

/**
 * Perform an action and verify clipboard content
 */
export async function copyAndVerifyClipboard(
  page: Page,
  action: () => Promise<void>,
  expectedText: string | RegExp,
): Promise<void> {
  await action()

  const clipboardText = await readClipboard(page)

  if (typeof expectedText === "string") {
    expect(clipboardText).toBe(expectedText)
  }
  else {
    expect(clipboardText).toMatch(expectedText)
  }
}

/**
 * Test copy button behavior with tooltip feedback
 */
export async function testCopyButton(
  page: Page,
  button: Locator,
  expectedContent: string | RegExp,
  originalTooltipText = "Copy full branch name",
  options?: {
    captureSnapshots?: boolean
    snapshotPrefix?: string
  },
): Promise<void> {
  // Hover to see original tooltip
  await button.hover()

  // Wait for tooltip to appear
  const originalTooltip = page.locator(`[role="tooltip"]:has-text("${originalTooltipText}")`)
  await expect(originalTooltip).toBeVisible()

  // Capture initial state snapshot if requested
  if (options?.captureSnapshots) {
    const prefix = options.snapshotPrefix || "copy-button"
    // Capture just the tooltip
    await captureFloatingElementSnapshot(originalTooltip, `${prefix}-initial`)
  }

  // Click the copy button
  await button.click()

  // Wait for tooltip to change to "Copied!"
  const copiedTooltip = page.locator("[role=\"tooltip\"]:has-text(\"Copied!\")")
  await expect(copiedTooltip).toBeVisible()

  // Verify the original tooltip is no longer visible
  await expect(originalTooltip).not.toBeVisible()

  // Capture copied state snapshot if requested
  if (options?.captureSnapshots) {
    const prefix = options.snapshotPrefix || "copy-button"
    // Capture just the tooltip
    await captureFloatingElementSnapshot(copiedTooltip, `${prefix}-copied`)
  }

  // Verify clipboard content
  const clipboardText = await readClipboard(page)

  if (typeof expectedContent === "string") {
    expect(clipboardText).toBe(expectedContent)
  }
  else {
    expect(clipboardText).toMatch(expectedContent)
  }
}

/**
 * Test context menu copy action
 */
export async function testContextMenuCopy(
  page: Page,
  menuItemText: string,
  expectedContent: string | RegExp,
): Promise<void> {
  const menuItem = page.getByRole("menuitem", { name: menuItemText })
  await menuItem.click()

  const clipboardText = await readClipboard(page)

  if (typeof expectedContent === "string") {
    expect(clipboardText).toBe(expectedContent)
  }
  else {
    expect(clipboardText).toMatch(expectedContent)
  }
}

/**
 * Clear clipboard content
 */
export async function clearClipboard(page: Page): Promise<void> {
  await page.evaluate(() => navigator.clipboard.writeText(""))
}