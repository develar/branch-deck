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
 * Gets the AI suggestion buttons
 */
export function getSuggestionButtons(page: Page) {
  return inlineBranchCreator.getPortal(page)
    .locator("[data-testid='branch-name-suggestions'] button")
    .filter({ hasText: /^user-name/ })
}

/**
 * Clicks the "Enable AI" button in the initial prompt
 */
export async function clickEnableAI(page: Page) {
  const enableButton = inlineBranchCreator.getPortal(page).locator("button:has-text('Enable AI')")
  await enableButton.click()
}

/**
 * Clicks the "Not now" button in the initial prompt
 */
export async function clickNotNow(page: Page) {
  const notNowButton = inlineBranchCreator.getPortal(page).locator("button:has-text('Not now')")
  await notNowButton.click()
}

/**
 * Gets the downloading icon element
 */
export function getDownloadingIcon(page: Page) {
  return inlineBranchCreator.getPortal(page).locator("[data-testid='ai-status-icon-downloading']")
}

/**
 * Waits for the download icon to appear and be visible
 */
export async function waitForDownloadIcon(page: Page) {
  const downloadingIcon = getDownloadingIcon(page)
  await expect(downloadingIcon).toBeVisible()
  return downloadingIcon
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
 * Pauses the download via the toast pause button
 */
export async function pauseDownload(page: Page) {
  const toastPauseButton = page.locator("button:has-text('Pause')").first()
  await expect(toastPauseButton).toBeVisible()
  await toastPauseButton.click()
}

/**
 * Waits for the download paused message to appear
 */
export async function waitForDownloadPaused(page: Page) {
  const pausedMessage = page.getByText("Download Paused", { exact: true })
  await expect(pausedMessage).toBeVisible()
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