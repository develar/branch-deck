import { test, expect } from "./fixtures/test-fixtures"
import { captureAriaSnapshot, captureModalSnapshot } from "./helpers/aria-snapshot-helpers"
import { openModal } from "./helpers/modal-helpers"

test.describe("Welcome Card", () => {
  test("shows welcome card for new users without branch prefix", async ({ page, setupRepo }) => {
    await setupRepo("NO_REPO")

    // Verify the welcome card structure using ARIA snapshot
    const welcomeCard = page.locator("[data-testid=\"welcome-card\"]")
    await expect(welcomeCard).toBeVisible()

    await captureAriaSnapshot(welcomeCard, "welcome-card-new-user")
  })

  test("opens branch prefix help modal from welcome card", async ({ page, setupRepo }) => {
    await setupRepo("NO_REPO")

    // Wait for welcome card to be visible
    const welcomeCard = page.locator("[data-testid=\"welcome-card\"]")
    await expect(welcomeCard).toBeVisible()

    // Click the "View guide" button to open modal
    const viewGuideButton = page.getByRole("button", { name: "View guide" })
    await openModal(page, viewGuideButton)

    // Verify the modal structure using ARIA snapshot
    await captureModalSnapshot(page, "branch-prefix-help-modal")
  })
})