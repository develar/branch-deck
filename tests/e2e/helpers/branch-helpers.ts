import type { Page, Locator } from "@playwright/test"
import { expect } from "@playwright/test"

/**
 * Find a branch row by branch name
 */
export function findBranchRow(page: Page, branchName: string): Locator {
  return page.locator(`[data-testid="branch-row"][data-branch-name="${branchName}"]`)
}

/**
 * Get all cells from a branch row with proper typing
 */
export function getBranchCells(branchRow: Locator) {
  return {
    branchName: branchRow.locator("td").nth(0),
    commits: branchRow.locator("td").nth(1),
    status: branchRow.locator("td").nth(2),
    actions: branchRow.locator("td").nth(3),
  }
}

/**
 * Expand a branch row
 */
export async function expandBranch(page: Page, branchRow: Locator): Promise<void> {
  const currentState = await branchRow.getAttribute("data-state")
  if (currentState === "closed") {
    const expandButton = branchRow.locator("button").first()
    await expandButton.click()
    await expect(branchRow).toHaveAttribute("data-state", "open")
  }
}

/**
 * Collapse a branch row
 */
export async function collapseBranch(page: Page, branchRow: Locator): Promise<void> {
  const currentState = await branchRow.getAttribute("data-state")
  if (currentState === "open") {
    const collapseButton = branchRow.locator("button").first()
    await collapseButton.click()
    await expect(branchRow).toHaveAttribute("data-state", "closed")
  }
}

/**
 * Verify branch row state (expanded/collapsed)
 */
export async function verifyBranchState(branchRow: Locator, state: "open" | "closed"): Promise<void> {
  await expect(branchRow).toHaveAttribute("data-state", state)
}

/**
 * Get the expanded details row for a branch
 */
export function getBranchDetailsRow(page: Page, branchName: string): Locator {
  // Use + selector to get the immediate next sibling tr element
  return page.locator(`[data-branch-name="${branchName}"] + tr`)
}

/**
 * Get the copy button from a branch row's actions cell
 */
export function getCopyButton(branchRow: Locator): Locator {
  const { actions } = getBranchCells(branchRow)
  return actions.locator("button").first()
}

/**
 * Get the push button from a branch row's actions cell
 */
export function getPushButton(branchRow: Locator): Locator {
  const { actions } = getBranchCells(branchRow)
  return actions.locator("button").nth(1)
}

/**
 * Verify branch has expected number of commits
 */
export async function verifyCommitCount(branchRow: Locator, expectedText: string): Promise<void> {
  const { commits } = getBranchCells(branchRow)
  await expect(commits).toContainText(expectedText)
}

/**
 * Verify branch status
 */
export async function verifyBranchStatus(branchRow: Locator, expectedStatus: string): Promise<void> {
  const { status } = getBranchCells(branchRow)
  const statusBadge = status.locator(".lowercase")
  await expect(statusBadge).toContainText(expectedStatus)
}

/**
 * Check if branch is processing (has animate-pulse class)
 */
export async function isBranchProcessing(branchRow: Locator): Promise<boolean> {
  const classes = await branchRow.getAttribute("class")
  return classes?.includes("animate-pulse") ?? false
}

/**
 * Wait for branch to finish processing
 */
export async function waitForBranchProcessingComplete(branchRow: Locator): Promise<void> {
  if (await isBranchProcessing(branchRow)) {
    await expect(branchRow).not.toHaveClass(/animate-pulse/)
  }
}