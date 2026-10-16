// @spec spec/commit/commit-row.spec.md
import type { Locator, Page } from "@playwright/test"
import { testWithConflicts, test as baseTest, expect } from "../fixtures/test-fixtures"
import { finishedSyncCount, waitForSyncComplete } from "../helpers/sync-helpers"
import { selectCommit, clickGroupIntoBranchButton, multiSelectCommit, shiftSelectCommit, selectAllCommits, openContextMenu, getSelectedCommits } from "../helpers/selection-helpers"
import { inlineBranchCreator, submitInlineForm, testInputAutoFocus } from "../helpers/inline-form-helpers"
import { captureHtmlSnapshot, captureFloatingElementSnapshot } from "../helpers/aria-snapshot-helpers"
import { findBranchRow } from "../helpers/branch-helpers"

baseTest.describe("Unassigned Commits Feature", () => {
  testWithConflicts.describe("Missing Commits Detection", () => {
    testWithConflicts.beforeEach(async ({ setupConflictRepo }) => {
      await setupConflictRepo("conflict_unassigned")
    })

    testWithConflicts("should show good feature-auth branch and unassigned commits", async ({ page }) => {
      // Check for good branch (feature-auth) - should sync cleanly
      const featureAuthBranch = findBranchRow(page, "feature-auth")
      await expect(featureAuthBranch).toBeVisible()

      // Feature-auth should show as synced successfully (not expanded)
      await expect(featureAuthBranch).not.toHaveClass(/.*expanded.*/)

      // Get the unassigned commits section
      const unassignedSection = page.locator("[data-testid=\"unassigned-commits-section\"]")

      // Should have unassigned commits section
      await expect(unassignedSection).toBeVisible()

      // Capture HTML snapshot of unassigned commits section
      await captureHtmlSnapshot(unassignedSection, "unassigned-commits-section")

      // Test creating a branch with only the password hashing commit (will cause missing commits)
      // Select the "Implement secure password hashing" commit which depends on "Add bcrypt dependency"
      const passwordHashingCommit = page.locator("[data-row-id]:has-text('Implement secure password hashing')")
      await expect(passwordHashingCommit).toBeVisible()
      await selectCommit(page, passwordHashingCommit)

      // Click the "Move to…" button
      await clickGroupIntoBranchButton(page)

      // Inline branch creator should appear
      await inlineBranchCreator.waitForVisible(page)

      // The input should be focused
      const branchNameInput = inlineBranchCreator.getInput(page)
      await testInputAutoFocus(branchNameInput)

      // Type a branch name and submit
      await submitInlineForm(branchNameInput, "password-security")

      // Branch creator should disappear after creation
      await inlineBranchCreator.waitForHidden(page)

      // The test demonstrates that a branch can be created with a single commit,
      // even though it depends on another commit. The conflict will be detected
      // during the sync process when the branch is pushed.
    })

    testWithConflicts("should handle branch creation with conflicts correctly", async ({ page }) => {
      // This test verifies that:
      // 1. Creating a branch with commits that have conflicts doesn't crash the app
      // 2. The branch is not shown in the UI (correct behavior - branches with conflicts aren't created)
      // 3. The frontend handles the missing row gracefully

      // Get the unassigned commits section
      const unassignedSection = page.locator("[data-testid=\"unassigned-commits-section\"]")

      // Wait for unassigned commits to be visible
      await expect(unassignedSection).toBeVisible()

      // Capture HTML snapshot of initial state
      await captureHtmlSnapshot(unassignedSection, "unassigned-commits-before-branch-creation")

      // Select the "Implement secure password hashing" commit which depends on "Add bcrypt dependency"
      const passwordHashingCommit = page.locator("[data-row-id]:has-text('Implement secure password hashing')")
      await expect(passwordHashingCommit).toBeVisible()
      await selectCommit(page, passwordHashingCommit)

      // Click the "Move to…" button
      await clickGroupIntoBranchButton(page)

      // Inline branch creator should appear
      await inlineBranchCreator.waitForVisible(page)

      // Type a branch name and submit
      const branchNameInput = inlineBranchCreator.getInput(page)
      const syncsBefore = await finishedSyncCount(page)
      await submitInlineForm(branchNameInput, "security")

      // Wait for branch creation to complete
      await inlineBranchCreator.waitForHidden(page)

      // Wait for the automatic sync to complete
      await waitForSyncComplete(page, syncsBefore)

      // IMPORTANT: The security branch should NOT appear in the UI
      // This is correct behavior - branches with conflicts are not created
      const securityBranch = findBranchRow(page, "user-name-security")
      await expect(securityBranch).not.toBeVisible()

      // Verify that only feature-auth branch exists
      await expect(findBranchRow(page, "feature-auth")).toBeVisible()

      // The app should not have crashed - verify it's still functional
      // Try to select another commit to ensure the UI is responsive
      const bcryptCommit = page.locator("[data-row-id]:has-text('Add bcrypt dependency')")
      await expect(bcryptCommit).toBeVisible()
      await selectCommit(page, bcryptCommit)

      // Verify we can still interact with the UI
      await expect(bcryptCommit).toHaveAttribute("data-selected", "true")
    })

    testWithConflicts("should verify commit selection states with ARIA snapshots", async ({ page }) => {
      // Get the unassigned commits section
      const unassignedSection = page.locator("[data-testid=\"unassigned-commits-section\"]")

      // Wait for unassigned commits to be visible
      await expect(unassignedSection).toBeVisible()

      // Test help popover by hovering over the info icon
      const helpIcon = page.locator("[data-testid=\"unassigned-commits-help-icon\"]")
      await helpIcon.hover()

      // the popover is titled as the card, then says what is unassigned and the keys of the selection
      const popover = page.locator("[data-slot=popover-content]")
      await expect(popover.locator("[data-slot=popover-title]")).toHaveText("Unassigned Commits")
      await expect(popover).toContainText("Shortcuts")

      // Capture help popover content
      await captureFloatingElementSnapshot(popover, "unassigned-commits-help-popover")

      // Move mouse away to hide popover
      await page.mouse.move(0, 0)
      await expect(popover).not.toBeVisible()

      // Initial state - no commits selected
      await captureHtmlSnapshot(unassignedSection, "unassigned-commits-no-selection")

      // Select the password hashing commit
      const passwordHashingCommit = page.locator("[data-row-id]:has-text('Implement secure password hashing')")
      await selectCommit(page, passwordHashingCommit)

      // the selection's actions appear in the card header
      await expect(unassignedSection.getByTestId("selection-actions")).toHaveText(/1 selected/)

      // Verify selection state
      await captureHtmlSnapshot(unassignedSection, "unassigned-commits-one-selected")

      // Multi-select the bcrypt dependency commit
      const bcryptCommit = page.locator("[data-row-id]:has-text('Add bcrypt dependency')")
      await multiSelectCommit(page, bcryptCommit)

      // Both commits should be selected
      await captureHtmlSnapshot(unassignedSection, "unassigned-commits-both-selected")

      // the header counts both commits and offers the grouping action
      const selectionActions = unassignedSection.getByTestId("selection-actions")
      await expect(selectionActions).toHaveText(/2 selected/)
      await captureHtmlSnapshot(selectionActions, "selection-actions")
    })
  })

  testWithConflicts.describe("Selection Gestures", () => {
    // newest first, as the card lists them
    const SUBJECTS = ["Refactor: Extract configuration constants", "Implement secure password hashing", "Add bcrypt dependency", "Add UserService"]

    testWithConflicts.beforeEach(async ({ page, setupConflictRepo }) => {
      await setupConflictRepo("conflict_unassigned")
      await expect(page.getByTestId("unassigned-commits-section").locator("tr[data-row-id]")).toHaveCount(SUBJECTS.length)
    })

    function commitRow(page: Page, subject: string): Locator {
      return page.getByTestId("unassigned-commits-section").locator("tr[data-row-id]").filter({ hasText: subject })
    }

    testWithConflicts("should select a range of rows with Shift+click", async ({ page }) => {
      await selectCommit(page, commitRow(page, SUBJECTS[0]!))
      await shiftSelectCommit(commitRow(page, SUBJECTS[2]!))

      await expect(page.getByTestId("selection-actions")).toHaveText(/3 selected/)
      await expect(getSelectedCommits(page)).toHaveCount(3)
      await expect(commitRow(page, SUBJECTS[1]!)).toHaveAttribute("data-selected", "true")
      await expect(commitRow(page, SUBJECTS[3]!)).toHaveAttribute("data-selected", "false")
    })

    testWithConflicts("should select all rows with Cmd/Ctrl+A and clear the selection with Escape", async ({ page }) => {
      const table = page.getByTestId("unassigned-commits-section").locator("table")
      await selectAllCommits(page, table)
      await expect(page.getByTestId("selection-actions")).toHaveText(/4 selected/)
      await expect(getSelectedCommits(page)).toHaveCount(SUBJECTS.length)

      await page.keyboard.press("Escape")
      await expect(page.getByTestId("selection-actions")).toHaveCount(0)
      await expect(getSelectedCommits(page)).toHaveCount(0)
    })

    testWithConflicts("should act on all selected commits from the context menu of a selected row", async ({ page }) => {
      await selectCommit(page, commitRow(page, SUBJECTS[1]!))
      await multiSelectCommit(page, commitRow(page, SUBJECTS[2]!))

      await openContextMenu(page, commitRow(page, SUBJECTS[1]!))
      await expect(page.getByRole("menuitem", { name: "Move to…" })).toBeVisible()
      await expect(page.getByRole("menuitem", { name: "Copy 2 Commit Hashes" })).toBeVisible()
      await expect(page.getByRole("menuitem", { name: "Copy 2 Commit Messages" })).toBeVisible()

      // Escape closes the menu and keeps the selection
      await page.keyboard.press("Escape")
      await expect(page.getByRole("menu")).toHaveCount(0)
      await expect(page.getByTestId("selection-actions")).toHaveText(/2 selected/)
    })

    testWithConflicts("should act on that commit only from the context menu of a row that is not selected", async ({ page }) => {
      await selectCommit(page, commitRow(page, SUBJECTS[1]!))
      await multiSelectCommit(page, commitRow(page, SUBJECTS[2]!))

      const unselectedRow = commitRow(page, SUBJECTS[3]!)
      await openContextMenu(page, unselectedRow)
      await expect(page.getByRole("menuitem", { name: "Move to…" })).toBeVisible()
      await expect(page.getByRole("menuitem", { name: "Copy Commit Hash" })).toBeVisible()
      await expect(page.getByRole("menuitem", { name: "Copy 2 Commit Hashes" })).toHaveCount(0)

      // opening the menu does not change the selection
      await expect(unselectedRow).toHaveAttribute("data-selected", "false")
      await page.keyboard.press("Escape")
      await expect(page.getByRole("menu")).toHaveCount(0)
      await expect(page.getByTestId("selection-actions")).toHaveText(/2 selected/)
    })
  })

  baseTest.describe("Empty State and Edge Cases", () => {
    baseTest("should handle empty state with no unassigned commits", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      // Setup test repository with simple template (all commits have prefixes)
      await setupRepo("simple")

      // Sync branches
      await syncAndWaitForBranches()

      // Should NOT have unassigned commits section
      await expect(page.locator("[data-testid=\"unassigned-commits-section\"]")).not.toBeVisible()

      // Verify branches are visible instead
      await expect(page.locator("[data-testid=\"branch-row\"]").first()).toBeVisible()
    })
  })

  baseTest.describe("Singular Form Test", () => {
    baseTest("should display singular form for 1 commit", async ({ page, setupRepo, syncAndWaitForBranches }) => {
      // Setup test repository with single_unassigned template
      await setupRepo("single_unassigned")

      // Sync branches
      await syncAndWaitForBranches()

      // Get the unassigned commits section
      const unassignedSection = page.locator("[data-testid=\"unassigned-commits-section\"]")

      // Should have unassigned commits section
      await expect(unassignedSection).toBeVisible()

      // Should show "1 commit" (singular)
      await expect(page.locator("text=1 commit").first()).toBeVisible()

      // Capture HTML snapshot
      await captureHtmlSnapshot(unassignedSection, "unassigned-commits-single")
    })
  })
})
