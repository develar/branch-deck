import type { Page, Locator } from "@playwright/test"
import { expect } from "@playwright/test"

/**
 * Open a modal by clicking a trigger element
 */
export async function openModal(
  page: Page,
  triggerSelector: string | Locator,
): Promise<Locator> {
  const trigger = typeof triggerSelector === "string"
    ? page.locator(triggerSelector)
    : triggerSelector

  await trigger.click()

  const modal = page.getByRole("dialog")
  await expect(modal).toBeVisible()

  return modal
}

/**
 * Close modal using Escape key
 */
export async function closeModalWithEscape(page: Page): Promise<void> {
  await page.keyboard.press("Escape")
  const modal = page.getByRole("dialog")
  await expect(modal).not.toBeVisible()
}

/**
 * Close modal using close button
 */
export async function closeModalWithButton(page: Page): Promise<void> {
  // Try different selectors for close button
  const closeButton = page.getByRole("button", { name: "Close" })
    .or(page.locator("[aria-label=\"Close\"]"))
    .or(page.locator("button").filter({ hasText: "×" }))

  await closeButton.click()
  const modal = page.getByRole("dialog")
  await expect(modal).not.toBeVisible()
}

/**
 * Verify modal is visible and contains expected content
 */
export async function verifyModalContent(
  page: Page,
  expectedContent: string | string[],
): Promise<void> {
  const modal = page.getByRole("dialog")
  await expect(modal).toBeVisible()

  const contents = Array.isArray(expectedContent) ? expectedContent : [expectedContent]

  for (const content of contents) {
    await expect(modal).toContainText(content)
  }
}

/**
 * Get modal header/title
 */
export function getModalTitle(page: Page): Locator {
  const modal = page.getByRole("dialog")
  return modal.locator("h2, h3").first()
}

/**
 * Get modal body content
 */
export function getModalBody(page: Page): Locator {
  const modal = page.getByRole("dialog")
  return modal.locator("[role=\"document\"]").or(modal)
}

/**
 * Click a button inside the modal
 */
export async function clickModalButton(
  page: Page,
  buttonText: string,
): Promise<void> {
  const modal = page.getByRole("dialog")
  const button = modal.getByRole("button", { name: buttonText })
  await button.click()
}

/**
 * Wait for modal to close after an action
 */
export async function waitForModalClose(page: Page): Promise<void> {
  const modal = page.getByRole("dialog")
  await expect(modal).not.toBeVisible()
}

/**
 * Test modal keyboard navigation
 */
export async function testModalKeyboardNavigation(page: Page): Promise<void> {
  const modal = page.getByRole("dialog")
  await expect(modal).toBeVisible()

  // Tab through focusable elements
  await page.keyboard.press("Tab")
  await page.waitForTimeout(100)

  // Escape should close modal
  await page.keyboard.press("Escape")
  await expect(modal).not.toBeVisible()
}
