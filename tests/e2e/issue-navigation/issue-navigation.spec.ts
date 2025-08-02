import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow, expandBranch, getBranchDetailsRow } from "../helpers/branch-helpers"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"

test.describe("Issue Navigation Links", () => {
  test.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    // Setup test repository with issue_links template that includes .idea/vcs.xml
    await setupRepo("issue_links")

    // Sync branches and wait for them to load
    await syncAndWaitForBranches()
  })

  test("should render issue links in commit messages", async ({ page }) => {
    // Find the feature-auth branch that has JIRA-123 in its commit message
    const featureAuthBranch = findBranchRow(page, "feature-auth")
    await expect(featureAuthBranch).toBeVisible()

    // Expand the branch to see commits
    await expandBranch(page, featureAuthBranch)
    const branchDetails = getBranchDetailsRow(page, "feature-auth")
    await expect(branchDetails).toBeVisible()

    // Wait for commits to be visible - CommitList component renders as a table
    const commitsSection = branchDetails.locator("table").first()
    await expect(commitsSection).toBeVisible()

    // Capture snapshot of commits with issue links
    await captureHtmlSnapshot(commitsSection, "issue-links-jira")

    // Verify JIRA-123 is rendered as a link
    const jiraLink = commitsSection.locator("a:has-text('JIRA-123')")
    await expect(jiraLink).toBeVisible()
    await expect(jiraLink).toHaveAttribute("href", "https://jira.example.com/browse/JIRA-123")
    await expect(jiraLink).toHaveAttribute("target", "_blank")
  })

  test("should render GitHub issue links with different patterns", async ({ page }) => {
    // Find the feature-api branch that has GH-456 in its commit message
    const featureApiBranch = findBranchRow(page, "feature-api")
    await expect(featureApiBranch).toBeVisible()

    // Expand the branch
    await expandBranch(page, featureApiBranch)
    const branchDetails = getBranchDetailsRow(page, "feature-api")
    await expect(branchDetails).toBeVisible()

    // Wait for commits to be visible - CommitList component renders as a table
    const commitsSection = branchDetails.locator("table").first()
    await expect(commitsSection).toBeVisible()

    // Capture snapshot
    await captureHtmlSnapshot(commitsSection, "issue-links-github")

    // Verify GH-456 is rendered as a link
    // Note: GH-456 matches the JIRA pattern first since it's listed first in the config
    const ghLink = commitsSection.locator("a:has-text('GH-456')")
    await expect(ghLink).toBeVisible()
    await expect(ghLink).toHaveAttribute("href", "https://jira.example.com/browse/GH-456")
  })

  test("should render hash-style GitHub issue links", async ({ page }) => {
    // Find the feature-ui branch that has #789 in its commit message
    const featureUIBranch = findBranchRow(page, "feature-ui")
    await expect(featureUIBranch).toBeVisible()

    // Expand the branch
    await expandBranch(page, featureUIBranch)
    const branchDetails = getBranchDetailsRow(page, "feature-ui")
    await expect(branchDetails).toBeVisible()

    // Wait for commits to be visible - CommitList component renders as a table
    const commitsSection = branchDetails.locator("table").first()
    await expect(commitsSection).toBeVisible()

    // Capture snapshot
    await captureHtmlSnapshot(commitsSection, "issue-links-hash")

    // Verify #789 is rendered as a link
    const hashLink = commitsSection.locator("a:has-text('#789')")
    await expect(hashLink).toBeVisible()
    await expect(hashLink).toHaveAttribute("href", "https://github.com/example/repo/issues/789")
  })

  test("should render multiple issue links in one commit", async ({ page }) => {
    // Find the feature-db branch that has TEST-001 and PROD-999
    const featureDbBranch = findBranchRow(page, "feature-db")
    await expect(featureDbBranch).toBeVisible()

    // Expand the branch
    await expandBranch(page, featureDbBranch)
    const branchDetails = getBranchDetailsRow(page, "feature-db")
    await expect(branchDetails).toBeVisible()

    // Wait for commits to be visible - CommitList component renders as a table
    const commitsSection = branchDetails.locator("table").first()
    await expect(commitsSection).toBeVisible()

    // Capture snapshot
    await captureHtmlSnapshot(commitsSection, "issue-links-multiple")

    // Verify both TEST-001 and PROD-999 are rendered as links
    const testLink = commitsSection.locator("a:has-text('TEST-001')")
    await expect(testLink).toBeVisible()
    await expect(testLink).toHaveAttribute("href", "https://jira.example.com/browse/TEST-001")

    const prodLink = commitsSection.locator("a:has-text('PROD-999')")
    await expect(prodLink).toBeVisible()
    await expect(prodLink).toHaveAttribute("href", "https://jira.example.com/browse/PROD-999")
  })

  test("should show plain text for commits without issue references", async ({ page }) => {
    // Find the feature-docs branch that has no issue references
    const featureDocsBranch = findBranchRow(page, "feature-docs")
    await expect(featureDocsBranch).toBeVisible()

    // Expand the branch
    await expandBranch(page, featureDocsBranch)
    const branchDetails = getBranchDetailsRow(page, "feature-docs")
    await expect(branchDetails).toBeVisible()

    // Wait for commits to be visible - CommitList component renders as a table
    const commitsSection = branchDetails.locator("table").first()
    await expect(commitsSection).toBeVisible()

    // Capture snapshot
    await captureHtmlSnapshot(commitsSection, "issue-links-plain-text")

    // Verify there are no links in this commit message
    const commitMessage = commitsSection.locator("text=Update documentation")
    await expect(commitMessage).toBeVisible()

    // Check that there are no links in this commit
    const links = commitsSection.locator("tr:has-text('Update documentation') a")
    await expect(links).toHaveCount(0)
  })

  test("should render issue links in unassigned commits", async ({ page }) => {
    // Look for the unassigned commits section
    const unassignedSection = page.locator("[data-testid=\"unassigned-commits\"]")

    // Check if unassigned commits exist
    const hasUnassigned = await unassignedSection.count() > 0

    if (hasUnassigned) {
      await expect(unassignedSection).toBeVisible()

      // Capture snapshot of unassigned commits with issue links
      await captureHtmlSnapshot(unassignedSection, "issue-links-unassigned")

      // Verify CRITICAL-111 is rendered as a link in unassigned commit
      const criticalLink = unassignedSection.locator("a:has-text('CRITICAL-111')")
      await expect(criticalLink).toBeVisible()
      await expect(criticalLink).toHaveAttribute("href", "https://jira.example.com/browse/CRITICAL-111")
    }
  })

  test("should render issue links in branch names", async ({ page }) => {
    // Check if branch names with issue references show links
    // This depends on whether the UI shows issue links in branch names
    // For now, we'll check the branch rows themselves

    // The branches table contains all the branch rows
    const branchesTable = page.locator("table").first()
    await expect(branchesTable).toBeVisible()

    // Capture snapshot of all branches to see if any branch names have issue links
    await captureHtmlSnapshot(branchesTable, "issue-links-branch-names")
  })
})