// @spec spec/commit/commit-row.spec.md
import { testWithConflicts, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { findBranchRow, expandBranch, getBranchDetailsRow, verifyBranchState } from "../helpers/branch-helpers"
import { syncBranches } from "../helpers/sync-helpers"
import { waitForConflictViewer, waitForStorePersistence, waitForTabSwitchSettled } from "../helpers/wait-helpers"

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
      // the badge says "Merge conflict"; the title names the commit and what it does not apply onto
      await expect(bugFixDetails.getByTestId("conflict-title")).toHaveText(/^“Implement secure password hashing” does not apply cleanly onto (origin\/)?master$/)
      // the explanation names the branches of the missing commits and what the branch is built on
      const explanation = bugFixDetails.getByTestId("conflict-explanation")
      await expect(explanation).toContainText("1 commit of feature-auth changed these files first, as did 1 unassigned commit")
      await expect(explanation).toContainText(/bug-fix is built on (origin\/)?master, which doesn't have them\./)
      // the missing commits are grouped by their branch, the unassigned ones last
      await expect(bugFixDetails.getByTestId("missing-commits").locator("[data-missing-group]")).toHaveText(["feature-auth1 commit", "Unassigned1 commit"])

      // Capture snapshot with Diff tab (default view)
      await captureHtmlSnapshot(bugFixDetails, "conflict-viewer-diff-tab")

      // Click on 3-way Merge tab
      const threeWayTab = conflictSection.locator('button[role="tab"]:has-text("3-way Merge")')
      await threeWayTab.click()

      // Wait for 3-way content to be visible and the Diff panel to finish its exit transition
      await waitForTabSwitchSettled(bugFixDetails)

      // Capture snapshot with 3-way Merge tab
      await captureHtmlSnapshot(bugFixDetails, "conflict-viewer-3way-tab")
    })

    test("should show the branch of the missing commits from the explanation", async ({ page }) => {
      const bugFixBranch = findBranchRow(page, "bug-fix")
      await expandBranch(page, bugFixBranch)
      const bugFixDetails = getBranchDetailsRow(page, "bug-fix")
      await waitForConflictViewer(page, bugFixDetails)

      await bugFixDetails.getByTestId("missing-commit-owners").getByRole("button", { name: "feature-auth" }).click()
      // the branch opens with its missing commit
      await expect(getBranchDetailsRow(page, "feature-auth")).toContainText("Add authentication to UserService")
    })

    test("should list every conflicting file in the Diff View, also one without conflict markers", async ({ page }) => {
      const bugFixBranch = findBranchRow(page, "bug-fix")
      await expandBranch(page, bugFixBranch)
      const bugFixDetails = getBranchDetailsRow(page, "bug-fix")
      await waitForConflictViewer(page, bugFixDetails)

      const conflictingFiles = bugFixDetails.getByTestId("conflicting-files")
      // "Conflicts only" is on by default
      await expect(conflictingFiles.getByRole("switch")).toBeChecked()
      const badge = conflictingFiles.locator("[data-slot=badge]").first()
      await expect(badge).toHaveText(/^[1-9]\d* files?$/)
      const fileCount = Number.parseInt((await badge.textContent())!, 10)

      // the accordion is never empty under the badge: a file without conflict markers keeps its whole diff
      const files = conflictingFiles.getByRole("tabpanel").getByTestId("file-diff")
      await expect(files).toHaveCount(fileCount)
      const file = files.first()
      await file.getByRole("button", { name: "src/main/kotlin/com/example/service/UserService.kt" }).click()
      await expect(file).toContainText("No conflict markers in this file: showing the whole diff")
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

      // Find the file accordion button of a missing commit (the conflicting files above list the same file)
      const fileAccordion = bugFixDetails.getByTestId("missing-commits").locator("button:has-text('src/main/kotlin/com/example/service/UserService.kt')").first()
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

  test.describe("Commit Preview", () => {
    test.beforeEach(async ({ setupConflictRepo }) => {
      await setupConflictRepo("conflict_branches")
    })

    test("should open one preview for a missing commit nested in a conflict row", async ({ page }) => {
      const bugFixBranch = findBranchRow(page, "bug-fix")
      await expandBranch(page, bugFixBranch)
      const bugFixDetails = getBranchDetailsRow(page, "bug-fix")
      await waitForConflictViewer(page, bugFixDetails)

      // the missing commit row sits inside the row of the conflicting commit; both match `:hover`
      const missingCommitRow = bugFixDetails.locator("tr[data-row-id] tr[data-row-id]").filter({ hasText: "Add UserService" })
      await missingCommitRow.hover()
      await page.keyboard.press("Space")

      const preview = page.getByTestId("commit-preview")
      await expect(preview).toHaveCount(1)
      await expect(preview.getByRole("heading", { level: 2 })).toHaveText("Add UserService")
      await expect(preview.getByTestId("commit-preview-position")).toHaveText("2 / 2")
      // a missing commit is not on the current branch: its message is read-only
      await expect(preview.getByRole("button", { name: /Edit/ })).toHaveCount(0)
    })
  })

  test.describe("Commit Hash Popover", () => {
    test.beforeEach(async ({ setupConflictRepo }) => {
      await setupConflictRepo("conflict_branches")
    })

    test("should show the commit header on hover over a commit hash of the 3-way view", async ({ page }) => {
      // the unassigned commits render above the table after the branches: the hover must come after that shift
      await expect(page.getByTestId("unassigned-commits-section")).toBeVisible()
      const bugFixBranch = findBranchRow(page, "bug-fix")
      await expandBranch(page, bugFixBranch)
      const bugFixDetails = getBranchDetailsRow(page, "bug-fix")
      const conflictSection = await waitForConflictViewer(page, bugFixDetails)
      // the 3-way view names the commit of each side above it: the target is the base of the branch
      await conflictSection.locator('button[role="tab"]:has-text("3-way Merge")').click()
      await waitForTabSwitchSettled(bugFixDetails)
      const conflictingFiles = bugFixDetails.getByTestId("conflicting-files")
      await conflictingFiles.getByRole("tabpanel").getByRole("button", { name: "src/main/kotlin/com/example/service/UserService.kt", exact: true }).click()

      const hash = conflictingFiles.locator("p", { hasText: /^Changes from base to current HEAD / }).locator("[data-slot=popover-trigger]")
      await expect(hash).toHaveText(/^[0-9a-f]{7,}$/)
      const shortHash = (await hash.textContent())!

      await hash.hover()
      const popover = page.locator("[data-slot=popover-content]")
      await expect(popover).toBeVisible()
      // the popover repeats the commit header: the subject above the meta line that starts with the short hash
      await expect(popover).toContainText("Initial project setup")
      await expect(popover.locator(".font-mono").first()).toHaveText(shortHash)
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
      const threeWayTab = conflictSection.locator('button[role="tab"]:has-text("3-way Merge")')
      await threeWayTab.click()

      // Wait for 3-way content to be visible and the Diff panel to finish its exit transition
      await waitForTabSwitchSettled(bugFixDetails)

      // Capture state before reload
      await captureHtmlSnapshot(bugFixDetails, "conflict-viewer-before-reload")

      // Wait for store persistence
      await waitForStorePersistence(page)

      // Reload page to test persistence
      await page.reload()

      // Need to sync branches again after reload
      await syncBranches(page, ['[data-testid="branch-row"][data-branch-name="feature-auth"]', '[data-testid="branch-row"][data-branch-name="bug-fix"]'])

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
