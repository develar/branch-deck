import { test, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot, captureModalSnapshot } from "../helpers/aria-snapshot-helpers"
import { openModal } from "../helpers/modal-helpers"

test.describe("Welcome Card", () => {
  test("shows welcome card for new users without branch prefix", async ({ page, setupRepo }) => {
    await setupRepo("NO_REPO")

    // Verify the welcome card structure using HTML snapshot
    const welcomeCard = page.locator("[data-testid=\"welcome-card\"]")
    await expect(welcomeCard).toBeVisible()

    await captureHtmlSnapshot(welcomeCard, "welcome-card-new-user")
  })

  test("opens branch prefix help modal from welcome card", async ({ page, setupRepo }) => {
    await setupRepo("NO_REPO")

    // Wait for welcome card to be visible
    const welcomeCard = page.locator("[data-testid=\"welcome-card\"]")
    await expect(welcomeCard).toBeVisible()

    // Click the "View guide" button to open modal
    const viewGuideButton = page.getByRole("button", { name: "View guide" })
    await openModal(page, viewGuideButton)

    // Verify the modal structure using HTML snapshot
    await captureModalSnapshot(page, "branch-prefix-help-modal")
  })
})