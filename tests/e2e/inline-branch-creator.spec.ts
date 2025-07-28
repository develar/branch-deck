import { testWithUnassigned, expect } from "./fixtures/test-fixtures"
import { selectCommit, multiSelectCommit, clickGroupIntoBranchButton } from "./helpers/selection-helpers"
import { inlineBranchCreator, submitInlineForm, testInputAutoFocus, testEscapeKeyBehavior, testEscapeKeyWithButtonFocus, validateBranchNameError } from "./helpers/inline-form-helpers"
import { captureAriaSnapshot } from "./helpers/aria-snapshot-helpers"
import { waitForNavigationComplete } from "./helpers/wait-helpers"

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
    await selectCommit(page, firstCommit)

    // Multi-select second commit
    await multiSelectCommit(page, secondCommit)

    // Click "Group into Branch" button
    await clickGroupIntoBranchButton(page)

    // Inline branch creator should appear
    await inlineBranchCreator.waitForVisible(page)

    // Verify the branch creator structure using ARIA snapshot
    const branchCreatorPortal = inlineBranchCreator.getPortal(page)
    await captureAriaSnapshot(branchCreatorPortal, "branch-creator-form")

    // The input should be focused and potentially have AI suggestion
    const input = inlineBranchCreator.getInput(page)
    await testInputAutoFocus(input)

    // Type a branch name and submit
    await submitInlineForm(input, "feature-authentication")

    // Wait for branch creation to complete
    await page.waitForTimeout(500)

    // Branch creator should disappear after successful creation
    await inlineBranchCreator.waitForHidden(page)

    // After creating a branch, the page will refresh
    await waitForNavigationComplete(page)

    // Then wait for the new branch to appear
    // The branch name will include the prefix, so look for "authentication" part
    await page.waitForSelector("text=authentication", { timeout: 10000 })
  })

  test("should validate branch name format", async ({ page }) => {
    // Select a commit
    const firstCommit = page.locator("[data-row-id]").first()
    await selectCommit(page, firstCommit)

    // Open branch creator
    await clickGroupIntoBranchButton(page)

    await inlineBranchCreator.waitForVisible(page)
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
    // Select a commit
    const firstCommit = page.locator("[data-row-id]").first()
    await selectCommit(page, firstCommit)

    // Open branch creator
    await clickGroupIntoBranchButton(page)

    await inlineBranchCreator.waitForVisible(page)

    const input = inlineBranchCreator.getInput(page)
    await testEscapeKeyBehavior(page, inlineBranchCreator.getPortal(page), input, "test-branch")

    // Selection should remain
    await expect(firstCommit).toHaveAttribute("data-selected", "true")
  })

  test("should cancel with Escape key even when button is focused", async ({ page }) => {
    // Select a commit
    const firstCommit = page.locator("[data-row-id]").first()
    await selectCommit(page, firstCommit)

    // Open branch creator
    await clickGroupIntoBranchButton(page)

    await inlineBranchCreator.waitForVisible(page)

    const input = inlineBranchCreator.getInput(page)
    const cancelButton = inlineBranchCreator.getCancelButton(page)
    await testEscapeKeyWithButtonFocus(page, inlineBranchCreator.getPortal(page), input, cancelButton, "test-branch")
  })

  test("should focus input when opened", async ({ page }) => {
    // Select a commit
    const firstCommit = page.locator("[data-row-id]").first()
    await selectCommit(page, firstCommit)

    // Open branch creator
    await clickGroupIntoBranchButton(page)

    // Input should be focused immediately
    await inlineBranchCreator.waitForVisible(page)
    const input = inlineBranchCreator.getInput(page)
    await testInputAutoFocus(input)

    // Should be able to type immediately
    await page.keyboard.type("quick-type-test")
    await expect(input).toHaveValue("quick-type-test")
  })

  test("should open with Enter key when commits are selected", async ({ page }) => {
    // Select a commit
    const firstCommit = page.locator("[data-row-id]").first()
    await selectCommit(page, firstCommit)

    // Ensure the commit list container has focus for keyboard events
    const commitListContainer = page.locator("[tabindex=\"0\"]").first()
    await commitListContainer.focus()

    // Press Enter to open branch creator
    await page.keyboard.press("Enter")

    // Branch creator should appear
    await inlineBranchCreator.waitForVisible(page)

    // Input should be focused
    const input = inlineBranchCreator.getInput(page)
    await testInputAutoFocus(input)
  })

  test("should show AI toggle and verify form structure", async ({ page }) => {
    // Select a commit
    const firstCommit = page.locator("[data-row-id]").first()
    await selectCommit(page, firstCommit)

    // Open branch creator
    await clickGroupIntoBranchButton(page)

    await inlineBranchCreator.waitForVisible(page)

    // Verify the branch creator structure with ARIA snapshot
    const branchCreatorPortal = inlineBranchCreator.getPortal(page)

    // Check if AI is available by looking for the AI button
    const aiToggle = inlineBranchCreator.getAiToggle(page)
    const isAiAvailable = await aiToggle.isVisible()

    if (isAiAvailable) {
      // With AI available
      await captureAriaSnapshot(branchCreatorPortal, "branch-creator-with-ai")

      // Toggle AI on
      await aiToggle.click()

      // Check if pressed state changed
      const isPressed = await aiToggle.getAttribute("aria-pressed")
      if (isPressed === "true") {
        await captureAriaSnapshot(branchCreatorPortal, "branch-creator-ai-enabled")
      }
    }
    else {
      // Without AI
      await captureAriaSnapshot(branchCreatorPortal, "branch-creator-no-ai")
    }
  })
})