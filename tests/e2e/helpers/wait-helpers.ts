import type { Page, Locator } from "@playwright/test"
import { expect } from "@playwright/test"

/**
 * Wait for processing state to complete (animate-pulse class removed)
 */
export async function waitForProcessingComplete(page: Page, element: Locator): Promise<void> {
  // Check if element is currently processing
  const elementClasses = await element.getAttribute("class")
  const isProcessing = elementClasses?.includes("animate-pulse")

  if (isProcessing) {
    // Wait for processing to complete - the animate-pulse class should be removed
    await expect(element).not.toHaveClass(/animate-pulse/)
  }
}

/**
 * Wait for a specific API response
 */
export async function waitForApiResponse(
  page: Page,
  urlPattern: string | RegExp,
  expectedStatus = 200,
): Promise<void> {
  await page.waitForResponse((response) => {
    const matches = typeof urlPattern === "string"
      ? response.url().includes(urlPattern)
      : urlPattern.test(response.url())
    return matches && response.status() === expectedStatus
  })
}

/**
 * Wait for element with specific text
 */
export async function waitForElementWithText(
  page: Page,
  text: string,
  options?: { timeout?: number },
): Promise<void> {
  await page.waitForSelector(`text=${text}`, options)
}

/**
 * Wait for branch details to expand
 */
export async function waitForBranchDetailsExpanded(
  page: Page,
  branchName: string,
): Promise<Locator> {
  // Wait for the branch details row to be visible
  const detailsRow = page.locator(`[data-branch-name="${branchName}"] ~ tr`)
  await detailsRow.waitFor({ state: "visible", timeout: 5000 })
  return detailsRow
}

/**
 * Wait with timeout and return a default value if timeout
 */
export async function waitWithTimeout<T>(
  promise: Promise<T>,
  timeoutMs: number,
  defaultValue: T,
): Promise<T> {
  return Promise.race([
    promise,
    new Promise<T>(resolve => setTimeout(() => resolve(defaultValue), timeoutMs)),
  ])
}

/**
 * Wait for tooltip to appear with specific text
 */
export async function waitForTooltip(
  page: Page,
  text: string,
  timeout = 5000,
): Promise<Locator> {
  const tooltip = page.locator(`[role="tooltip"]:has-text("${text}")`)
  await expect(tooltip).toBeVisible({ timeout })
  return tooltip
}

/**
 * Wait for tooltip text to change
 */
export async function waitForTooltipChange(
  page: Page,
  fromText: string,
  toText: string,
  timeout = 5000,
): Promise<void> {
  // Wait for original tooltip to disappear
  const originalTooltip = page.locator(`[role="tooltip"]:has-text("${fromText}")`)
  await expect(originalTooltip).not.toBeVisible({ timeout })

  // Wait for new tooltip to appear
  const newTooltip = page.locator(`[role="tooltip"]:has-text("${toText}")`)
  await expect(newTooltip).toBeVisible({ timeout })
}

/**
 * Wait for conflict viewer to load in branch details
 */
export async function waitForConflictViewer(
  page: Page,
  branchDetailsRow: Locator,
  timeout = 10000,
): Promise<Locator> {
  // Wait for conflict text to appear
  const conflictText = branchDetailsRow.locator("text=Cherry-pick conflict detected").first()
  await expect(conflictText).toBeVisible({ timeout })

  // Return the tab container for the conflict viewer
  const tabContainer = branchDetailsRow.locator("[role=\"tablist\"]").first()
  await expect(tabContainer).toBeVisible({ timeout })

  return tabContainer
}

/**
 * Wait for popover to appear
 */
export async function waitForPopover(
  page: Page,
  content: string | RegExp,
  timeout = 5000,
): Promise<Locator> {
  const popover = typeof content === "string"
    ? page.locator("div").filter({ hasText: content }).first()
    : page.locator("div", { hasText: content }).first()

  await expect(popover).toBeVisible({ timeout })
  return popover
}

/**
 * Wait for navigation to complete after an action
 */
export async function waitForNavigationComplete(page: Page): Promise<void> {
  await page.waitForLoadState("networkidle")
}

/**
 * Wait for store persistence to complete (debounced operations)
 */
export async function waitForStorePersistence(page: Page, debounceMs = 600): Promise<void> {
  // Wait for debounce time plus a buffer
  await page.waitForTimeout(debounceMs)
  await page.waitForLoadState("networkidle")
}

/**
 * Wait for floating toolbar to appear
 */
export async function waitForFloatingToolbar(
  page: Page,
  timeout = 10000,
): Promise<Locator> {
  // The floating toolbar has a 150ms delay before showing
  await page.waitForTimeout(300)

  const toolbar = page.locator("[data-testid=\"floating-selection-bar\"]")
  await expect(toolbar).toBeVisible({ timeout })
  return toolbar
}