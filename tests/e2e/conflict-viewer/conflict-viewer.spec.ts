import { testWithConflicts, expect } from "../fixtures/test-fixtures"
import { syncBranches } from "../helpers/sync-helpers"
import { findBranchRow, expandBranch, getBranchDetailsRow, verifyBranchState } from "../helpers/branch-helpers"
import { waitForConflictViewer, waitForStorePersistence } from "../helpers/wait-helpers"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"

const test = testWithConflicts

test.describe("Conflict Viewer Feature", () => {
  test.describe("Branch Conflicts", () => {
    test.beforeEach(async ({ setupConflictRepo }) => {
      await setupConflictRepo("conflict_branches")
    })

    test("should show branches with missing commits", async ({ page }) => {
      // Check for good branch (feature-auth) - should sync cleanly
      const featureAuthBranch = findBranchRow(page, "feature-auth")
      await expect(featureAuthBranch).toBeVisible()

      // Feature-auth should NOT be expanded (synced successfully)
      await verifyBranchState(featureAuthBranch, "closed")

      // Check for bug-fix branch - should have conflicts due to missing commits
      const bugFixBranch = findBranchRow(page, "bug-fix")
      await expect(bugFixBranch).toBeVisible()

      // Expand the bug-fix branch to see conflicts
      await expandBranch(page, bugFixBranch)
      const bugFixDetails = getBranchDetailsRow(page, "bug-fix")

      // Wait for conflict viewer to be visible
      const conflictSection = await waitForConflictViewer(page, bugFixDetails)

      // Capture snapshot with Diff tab (default view)
      await captureHtmlSnapshot(bugFixDetails, "conflict-viewer-diff-tab")

      // Click on 3-way Merge tab
      const threeWayTab = conflictSection.locator("button[role=\"tab\"]:has-text(\"3-way Merge\")")
      await threeWayTab.click()

      // Wait for 3-way content to be visible
      const threeWayPanel = bugFixDetails.locator("[role=\"tabpanel\"][data-state=\"active\"]").first()
      await expect(threeWayPanel).toBeVisible()

      // Capture snapshot with 3-way Merge tab
      await captureHtmlSnapshot(bugFixDetails, "conflict-viewer-3way-tab")
    })

    test("should expand/collapse file accordions in missing commits details", async ({ page }) => {
      // Find the bug-fix branch that has conflicts
      const bugFixBranch = findBranchRow(page, "bug-fix")
      await expect(bugFixBranch).toBeVisible()

      // Expand the bug-fix branch to see conflicts
      await expandBranch(page, bugFixBranch)
      const bugFixDetails = getBranchDetailsRow(page, "bug-fix")

      // Wait for conflict viewer to be visible
      await waitForConflictViewer(page, bugFixDetails)

      // Find the file accordion button and wait for it to be visible
      const fileAccordion = bugFixDetails.locator("button:has-text('src/main/kotlin/com/example/service/UserService.kt')").first()
      await expect(fileAccordion).toBeVisible()

      // Capture initial state (accordion collapsed) - capture the entire details row
      await captureHtmlSnapshot(bugFixDetails, "missing-commits-accordion-collapsed")

      // Click to expand the accordion
      await fileAccordion.click()

      // Wait for accordion content to be visible - look for the expanded state
      await expect(fileAccordion).toHaveAttribute("aria-expanded", "true")

      // Wait for expansion animation to complete
      await page.waitForTimeout(300)

      // Capture expanded state
      await captureHtmlSnapshot(bugFixDetails, "missing-commits-accordion-expanded")

      // Click accordion again to collapse
      await fileAccordion.click()

      // Verify accordion is collapsed
      await expect(fileAccordion).toHaveAttribute("aria-expanded", "false")

      // Capture final collapsed state
      await captureHtmlSnapshot(bugFixDetails, "missing-commits-accordion-collapsed-final")
    })
  })

  test.describe("Conflict Viewer Settings", () => {
    test.beforeEach(async ({ setupConflictRepo }) => {
      await setupConflictRepo("conflict_branches")
    })

    test("should persist conflict viewer settings", async ({ page }) => {
      // Find the bug-fix branch that has conflicts
      const bugFixBranch = findBranchRow(page, "bug-fix")
      await expect(bugFixBranch).toBeVisible()

      // Expand the bug-fix branch to see conflicts
      await expandBranch(page, bugFixBranch)
      const bugFixDetails = getBranchDetailsRow(page, "bug-fix")

      // Get conflict viewer
      const conflictSection = await waitForConflictViewer(page, bugFixDetails)

      // Switch to 3-way view
      const threeWayTab = conflictSection.locator("button[role=\"tab\"]:has-text(\"3-way Merge\")")
      await threeWayTab.click()

      // Wait for 3-way content to be visible
      const threeWayPanel = bugFixDetails.locator("[role=\"tabpanel\"][data-state=\"active\"]").first()
      await expect(threeWayPanel).toBeVisible()

      // Capture state before reload
      await captureHtmlSnapshot(bugFixDetails, "conflict-viewer-before-reload")

      // Wait for store persistence
      await waitForStorePersistence(page)

      // Reload page to test persistence
      await page.reload()

      // Need to sync branches again after reload
      await syncBranches(page, [
        "[data-testid=\"branch-row\"][data-branch-name=\"feature-auth\"]",
        "[data-testid=\"branch-row\"][data-branch-name=\"bug-fix\"]",
      ])

      // Wait for bug-fix branch and expand it again
      const bugFixBranchAfterReload = findBranchRow(page, "bug-fix")
      await expect(bugFixBranchAfterReload).toBeVisible()
      await expandBranch(page, bugFixBranchAfterReload)
      const bugFixDetailsAfterReload = getBranchDetailsRow(page, "bug-fix")

      // Wait for conflict viewer after reload
      await waitForConflictViewer(page, bugFixDetailsAfterReload)

      // Capture state after reload - should show 3-way tab selected
      await captureHtmlSnapshot(bugFixDetailsAfterReload, "conflict-viewer-after-reload")
    })
  })
})
