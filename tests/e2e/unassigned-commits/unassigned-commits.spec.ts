import { testWithConflicts, test as baseTest, expect } from "../fixtures/test-fixtures"
import { waitForSyncComplete } from "../helpers/sync-helpers"
import { selectCommit, clickGroupIntoBranchButton, multiSelectCommit } from "../helpers/selection-helpers"
import { inlineBranchCreator, submitInlineForm, testInputAutoFocus } from "../helpers/inline-form-helpers"
import { captureHtmlSnapshot, captureFloatingElementSnapshot } from "../helpers/aria-snapshot-helpers"
import { waitForPopover } from "../helpers/wait-helpers"
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

      // Click "Group into Branch" button
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

      // Click "Group into Branch" button
      await clickGroupIntoBranchButton(page)

      // Inline branch creator should appear
      await inlineBranchCreator.waitForVisible(page)

      // Type a branch name and submit
      const branchNameInput = inlineBranchCreator.getInput(page)
      await submitInlineForm(branchNameInput, "security")

      // Wait for branch creation to complete
      await inlineBranchCreator.waitForHidden(page)

      // Wait for the automatic sync to complete
      await waitForSyncComplete(page)

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

      // Wait for popover to appear
      const popover = await waitForPopover(page, "Selection shortcuts")

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

      // Verify selection state
      await captureHtmlSnapshot(unassignedSection, "unassigned-commits-one-selected")

      // Multi-select the bcrypt dependency commit
      const bcryptCommit = page.locator("[data-row-id]:has-text('Add bcrypt dependency')")
      await multiSelectCommit(page, bcryptCommit)

      // Both commits should be selected
      await captureHtmlSnapshot(unassignedSection, "unassigned-commits-both-selected")

      // Verify floating selection bar appears
      const floatingBar = page.locator("[data-testid=\"floating-selection-bar\"]")
      await expect(floatingBar).toBeVisible()
      await captureHtmlSnapshot(floatingBar, "floating-selection-bar")
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