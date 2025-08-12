import { test, expect } from "./fixtures/test-fixtures"
import { waitForBranchSyncComplete } from "./helpers/sync-helpers"
import { openContextMenu, clickContextMenuItem } from "./helpers/selection-helpers"
import { inlineIssueReference, submitInlineForm, testInputAutoFocus, testEscapeKeyBehavior, testEscapeKeyWithButtonFocus, validateFormError } from "./helpers/inline-form-helpers"
import { waitForApiResponse, waitForProcessingComplete } from "./helpers/wait-helpers"
import { findBranchRow, isBranchProcessing } from "./helpers/branch-helpers"

test.describe("Issue Reference Feature", () => {
  test.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    // Setup test repository with simple template
    await setupRepo("simple")

    // Sync branches and wait for them to load
    await syncAndWaitForBranches()
  })

  test("should add issue reference to branch via context menu", async ({ page }) => {
    // Find the branch row
    const branchRow = findBranchRow(page, "test-branch")

    // Wait for the branch to finish syncing
    await waitForBranchSyncComplete(page, "test-branch")

    // Right-click to open context menu
    await openContextMenu(page, branchRow)

    // Wait for the menu item to appear - it only shows if commits are loaded
    await page.waitForSelector("text=Add Issue Reference")

    // Click the menu item
    await clickContextMenuItem(page, "Add Issue Reference")

    // The inline input should appear
    await inlineIssueReference.waitForVisible(page)

    // The input should be focused
    const input = inlineIssueReference.getInput(page)
    await testInputAutoFocus(input)

    // Type an issue reference and submit
    await submitInlineForm(input, "GH-123")

    // The inline input should disappear immediately after submission
    await inlineIssueReference.waitForHidden(page)

    // Now wait for the API call to complete (happens after form is hidden)
    await waitForApiResponse(page, "add_issue_reference_to_commits")

    // Wait for processing to complete if needed
    await waitForProcessingComplete(page, branchRow)

    // Verify the issue reference was added (would need to check commit messages)
    // This would be verified by checking that commits now have the prefix
  })

  test("should validate issue reference format", async ({ page }) => {
    // Open inline input
    const branchRow = findBranchRow(page, "test-branch")
    await openContextMenu(page, branchRow)
    await clickContextMenuItem(page, "Add Issue Reference")

    await inlineIssueReference.waitForVisible(page)
    const input = inlineIssueReference.getInput(page)
    await testInputAutoFocus(input)

    // Try to submit empty input - it should not submit
    await input.press("Enter")
    // The input should still be visible since submit was rejected
    await expect(inlineIssueReference.getForm(page)).toBeVisible()

    // Try invalid format
    await validateFormError(
      page,
      inlineIssueReference.getForm(page),
      input,
      "invalid!@#",
      "Issue reference must be in format like ABC-123",
    )

    // Valid format should work
    await submitInlineForm(input, "JIRA-456")

    await inlineIssueReference.waitForHidden(page)
  })

  test("should cancel with Escape key", async ({ page }) => {
    // Open inline input
    const branchRow = findBranchRow(page, "test-branch")
    await openContextMenu(page, branchRow)
    await clickContextMenuItem(page, "Add Issue Reference")

    await inlineIssueReference.waitForVisible(page)

    const input = inlineIssueReference.getInput(page)
    await testEscapeKeyBehavior(page, inlineIssueReference.getForm(page), input, "TEST-789")

    // Branch row should not show processing state
    const isProcessing = await isBranchProcessing(branchRow)
    expect(isProcessing).toBe(false)
  })

  test("should cancel with Escape key even when button is focused", async ({ page }) => {
    // Open inline input
    const branchRow = findBranchRow(page, "test-branch")
    await openContextMenu(page, branchRow)
    await clickContextMenuItem(page, "Add Issue Reference")

    await inlineIssueReference.waitForVisible(page)

    const input = inlineIssueReference.getInput(page)
    const cancelButton = inlineIssueReference.getCancelButton(page)
    await testEscapeKeyWithButtonFocus(page, inlineIssueReference.getForm(page), input, cancelButton, "TEST-123")
  })

  test("should focus input when opened", async ({ page }) => {
    // This tests the auto-focus behavior that was fixed
    const branchRow = page.locator("[data-testid=\"branch-row\"]").first()
    await openContextMenu(page, branchRow)
    await clickContextMenuItem(page, "Add Issue Reference")

    // Input should be focused immediately
    await inlineIssueReference.waitForVisible(page)
    const input = inlineIssueReference.getInput(page)
    await testInputAutoFocus(input)

    // Should be able to type immediately
    await input.type("QUICK-TYPE")
    await expect(input).toHaveValue("QUICK-TYPE")
  })
})
