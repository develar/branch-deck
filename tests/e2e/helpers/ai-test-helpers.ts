import type { Page } from "@playwright/test"
import { expect } from "@playwright/test"
import { inlineBranchCreator } from "./inline-form-helpers"

/**
 * Gets the AI status icon element (sparkles icon)
 */
export function getAIStatusIcon(page: Page) {
  return inlineBranchCreator.getPortal(page).locator("[data-testid='ai-status-icon']")
}

/**
 * Gets the AI suggestion chips (the test server names them `fix-…` and `update-…` after the commit message words)
 */
export function getSuggestionButtons(page: Page) {
  return inlineBranchCreator.getPortal(page)
    .locator("[data-testid='branch-name-suggestions'] button[aria-pressed]")
}

/**
 * Gets the disabled "Generating…" placeholders of a request in flight
 */
export function getGeneratingPlaceholders(page: Page) {
  return inlineBranchCreator.getPortal(page)
    .locator("[data-testid='branch-name-suggestions'] button")
    .filter({ hasText: "Generating…" })
}

/**
 * Gets what the user has to do for suggestions (install or log in to Claude Code)
 */
export function getSuggestionHint(page: Page) {
  return inlineBranchCreator.getPortal(page).getByTestId("ai-suggestion-hint")
}

/**
 * Clicks the "Enable AI" button in the initial prompt
 */
export async function clickEnableAI(page: Page) {
  await getEnableButton(page).click()
}

/**
 * Clicks the "Not now" button in the initial prompt
 */
export async function clickNotNow(page: Page) {
  await getNotNowButton(page).click()
}

/**
 * Verifies that AI is enabled (icon has primary color)
 */
export async function expectAIEnabled(page: Page) {
  const aiStatusIcon = getAIStatusIcon(page)
  await expect(aiStatusIcon).toBeVisible()
  await expect(aiStatusIcon).toHaveClass(/text-primary/)
}

/**
 * Verifies that AI is disabled (icon has muted color)
 */
export async function expectAIDisabled(page: Page) {
  const aiStatusIcon = getAIStatusIcon(page)
  await expect(aiStatusIcon).toBeVisible()
  await expect(aiStatusIcon).toHaveClass(/text-muted/)
}

/**
 * Gets the Enable AI button element
 */
export function getEnableButton(page: Page) {
  return inlineBranchCreator.getPortal(page).locator("button:has-text('Enable AI')")
}

/**
 * Gets the Not now button element
 */
export function getNotNowButton(page: Page) {
  return inlineBranchCreator.getPortal(page).locator("button:has-text('Not now')")
}
