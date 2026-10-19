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
 * Get the expanded details row for a branch: the next row, after the progress line of a running robot
 */
export function getBranchDetailsRow(page: Page, branchName: string): Locator {
  const row = `[data-branch-name="${branchName}"]`
  return page.locator(`${row} + tr:not([data-testid=robot-progress-row]), ${row} + [data-testid=robot-progress-row] + tr`)
}

/** The row of a commit, by its subject, in the expanded details of a branch */
export function getBranchCommitRow(page: Page, branchName: string, subject: string): Locator {
  return getBranchDetailsRow(page, branchName).locator("[data-row-id]").filter({ hasText: subject })
}

/** The row of a commit, by its subject, in the unassigned commits card */
export function getUnassignedCommitRow(page: Page, subject: string): Locator {
  return page.getByTestId("unassigned-commits-section").locator("[data-row-id]").filter({ hasText: subject })
}

/**
 * Get the copy button from a branch row's actions cell
 */
export function getCopyButton(branchRow: Locator): Locator {
  const { actions } = getBranchCells(branchRow)
  return actions.locator("button").first()
}

/** The chip after the name of a stack root: its size, the toggle of its collapse */
export function getStackChip(row: Locator): Locator {
  return row.getByTestId("stack-chip")
}

/** The push button of a branch row's actions cell: "↑ 3", named by its label ("Push (3)") */
export function getPushButton(branchRow: Locator): Locator {
  const { actions } = getBranchCells(branchRow)
  return actions.getByTestId("push-button")
}

/**
 * Pushes a branch with the push of its row's menu ("Push (2)"): the first push of a branch whose Safe Push or Create
 * Pull Request pushes it has no button on the row, only this item
 */
export async function pushFromRowMenu(page: Page, branchName: string): Promise<void> {
  await findBranchRow(page, branchName).click({ button: "right" })
  await page.getByRole("menuitem", { name: /^Push \(\d+\)/ }).click()
}

/** A pushed branch: its row has no push button, only its empty place, as the status says it is up to date */
export async function expectNothingToPush(branchRow: Locator): Promise<void> {
  await expect(getPushButton(branchRow)).toHaveCount(0)
  await expect(getBranchCells(branchRow).actions.getByTestId("push-placeholder")).toHaveCount(1)
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
  await expect(status.getByTestId("branch-status")).toContainText(expectedStatus)
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
