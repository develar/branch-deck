import type { Locator, Page } from "@playwright/test"
import { expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "./branch-helpers"

/** Hovers a status chip (pull request, Patronus robot) and returns its details popover */
export async function hoverStatusChip(page: Page, chip: Locator, popoverTestId: string): Promise<Locator> {
  await chip.hover()
  const popover = page.getByTestId(popoverTestId)
  await expect(popover).toBeVisible()
  return popover
}

/** Hovers the outgoing indicator and returns the overview */
export async function openOutgoingOverview(page: Page): Promise<Locator> {
  await page.getByTestId("outgoing-indicator").hover()
  const popover = page.getByTestId("outgoing-popover")
  await expect(popover).toBeVisible()
  return popover
}

/** Opens the preview of a branch with Space; its keys work once its body has the focus */
export async function openBranchPreview(page: Page, branchName: string): Promise<Locator> {
  await findBranchRow(page, branchName).hover()
  await page.keyboard.press("Space")
  const preview = page.getByTestId("branch-preview")
  await expect(preview.getByTestId("branch-preview-body")).toBeFocused()
  return preview
}

/** Closes the preview with Space and opens the one of another branch */
export async function switchBranchPreview(page: Page, preview: Locator, branchName: string): Promise<void> {
  await page.keyboard.press("Space")
  await expect(preview).toHaveCount(0)
  await findBranchRow(page, branchName).hover()
  await page.keyboard.press("Space")
  await expect(preview.getByRole("heading")).toHaveText(branchName)
}
