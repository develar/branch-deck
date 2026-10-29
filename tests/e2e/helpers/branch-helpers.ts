import type { Page, Locator } from "@playwright/test"
import { expect } from "../fixtures/base-test"
import type { TestRepositoryBuilder } from "./test-repository"

/**
 * Find a branch row by branch name
 */
export function findBranchRow(page: Page, branchName: string): Locator {
  return page.locator(`[data-testid="branch-row"][data-branch-name="${branchName}"]`)
}

/**
 * The pointer rests on the row of `branchName`: on its "⋯", until the tooltip of the button shows, the effect of the
 * rest the user sees. A count of what a hover of a row asks (its IPC) is anchored on it: the hover took place, and
 * time passed as the user rests
 */
export async function restOnBranchRow(page: Page, branchName: string): Promise<void> {
  const row = findBranchRow(page, branchName)
  await row.hover()
  await row.getByRole("button", { name: `More actions for ${branchName}` }).hover()
  await expect(page.getByRole("tooltip", { name: "More actions", exact: true })).toBeVisible()
}

/** `name@depth` of the branch rows in display order */
export async function branchTree(page: Page): Promise<string[]> {
  return page.getByTestId("branch-row").evaluateAll(rows => rows.map(row => `${row.getAttribute("data-branch-name")}@${row.getAttribute("data-depth") ?? 0}`))
}

/**
 * Unstacks oauth from login (`stacked_branches`) with its row's menu while the session holds the rewrite back (the
 * repository lock another process holds, the first sync of a warm start): the "⋯" of the row shows the wait, and its
 * tooltip says `busy`; answers that "⋯"
 */
export async function unstackOauthFromMenuWaiting(page: Page, busy: string | RegExp): Promise<Locator> {
  await page.locator('[data-testid="branch-row"][data-branch-name="oauth"]').click({ button: "right" })
  await page.getByRole("menuitem", { name: "Unstack from login" }).click()
  const more = findBranchRow(page, "oauth").getByRole("button", { name: "More actions for oauth" })
  await expect(more).toHaveAttribute("aria-busy", "true")
  await more.hover()
  // the tooltip of the menu item the click closed may still fade out
  await expect(page.locator("[data-slot=tooltip-content][data-open]", { hasText: busy })).toBeVisible()
  return more
}

/**
 * Releases the sync the session holds (`repo.holdRemoteAnswers({ hold: ["sync"] })`) that the unstack of oauth from login
 * (`stacked_branches`) waits behind: its toast comes with the sync that shows oauth at the root
 */
export async function releaseTheSyncOfTheUnstack(page: Page, repo: TestRepositoryBuilder): Promise<void> {
  await repo.holdRemoteAnswers({ release: ["sync"] })
  await expect(page.getByText(/^Unstacked oauth/)).toBeVisible()
  await expect.poll(() => branchTree(page)).toContain("oauth@0")
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

/** The name of a branch row, the key of the rows below it (`data-branch-of`) */
async function branchNameOf(branchRow: Locator): Promise<string> {
  const name = await branchRow.getAttribute("data-branch-name")
  if (name === null) {
    throw new Error("not a branch row: it has no data-branch-name")
  }
  return name
}

/** The row below a branch row that holds what its expansion shows: its commits, an error, "Loading commits…" */
export function getBranchCommitsRow(page: Page, branchName: string): Locator {
  return page.locator(`[data-testid="branch-commits-row"][data-branch-of="${branchName}"]`)
}

/** The commit rows below a branch row, in any row that belongs to it (`data-branch-of`), in display order */
export function getBranchCommitRows(page: Page, branchName: string): Locator {
  return page.locator(`[data-branch-of="${branchName}"] tr[data-row-id]`)
}

/** Expands a branch row with its folder button: the row of its commits shows below it */
export async function expandBranch(page: Page, branchRow: Locator): Promise<void> {
  if ((await branchRow.getAttribute("data-state")) === "closed") {
    await branchRow.locator("button").first().click()
  }
  await verifyBranchState(branchRow, "open")
}

/**
 * Collapses a branch row with its folder button: of an expanded row, the row of its commits showed first, and then no
 * commit of the branch stays below it
 */
export async function collapseBranch(page: Page, branchRow: Locator): Promise<void> {
  if ((await branchRow.getAttribute("data-state")) === "open") {
    await verifyBranchState(branchRow, "open")
    await branchRow.locator("button").first().click()
  }
  await verifyBranchState(branchRow, "closed")
}

/**
 * The state of a branch row and what it shows below it: expanded, the row of its commits; collapsed, neither that row
 * nor any commit row of the branch, so a row that keeps commits below it while collapsed fails
 */
export async function verifyBranchState(branchRow: Locator, state: "open" | "closed"): Promise<void> {
  await expect(branchRow).toHaveAttribute("data-state", state)
  const page = branchRow.page()
  const name = await branchNameOf(branchRow)
  if (state === "open") {
    await expect(getBranchCommitsRow(page, name)).toBeVisible()
  } else {
    await expect(getBranchCommitsRow(page, name)).toHaveCount(0)
    await expect(getBranchCommitRows(page, name)).toHaveCount(0)
  }
}

/**
 * Brings `target` under a still pointer: the pointer goes where `target` is while the branch row `above` is collapsed,
 * `above` expands by → (its commits lie there then), and ← collapses it: the rows move up and `target` comes under the
 * pointer, which does not move. WebKit tells the pointer is over it at once, Chromium at the next event of the pointer:
 * a "move" to where it is, which ends this. Returns that point.
 */
export async function bringUnderStillPointer(page: Page, target: Locator, above: Locator): Promise<{ x: number; y: number }> {
  const toggle = async (key: "ArrowRight" | "ArrowLeft") => {
    await page.keyboard.press(key)
    await verifyBranchState(above, key === "ArrowRight" ? "open" : "closed")
  }
  const isUnder = (at: { x: number; y: number }) => target.evaluate((element, point) => element.contains(document.elementFromPoint(point.x, point.y)), at)
  await above.focus()
  // expanded once first: the table measured its columns for both
  await toggle("ArrowRight")
  await toggle("ArrowLeft")
  const box = (await target.boundingBox())!
  const point = { x: box.x + box.width / 2, y: box.y + box.height / 2 }
  await toggle("ArrowRight")
  await page.mouse.move(point.x, point.y)
  await expect.poll(() => isUnder(point)).toBe(false)
  await toggle("ArrowLeft")
  await expect.poll(() => isUnder(point)).toBe(true)
  await page.mouse.move(point.x, point.y)
  return point
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

/** The node of a branch row: where the branch is on its way into master, its kind in `data-kind` (`getLandingNode`) */
export function getLandingNode(branchRow: Locator): Locator {
  return branchRow.getByTestId("lane-node")
}

/** The chip after the name of a stack root: its size, the toggle of its collapse */
export function getStackChip(row: Locator): Locator {
  return row.getByTestId("stack-chip")
}

/** The push button of a branch row's actions cell: "↑ 3", named by its label ("Push Branch (3)") */
export function getPushButton(branchRow: Locator): Locator {
  const { actions } = getBranchCells(branchRow)
  return actions.getByTestId("push-button")
}

/**
 * Pushes a branch with the push of its row's menu ("Push Branch (2)"): the first push of a branch whose Safe Push or Create
 * Pull Request pushes it has no button on the row, only this item
 */
export async function pushFromRowMenu(page: Page, branchName: string): Promise<void> {
  await findBranchRow(page, branchName).click({ button: "right" })
  await page.getByRole("menuitem", { name: /^Push Branch \(\d+\)/ }).click()
}

/** The "⋯" of a branch row's actions cell, the last of its actions */
export function getMoreButton(branchRow: Locator): Locator {
  return getBranchCells(branchRow).actions.getByRole("button", { name: /^More actions for / })
}

/** A row that offers no push (pushed, or its push a step of its landing): no push button, the rest of its actions there */
export async function expectNothingToPush(branchRow: Locator): Promise<void> {
  await expect(getMoreButton(branchRow)).toHaveCount(1)
  await expect(getPushButton(branchRow)).toHaveCount(0)
}

/** The Commits cell of a branch row shows `expected`, the count alone under its head */
export async function verifyCommitCount(branchRow: Locator, expected: number): Promise<void> {
  const { commits } = getBranchCells(branchRow)
  await expect(commits).toHaveText(String(expected))
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
