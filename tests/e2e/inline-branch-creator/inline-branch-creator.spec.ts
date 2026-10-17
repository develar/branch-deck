import { testWithUnassigned, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import {
  inlineBranchCreator,
  submitInlineForm,
  testInputAutoFocus,
  testEscapeKeyBehavior,
  testEscapeKeyWithButtonFocus,
  validateBranchNameError,
  openBranchCreatorForFirstCommit,
} from "../helpers/inline-form-helpers"
import { selectCommit, multiSelectCommit, clickGroupIntoBranchButton } from "../helpers/selection-helpers"

const test = testWithUnassigned

test.describe("Inline Branch Creator Feature", () => {
  test.beforeEach(async ({ page, setupUnassignedRepo }) => {
    await setupUnassignedRepo()

    // Verify we have 2 unassigned commits (Initial commit becomes origin/master)
    const unassignedCommits = page.locator("[data-row-id]")
    await expect(unassignedCommits).toHaveCount(2)
  })

  test("should create branch from selected commits", async ({ page }) => {
    // Select the first two commits
    const firstCommit = page.locator("[data-row-id]").first()
    const secondCommit = page.locator("[data-row-id]").nth(1)

    // Select first commit
    await selectCommit(firstCommit)

    // Multi-select second commit
    await multiSelectCommit(secondCommit)

    // Click the "Move to…" button
    await clickGroupIntoBranchButton(page)

    // Inline branch creator should appear
    await inlineBranchCreator.waitForVisible(page)

    // Verify the branch creator structure using HTML snapshot
    const branchCreatorPortal = inlineBranchCreator.getPortal(page)
    await captureHtmlSnapshot(branchCreatorPortal, "branch-creator-form")

    // The input should be focused and potentially have AI suggestion
    const input = inlineBranchCreator.getInput(page)
    await testInputAutoFocus(input)

    // Type a branch name and submit
    await submitInlineForm(input, "feature-authentication")

    // Branch creator should disappear after successful creation
    await inlineBranchCreator.waitForHidden(page)

    // Then wait for the new branch to appear
    // The branch name will include the prefix, so look for "authentication" part
    await page.waitForSelector("text=authentication")

    // Wait for unassigned commits section to be removed from DOM when it becomes empty
    await page.waitForSelector('[data-testid="unassigned-commits-section"]', { state: "detached" })
  })

  test("should validate branch name format", async ({ page }) => {
    await openBranchCreatorForFirstCommit(page)
    const input = inlineBranchCreator.getInput(page)

    // Try invalid branch name with special characters
    await validateBranchNameError(page, input, "invalid@branch#name")

    // Fix the branch name
    await input.fill("valid-branch-name")
    await input.press("Enter")

    // Should succeed
    await inlineBranchCreator.waitForHidden(page)
  })

  test("should cancel with Escape key", async ({ page }) => {
    await openBranchCreatorForFirstCommit(page)

    // Keep reference to first commit for later assertion
    const firstCommit = page.locator("[data-row-id]").first()

    const input = inlineBranchCreator.getInput(page)
    await testEscapeKeyBehavior(page, inlineBranchCreator.getPortal(page), input, "test-branch")

    // Selection should remain
    await expect(firstCommit).toHaveAttribute("data-selected", "true")
  })

  test("should cancel with Escape key even when button is focused", async ({ page }) => {
    await openBranchCreatorForFirstCommit(page)

    const input = inlineBranchCreator.getInput(page)
    const cancelButton = inlineBranchCreator.getCancelButton(page)
    await testEscapeKeyWithButtonFocus(page, inlineBranchCreator.getPortal(page), input, cancelButton, "test-branch")
  })

  test("should focus input when opened", async ({ page }) => {
    await openBranchCreatorForFirstCommit(page)

    // Input should be focused immediately
    const input = inlineBranchCreator.getInput(page)
    await testInputAutoFocus(input)

    // Should be able to type immediately
    await input.type("quick-type-test")
    await expect(input).toHaveValue("quick-type-test")
  })

  test("should open with Enter key when commits are selected", async ({ page }) => {
    // Select a commit
    const firstCommit = page.locator("[data-row-id]").first()
    await selectCommit(firstCommit)

    // Ensure the commit list container has focus for keyboard events
    const commitListContainer = page.locator('table[tabindex="0"]').first()
    await commitListContainer.focus()

    // Press Enter to open branch creator
    await page.keyboard.press("Enter")

    // Branch creator should appear
    await inlineBranchCreator.waitForVisible(page)

    // Input should be focused
    const input = inlineBranchCreator.getInput(page)
    await testInputAutoFocus(input)
  })

  test("should verify form structure", async ({ page }) => {
    await openBranchCreatorForFirstCommit(page)
    // the AI controls have their own spec (inline-branch-creator-ai)
    await captureHtmlSnapshot(inlineBranchCreator.getPortal(page), "branch-creator-no-ai")
  })
})
