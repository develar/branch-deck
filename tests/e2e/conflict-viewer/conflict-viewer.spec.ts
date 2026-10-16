// @spec spec/commit/commit-row.spec.md
import { testWithConflicts, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { findBranchRow, expandBranch, getBranchDetailsRow, verifyBranchState } from "../helpers/branch-helpers"
import { syncBranches } from "../helpers/sync-helpers"
import { waitForConflictViewer, waitForStorePersistence } from "../helpers/wait-helpers"

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
      await waitForConflictViewer(page, bugFixDetails)
      // the badge says "Merge conflict"; the title names the commit and what it does not apply onto
      await expect(bugFixDetails.getByTestId("conflict-title")).toHaveText(/^“Implement secure password hashing” does not apply cleanly onto (origin\/)?master$/)
      // the explanation names the branches of the missing commits and what the branch is built on
      const explanation = bugFixDetails.getByTestId("conflict-explanation")
      await expect(explanation).toContainText("1 commit of feature-auth changed these files first, as did 1 unassigned commit")
      await expect(explanation).toContainText(/bug-fix is built on (origin\/)?master, which doesn't have them\./)
      // the missing commits are grouped by their branch, the unassigned ones last
      await expect(bugFixDetails.getByTestId("missing-commits").locator("[data-missing-group]")).toHaveText(["feature-auth1 commit", "Unassigned1 commit"])

      await captureHtmlSnapshot(bugFixDetails, "conflict-viewer")
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

    test("should list every conflicting file, the first open, and say that a file without conflict markers is a modify/delete", async ({ page }) => {
      const bugFixBranch = findBranchRow(page, "bug-fix")
      await expandBranch(page, bugFixBranch)
      const bugFixDetails = getBranchDetailsRow(page, "bug-fix")
      await waitForConflictViewer(page, bugFixDetails)

      const conflictingFiles = bugFixDetails.getByTestId("conflicting-files")
      const badge = conflictingFiles.locator("[data-slot=badge]").first()
      await expect(badge).toHaveText(/^[1-9]\d* files?$/)
      const fileCount = Number.parseInt((await badge.textContent())!, 10)

      const files = conflictingFiles.getByTestId("file-diff")
      await expect(files).toHaveCount(fileCount)
      const file = files.first()
      await expect(file.getByTestId("file-conflict-count")).toHaveText("modify/delete")
      // open without a click: the change of the commit under the line that says what happened
      await expect(file).toContainText("One side deleted the file, the other changed it: the change of the commit")
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

  test.describe("Conflict Viewer Settings", () => {
    test.beforeEach(async ({ setupConflictRepo }) => {
      await setupConflictRepo("conflict_branches")
    })

    test("should persist the diff view mode", async ({ page }) => {
      const bugFixBranch = findBranchRow(page, "bug-fix")
      await expandBranch(page, bugFixBranch)
      const missingCommits = getBranchDetailsRow(page, "bug-fix").getByTestId("missing-commits")
      await waitForConflictViewer(page, getBranchDetailsRow(page, "bug-fix"))
      await missingCommits.getByRole("button", { name: "Split" }).click()
      await expect(missingCommits.getByRole("button", { name: "Split" })).toHaveAttribute("aria-pressed", "true")

      await waitForStorePersistence(page)
      await page.reload()
      await syncBranches(page, ['[data-testid="branch-row"][data-branch-name="feature-auth"]', '[data-testid="branch-row"][data-branch-name="bug-fix"]'])
      const bugFixAfterReload = findBranchRow(page, "bug-fix")
      if ((await bugFixAfterReload.getAttribute("data-state")) !== "open") {
        await expandBranch(page, bugFixAfterReload)
      }
      const missingAfterReload = getBranchDetailsRow(page, "bug-fix").getByTestId("missing-commits")
      await expect(missingAfterReload.getByRole("button", { name: "Split" })).toHaveAttribute("aria-pressed", "true")
    })
  })
})
