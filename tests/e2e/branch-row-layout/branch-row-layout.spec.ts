import type { Locator, Page } from "@playwright/test"
// @spec spec/branch/branch-row.spec.md
import { test, expect } from "../fixtures/test-fixtures"
import { captureBranchTableSnapshot, captureContextMenuSnapshot } from "../helpers/aria-snapshot-helpers"
import {
  findBranchRow,
  getBranchCells,
  expandBranch,
  collapseBranch,
  verifyBranchState,
  verifyBranchStatus,
  bringUnderStillPointer,
  getMoreButton,
  getPushButton,
  getBranchCommitRow,
  verifyCommitCount,
  expectNothingToPush,
} from "../helpers/branch-helpers"
import { pressCopyKey, testContextMenuCopy, readClipboard } from "../helpers/clipboard-helpers"
import { openBranchPreview } from "../helpers/review-status-helpers"
import { closeMenus, openContextMenu, openSubmenu } from "../helpers/selection-helpers"
import { getOpenedSubWindows } from "../helpers/sub-window-helpers"
import { pushedRemote, waitForBranchSyncComplete } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"

test.describe("Branch Row Layout and Copy Functionality", () => {
  let repo: TestRepositoryBuilder

  test.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    // Setup test repository with simple template
    repo = await setupRepo("simple")

    // Sync branches and wait for them to load
    await syncAndWaitForBranches()
  })

  test("should display correct branch row structure", async ({ page }) => {
    // Find the branch row
    const branchRow = findBranchRow(page, "test-branch")

    // Wait for the branch to finish syncing
    await waitForBranchSyncComplete(page, "test-branch")

    // Verify all columns are present
    const { branchName, status, actions } = getBranchCells(branchRow)

    await expect(branchName).toContainText("test-branch")
    await verifyCommitCount(branchRow, 1)
    await expect(status).toBeVisible()
    await verifyBranchStatus(branchRow, "not pushed")
    await expect(actions).toBeVisible()

    // no copy button holds a place in the actions: ⌘C and the menu copy the name
    await expect(getMoreButton(branchRow)).toBeVisible()
    await expect(actions.getByRole("button", { name: /^Copy/ })).toHaveCount(0)

    // Verify expand button is present
    const expandButton = branchName.locator("button").first()
    await expect(expandButton).toBeVisible()

    // The branch should be collapsed after sync (auto-expand is disabled)
    await verifyBranchState(branchRow, "closed")

    // Capture ARIA snapshot of standard (collapsed) branch row
    await captureBranchTableSnapshot(page, branchRow, "branch-row-standard")
  })

  test("should show expanded branch row with commits", async ({ page }) => {
    const branchRow = findBranchRow(page, "test-branch")

    await waitForBranchSyncComplete(page, "test-branch")

    // Manually expand the branch
    await expandBranch(page, branchRow)

    // Verify commits are visible in the expanded state
    const expandedRow = page.locator("tr").filter({ hasText: "foo" }).and(page.locator("tr:not([data-testid])"))
    await expect(expandedRow.first()).toBeVisible()

    // Capture ARIA snapshot of expanded branch row
    await captureBranchTableSnapshot(page, branchRow, "branch-row-expanded")

    // Test collapse/expand behavior
    await collapseBranch(page, branchRow)

    // Wait for content to be hidden
    await expect(expandedRow.first()).not.toBeVisible()

    // Expand again
    await expandBranch(page, branchRow)

    // Wait for content to be visible again
    await expect(expandedRow.first()).toBeVisible()
  })

  test("should display context menu with copy options", async ({ page }) => {
    const branchRow = findBranchRow(page, "test-branch")

    await waitForBranchSyncComplete(page, "test-branch")

    // Right-click to open context menu
    await openContextMenu(page, branchRow)

    // the menu by how often its actions are wanted: Safe Push first (no Patronus here: the push), the copies last
    await expect(page.getByRole("menuitem").first()).toHaveText(/^Push/)
    await captureContextMenuSnapshot(page, "branch-row-context-menu")

    // both copies in Copy ›
    await openSubmenu(page, "Copy")
    await expect(page.getByRole("menuitem", { name: "Copy Branch Name" })).toBeVisible()
    await expect(page.getByRole("menuitem", { name: "Copy Full Branch Name" })).toBeVisible()
  })

  test("should close the branch menu without its zoom under reduced motion", async ({ page }) => {
    const branchRow = findBranchRow(page, "test-branch")
    await waitForBranchSyncComplete(page, "test-branch")
    // the animation of each menu as it starts to close
    await page.evaluate(() => {
      const target = window as unknown as { __closingAnimations: string[] }
      target.__closingAnimations = []
      new MutationObserver(records => {
        for (const record of records) {
          if (record.target instanceof Element && record.target.matches("[data-slot=context-menu-content][data-closed]")) {
            target.__closingAnimations.push(getComputedStyle(record.target).animationName)
          }
        }
      }).observe(document.body, { subtree: true, attributes: true, attributeFilter: ["data-closed"] })
    })
    const openAndClose = async () => {
      await openContextMenu(page, branchRow)
      await expect(page.getByRole("menu")).toBeVisible()
      await page.keyboard.press("Escape")
      await expect(page.getByRole("menu")).toHaveCount(0)
      return page.evaluate(() => (window as unknown as { __closingAnimations: string[] }).__closingAnimations)
    }

    // the menu zooms out as it closes
    expect(await openAndClose()).toEqual(["exit"])
    // the OS asks for reduced motion: it goes without
    await page.emulateMedia({ reducedMotion: "reduce" })
    expect(await openAndClose()).toEqual(["exit", "none"])
  })

  test("should copy branch name via context menu", async ({ page }) => {
    const branchRow = findBranchRow(page, "test-branch")

    await waitForBranchSyncComplete(page, "test-branch")

    // Test Copy Branch Name
    await openContextMenu(page, branchRow)
    await openSubmenu(page, "Copy")
    await testContextMenuCopy(page, "Copy Branch Name", "test-branch")

    // Test Copy Full Branch Name
    await openContextMenu(page, branchRow)
    await openSubmenu(page, "Copy")
    await testContextMenuCopy(page, "Copy Full Branch Name", /^[\w-]+\/virtual\/test-branch$/)
  })

  test("should copy the full branch name with ⌘C on the focused row, the key hint of Copy Full Branch Name", async ({ page }) => {
    const branchRow = findBranchRow(page, "test-branch")
    await waitForBranchSyncComplete(page, "test-branch")

    await openContextMenu(page, branchRow)
    await openSubmenu(page, "Copy")
    await expect(page.getByRole("menuitem", { name: "Copy Full Branch Name" }).locator("kbd")).not.toHaveCount(0)
    await closeMenus(page)

    await branchRow.focus()
    await pressCopyKey(page)
    await expect.poll(() => readClipboard(page)).toMatch(/^[\w-]+\/virtual\/test-branch$/)
  })

  test("should open the branch menu from the more actions button without toggling the row", async ({ page }) => {
    const branchRow = findBranchRow(page, "test-branch")
    await waitForBranchSyncComplete(page, "test-branch")

    await branchRow.getByRole("button", { name: "More actions for test-branch" }).click()
    // the menu of the right-click
    await expect(page.getByRole("menuitem", { name: "Rename…" })).toBeVisible()
    await openSubmenu(page, "Copy")
    await testContextMenuCopy(page, "Copy Branch Name", "test-branch")
    await expect(page.getByRole("menu")).toHaveCount(0)
    await verifyBranchState(branchRow, "closed")
  })

  test("should open the popups of a row from its first hover or focus, and the branch menu only from the row", async ({ page }) => {
    const branchRow = findBranchRow(page, "test-branch")
    await waitForBranchSyncComplete(page, "test-branch")

    // the status text mounts its tooltip under the pointer: one hover opens it
    await branchRow.getByTestId("branch-status").hover()
    await expect(page.getByRole("tooltip")).toContainText("the push creates it")

    // "⋯" mounts its menu with the focus and keeps the focus: Enter opens the menu
    await getMoreButton(branchRow).focus()
    await expect(getMoreButton(branchRow)).toBeFocused()
    await page.keyboard.press("Enter")
    await expect(page.getByRole("menuitem", { name: "Rename…" })).toBeVisible()
    await closeMenus(page)

    // a commit under the row opens its own menu, not the branch's
    await expandBranch(page, branchRow)
    await openContextMenu(page, getBranchCommitRow(page, "test-branch", "foo"))
    await expect(page.getByRole("menu")).toBeVisible()
    await expect(page.getByRole("menuitem", { name: "Rename…" })).toHaveCount(0)
    await closeMenus(page)

    // the row opens the branch menu
    await openContextMenu(page, branchRow)
    await expect(page.getByRole("menuitem", { name: "Rename…" })).toBeVisible()
  })

  test("should keep the actions of the branch menu during a sync, the rewrites enabled", async ({ page }) => {
    const branchRow = findBranchRow(page, "test-branch")
    await waitForBranchSyncComplete(page, "test-branch")
    // a sync of the session that runs while the menu is open
    await repo.holdRemoteAnswers({ hold: ["sync"] })
    const syncButton = page.getByTestId("sync-button")
    await syncButton.click()
    await expect(syncButton).toHaveAttribute("aria-busy", "true")

    await openContextMenu(page, branchRow)
    // a rewrite goes to the session at once, named on the table the row shows
    await expect(page.getByRole("menuitem", { name: "Rename…" })).toBeEnabled()
    await expect(page.getByRole("menuitem", { name: "Move to…" })).toBeEnabled()
    // a review rewrites nothing: it opens during the sync
    await page.getByRole("menuitem", { name: "Review Changes" }).click()
    await expect.poll(async () => (await getOpenedSubWindows(page)).map(window => window.url)).toEqual([expect.stringMatching(/branch-review/)])
    await repo.holdRemoteAnswers({ release: ["sync"] })
    await expect(syncButton).not.toHaveAttribute("aria-busy", "true")
  })

  test("should offer the menu of the row in the branch preview, and close the preview for a form of the row", async ({ page }) => {
    await waitForBranchSyncComplete(page, "test-branch")
    const preview = await openBranchPreview(page, "test-branch")
    await preview.getByRole("button", { name: "More actions for test-branch" }).click()
    // the items of the row's menu; M is no key of the preview: no hint
    const move = page.getByRole("menuitem", { name: "Move to…" })
    await expect(move).toBeVisible()
    await expect(move.locator("kbd")).toHaveCount(0)
    await page.getByRole("menuitem", { name: "Rename…" }).click()
    await expect(preview).toHaveCount(0)
    await expect(page.getByTestId("inline-rename-input")).toBeVisible()
  })

  test("should handle branch with commits state", async ({ page }) => {
    const branchRow = findBranchRow(page, "test-branch")

    await waitForBranchSyncComplete(page, "test-branch")

    // Check actions column: the push and the branch menu ("Safe Push" only on Space)
    const { actions } = getBranchCells(branchRow)
    const buttons = actions.locator("button")
    await expect(buttons).toHaveCount(2)
    await expect(buttons.nth(1)).toHaveAccessibleName("More actions for test-branch")

    // Capture HTML snapshot of the branch row with commits state
    await captureBranchTableSnapshot(page, branchRow, "branch-with-commits-state")
  })

  test("should verify final branch state after all tests", async ({ page }) => {
    const branchRow = findBranchRow(page, "test-branch")

    await waitForBranchSyncComplete(page, "test-branch")

    // Verify the branch row is still functional
    await expect(branchRow).toBeVisible()

    // Verify we can still access the context menu
    await openContextMenu(page, branchRow)
    await expect(page.getByRole("menuitem", { name: "Rename…" })).toBeVisible()

    // Close context menu
    await page.keyboard.press("Escape")
  })
})

test.describe("Status of a pushed branch", () => {
  test("should show nothing in the Status cell of a branch up to date with its remote, which needs no action", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("simple", { syncOverrides: pushedRemote("test-branch") })
    await syncAndWaitForBranches()
    const row = findBranchRow(page, "test-branch")
    await verifyCommitCount(row, 1)
    // pushed, the remote as the branch: its preview says it, the cell is empty and the row offers no push
    const preview = await openBranchPreview(page, "test-branch")
    await expect(preview.getByTestId("branch-preview-status")).toHaveText("up to date")
    await page.keyboard.press("Space")
    await expect(preview).toHaveCount(0)
    await expect(getBranchCells(row).status.getByTestId("branch-status")).toHaveCount(0)
    await expect(row.getByTestId("branch-status-dot")).toHaveCount(0)
    await expectNothingToPush(row)
  })
})

test.describe("Hover over rows that move", () => {
  /** `trigger` under a still pointer (`bringUnderStillPointer`), its root mounted: Base UI's trigger, with its id */
  async function bringTriggerUnderStillPointer(page: Page, trigger: Locator): Promise<{ x: number; y: number }> {
    const point = await bringUnderStillPointer(page, trigger, findBranchRow(page, "docs"))
    await expect(trigger).toHaveAttribute("id", /^base-ui-/)
    return point
  }

  test(
    "should open no hover card or tooltip of a trigger that came under a still pointer, and open it at the next move",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      // docs, then auth and session stacked, none pushed: session's push button has its description, auth's status its tooltip
      await setupRepo("stacked_branches")
      await syncAndWaitForBranches()
      const push = getPushButton(findBranchRow(page, "session"))
      const description = page.getByTestId("push-description")
      // the trigger of the tooltip of auth's status: the text with its dot
      const status = findBranchRow(page, "auth").locator("[data-slot=tooltip-trigger]", { has: page.getByTestId("branch-status") })
      const tooltip = page.getByRole("tooltip").filter({ hasText: "The branch isn't on the remote yet: the push creates it." })

      // the delay of the hover runs out with the pointer still: the description, whose Enter pushed, stays closed
      const point = await bringTriggerUnderStillPointer(page, push)
      await page.clock.fastForward(1000)
      await expect(description).toHaveCount(0)
      // a move of a pixel over the button opens it after the delay
      await page.mouse.move(point.x + 1, point.y)
      await page.clock.fastForward(1000)
      await expect(description).toContainText("Push session Only")
      await page.mouse.move(640, 700)
      await expect(description).toHaveCount(0)

      // the same for the tooltip of a status
      const at = await bringTriggerUnderStillPointer(page, status)
      await page.clock.fastForward(1000)
      await expect(tooltip).toHaveCount(0)
      await page.mouse.move(at.x + 1, at.y)
      await page.clock.fastForward(1000)
      await expect(tooltip).toBeVisible()
    },
  )
})
