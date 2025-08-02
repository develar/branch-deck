import type { Page, Locator } from "@playwright/test"
import { expect } from "@playwright/test"
import { selectCommit, clickGroupIntoBranchButton } from "./selection-helpers"

/**
 * Test escape key behavior for inline forms
 */
export async function testEscapeKeyBehavior(
  page: Page,
  formLocator: Locator,
  inputLocator: Locator,
  testValue = "test-value",
): Promise<void> {
  // Type some text
  await inputLocator.fill(testValue)

  // Press Escape to cancel
  await inputLocator.press("Escape")

  // Form should disappear
  await expect(formLocator).not.toBeVisible()
}

/**
 * Test escape key behavior when a button is focused
 */
export async function testEscapeKeyWithButtonFocus(
  page: Page,
  formLocator: Locator,
  inputLocator: Locator,
  cancelButtonLocator: Locator,
  testValue = "test-value",
): Promise<void> {
  // Type some text
  await inputLocator.fill(testValue)

  // Focus on the Cancel button
  await cancelButtonLocator.focus()
  await expect(cancelButtonLocator).toBeFocused()

  // Press Escape while button is focused
  await page.keyboard.press("Escape")

  // Form should disappear
  await expect(formLocator).not.toBeVisible()
}

/**
 * Test that input is focused when form opens
 */
export async function testInputAutoFocus(inputLocator: Locator): Promise<void> {
  // Use Playwright's built-in retry assertion - polls automatically for up to 5 seconds
  await expect(inputLocator).toBeFocused()
}

/**
 * Submit inline form with value
 */
export async function submitInlineForm(
  inputLocator: Locator,
  value: string,
): Promise<void> {
  await inputLocator.fill(value)
  await inputLocator.press("Enter")
}

/**
 * Validate form shows error for invalid input
 */
export async function validateFormError(
  page: Page,
  formLocator: Locator,
  inputLocator: Locator,
  invalidValue: string,
  expectedError: string,
): Promise<void> {
  // Try invalid value
  await inputLocator.fill(invalidValue)
  await inputLocator.press("Enter")

  // Form should still be visible
  await expect(formLocator).toBeVisible()

  // Should show error message
  const errorMessage = formLocator.locator("text=" + expectedError)
  await expect(errorMessage).toBeVisible()
}

/**
 * Validate branch name format error
 */
export async function validateBranchNameError(
  page: Page,
  inputLocator: Locator,
  invalidValue: string,
): Promise<void> {
  await inputLocator.fill(invalidValue)
  await inputLocator.press("Enter")

  // Should show error message
  const errorMessage = inlineBranchCreator.getPortal(page).locator(".text-xs").filter({ hasText: "Use letters, numbers, -, _, ." })
  await expect(errorMessage).toBeVisible()

  // Input should still be visible
  await expect(inlineBranchCreator.getPortal(page)).toBeVisible()
}

/**
 * Opens the inline branch creator for the first commit in the list
 */
export async function openBranchCreatorForFirstCommit(page: Page): Promise<void> {
  const firstCommit = page.locator("[data-row-id]").first()
  await selectCommit(page, firstCommit)
  await clickGroupIntoBranchButton(page)
  await inlineBranchCreator.waitForVisible(page)
}

/**
 * Common inline branch creator helpers
 */
export const inlineBranchCreator = {
  getPortal: (page: Page) => page.locator("#inline-branch-creator-portal"),
  getInput: (page: Page) => page.locator("#inline-branch-creator-portal input[type=\"text\"]"),
  getCancelButton: (page: Page) => page.locator("#inline-branch-creator-portal button:has-text('Cancel')"),
  getAiToggle: (page: Page) => page.locator("#inline-branch-creator-portal button[aria-label=\"AI\"]"),

  async waitForVisible(page: Page): Promise<void> {
    await expect(this.getPortal(page)).toBeVisible()
  },

  async waitForHidden(page: Page): Promise<void> {
    await expect(this.getPortal(page)).not.toBeVisible()
  },
}

/**
 * Common inline issue reference helpers
 */
export const inlineIssueReference = {
  getForm: (page: Page) => page.locator("[data-testid=\"inline-issue-input\"]"),
  getInput: (page: Page) => page.locator("[data-testid=\"inline-issue-input\"] input[type=\"text\"]"),
  getCancelButton: (page: Page) => page.locator("[data-testid=\"inline-issue-input\"] button:has-text('Cancel')"),
  getErrorMessage: (page: Page) => page.locator("[data-testid=\"inline-issue-input\"] .text-error"),

  async waitForVisible(page: Page): Promise<void> {
    await expect(this.getForm(page)).toBeVisible()
  },

  async waitForHidden(page: Page): Promise<void> {
    await expect(this.getForm(page)).not.toBeVisible()
  },
}