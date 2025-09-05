import { test, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot, captureModalSnapshot } from "../helpers/aria-snapshot-helpers"
import { openModal } from "../helpers/modal-helpers"

test.describe("Welcome Card", () => {
  test("shows welcome card for new users without branch prefix", async ({ page, setupRepo }) => {
    await setupRepo("NO_REPO", {
      prepopulateStore: false, // Don't auto-populate store so welcome card shows
    })

    // Verify the welcome card structure using HTML snapshot
    const welcomeCard = page.locator("[data-testid=\"welcome-card\"]")
    await expect(welcomeCard).toBeVisible()

    await captureHtmlSnapshot(welcomeCard, "welcome-card-new-user")
  })

  test("opens branch prefix help modal from welcome card", async ({ page, setupRepo }) => {
    await setupRepo("NO_REPO", {
      prepopulateStore: false, // Don't auto-populate store so welcome card shows
    })

    // Wait for welcome card to be visible
    const welcomeCard = page.locator("[data-testid=\"welcome-card\"]")
    await expect(welcomeCard).toBeVisible()

    // Click the "View guide" button to open modal
    const viewGuideButton = page.getByRole("button", { name: "View guide" })
    await openModal(page, viewGuideButton)

    // Verify the modal structure using HTML snapshot
    await captureModalSnapshot(page, "branch-prefix-help-modal")
  })

  test("shows Step 2 when repository has no branch prefix configured", async ({ page, setupRepo }) => {
    await setupRepo("simple_no_prefix", { prepopulateStore: true })

    const welcomeCard = page.locator("[data-testid=\"welcome-card\"]")
    await expect(welcomeCard).toBeVisible()

    // Should show Step 2 content (check for the heading specifically)
    await expect(welcomeCard.getByRole("heading", { name: "Configure your branch prefix" })).toBeVisible()

    // Should NOT show Step 1 content
    await expect(welcomeCard.getByText("Select a Git repository")).toHaveCount(0)

    // Capture HTML snapshot for regression safety
    await captureHtmlSnapshot(welcomeCard, "welcome-card-repo-no-prefix")
  })

  test("keeps Step 2 visible after selecting a repository without branch prefix", async ({ page, setupRepo }) => {
    await setupRepo("NO_REPO", { prepopulateStore: false })

    // Welcome card should be visible initially
    const welcomeCard = page.locator("[data-testid=\"welcome-card\"]")
    await expect(welcomeCard).toBeVisible()

    // Select a repository via the browse button
    const browseButton = page.getByTestId("browse-repository-button")
    await browseButton.click()

    // After repository selection, Step 2 should still be present if no prefix is configured
    // Re-query the card to avoid staleness if DOM re-rendered
    const welcomeCardAfter = page.locator("[data-testid=\"welcome-card\"]")
    await expect(welcomeCardAfter).toBeVisible()
    await expect(welcomeCardAfter.getByText("Configure your branch prefix")).toBeVisible()

    // Step 1 should no longer be shown inside the welcome card (avoid matching tooltip text elsewhere)
    await expect(welcomeCardAfter.getByText("Select a Git repository")).toHaveCount(0)

    // Capture HTML snapshot for regression safety
    await captureHtmlSnapshot(welcomeCardAfter, "welcome-card-repo-selected-no-prefix")
  })
})
