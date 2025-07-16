import { test, expect } from "./fixtures/base-test"
import { TestRepositoryBuilder } from "./helpers/test-repository"

test.describe("Issue Reference Feature", () => {
  let repoBuilder: TestRepositoryBuilder

  test.beforeEach(async ({ page }) => {
    // Create a test repository using the simple template
    repoBuilder = new TestRepositoryBuilder()
      .useTemplate("simple")
    await repoBuilder.init()

    // Open browser console to see debug logs
    page.on("console", (msg) => {
      console.log(`[Browser]`, msg.type(), msg.text())
    })

    // Navigate to the app with the test repository path and ID in URL
    await page.goto(`/?testRepo=${encodeURIComponent(repoBuilder.path)}&repoId=${repoBuilder.id}`)
    await page.waitForLoadState("networkidle")

    // Wait for branch prefix to be loaded from git config
    const branchPrefixInput = page.locator("input[placeholder=\"Enter branch prefix...\"]")
    await expect(branchPrefixInput).toHaveValue("user-name")

    // Click sync button to load branches
    const syncButton = page.locator("button:has-text(\"Sync Virtual Branches\")")

    // Wait for the button to be enabled
    await expect(syncButton).toBeEnabled()

    await syncButton.click()

    // Wait for branches to load
    await page.waitForSelector("[data-testid=\"branch-row\"]")
  })

  test.afterEach(async () => {
    await repoBuilder.cleanup()
  })

  test("should add issue reference to branch via context menu", async ({ page }) => {
    // Find the branch row
    const branchRow = page.locator("[data-testid=\"branch-row\"]", {
      hasText: "test-branch",
    })

    // Wait for the branch to finish syncing (status badge appears instead of progress bar)
    const statusCell = branchRow.locator("td").nth(2) // Third cell is status
    await expect(statusCell.locator("[role=\"progressbar\"]")).not.toBeVisible()

    // Now wait for the status badge to appear
    const statusBadge = statusCell.locator(".lowercase")
    await expect(statusBadge).toBeVisible()
    const statusText = await statusBadge.textContent()
    console.log("Branch status text:", statusText)

    // Right-click to open context menu
    await branchRow.click({ button: "right" })

    // Wait for context menu to appear and click the menu item
    const addIssueMenuItem = page.getByRole("menuitem", { name: "Add Issue Reference" })
    await addIssueMenuItem.waitFor({ state: "visible" })

    await addIssueMenuItem.click()

    // The inline input should appear
    const inlineInput = page.locator("[data-testid=\"inline-issue-input\"]")
    await expect(inlineInput).toBeVisible()

    // The input should be focused
    const input = inlineInput.locator("input[type=\"text\"]")
    await expect(input).toBeFocused()

    // Type an issue reference
    await input.fill("GH-123")

    // Submit with Enter
    await input.press("Enter")

    // The inline input should disappear immediately after submission
    await expect(inlineInput).not.toBeVisible()

    // Now wait for the API call to complete (happens after form is hidden)
    await page.waitForResponse(response =>
      response.url().includes("add_issue_reference_to_commits") && response.status() === 200,
    )

    // The branch should either show processing state OR already be done
    // (processing might be very fast in tests)
    const branchRowClasses = await branchRow.getAttribute("class")
    const isProcessing = branchRowClasses?.includes("animate-pulse")

    if (isProcessing) {
      // Wait for processing to complete - the animate-pulse class should be removed
      await expect(branchRow).not.toHaveClass(/animate-pulse/)
    }

    // Verify the issue reference was added (would need to check commit messages)
    // This would be verified by checking that commits now have the prefix
  })

  test("should validate issue reference format", async ({ page }) => {
    // Open inline input
    const branchRow = page.locator("[data-testid=\"branch-row\"]", {
      hasText: "test-branch",
    })
    await branchRow.click({ button: "right" })
    const addIssueMenuItem = page.getByRole("menuitem", { name: "Add Issue Reference" })
    await addIssueMenuItem.waitFor({ state: "visible" })
    await addIssueMenuItem.click()

    const inlineInput = page.locator("[data-testid=\"inline-issue-input\"]")
    const input = inlineInput.locator("input")
    await expect(input).toBeFocused()

    // Try to submit empty input - it should not submit
    await input.press("Enter")
    // The input should still be visible since submit was rejected
    await expect(inlineInput).toBeVisible()

    // Try invalid format
    await input.fill("invalid!@#")
    await input.press("Enter")
    // Should show error message for invalid format
    const errorMessage = page.locator("[data-testid=\"inline-issue-input\"] .text-error")
    await expect(errorMessage).toContainText("Issue reference must be in format like ABC-123")

    // Valid format should work
    await input.fill("JIRA-456")
    await input.press("Enter")

    await expect(inlineInput).not.toBeVisible()
  })

  test("should cancel with Escape key", async ({ page }) => {
    // Open inline input
    const branchRow = page.locator("[data-testid=\"branch-row\"]", {
      hasText: "test-branch",
    })
    await branchRow.click({ button: "right" })
    const addIssueMenuItem = page.getByRole("menuitem", { name: "Add Issue Reference" })
    await addIssueMenuItem.waitFor({ state: "visible" })
    await addIssueMenuItem.click()

    const inlineInput = page.locator("[data-testid=\"inline-issue-input\"]")
    await expect(inlineInput).toBeVisible()

    const input = inlineInput.locator("input")
    await input.fill("TEST-789")

    // Press Escape to cancel
    await input.press("Escape")

    // Input should disappear without submitting
    await expect(inlineInput).not.toBeVisible()

    // Branch row should not show processing state
    const branchRowClasses = await branchRow.getAttribute("class")
    expect(branchRowClasses).not.toContain("animate-pulse")
  })

  test("should cancel with Escape key even when button is focused", async ({ page }) => {
    // Open inline input
    const branchRow = page.locator("[data-testid=\"branch-row\"]", {
      hasText: "test-branch",
    })
    await branchRow.click({ button: "right" })
    const addIssueMenuItem = page.getByRole("menuitem", { name: "Add Issue Reference" })
    await addIssueMenuItem.waitFor({ state: "visible" })
    await addIssueMenuItem.click()

    const inlineInput = page.locator("[data-testid=\"inline-issue-input\"]")
    await expect(inlineInput).toBeVisible()

    const input = inlineInput.locator("input")
    await input.fill("TEST-123")

    // Focus on the Cancel button
    const cancelButton = inlineInput.locator("button:has-text('Cancel')")
    await cancelButton.focus()
    await expect(cancelButton).toBeFocused()

    // Press Escape while button is focused
    await page.keyboard.press("Escape")

    // Input should disappear
    await expect(inlineInput).not.toBeVisible()
  })

  test("should focus input when opened", async ({ page }) => {
    // This tests the auto-focus behavior that was fixed
    const branchRow = page.locator("[data-testid=\"branch-row\"]").first()
    await branchRow.click({ button: "right" })
    const addIssueMenuItem = page.getByRole("menuitem", { name: "Add Issue Reference" })
    await addIssueMenuItem.waitFor({ state: "visible" })
    await addIssueMenuItem.click()

    // Input should be focused immediately
    const input = page.locator("[data-testid=\"inline-issue-input\"] input")
    await expect(input).toBeFocused()

    // Should be able to type immediately
    await page.keyboard.type("QUICK-TYPE")
    await expect(input).toHaveValue("QUICK-TYPE")
  })
})