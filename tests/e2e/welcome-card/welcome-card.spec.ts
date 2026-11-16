import { test, expect } from "../fixtures/test-fixtures"
import { captureHtmlSnapshot, captureModalSnapshot } from "../helpers/aria-snapshot-helpers"
import { openModal } from "../helpers/modal-helpers"
import { pressAndExpectSettingsOpened } from "../helpers/settings-helpers"
import { runAndWaitForAnswer } from "../helpers/sync-helpers"
import { TestRepositoryBuilder } from "../helpers/test-repository"
import { useTitleBar } from "../helpers/title-bar-helpers"
import { expectClosed, routeCommandError } from "../helpers/wait-helpers"

test.describe("Welcome Card", () => {
  test("shows the welcome card for new users, which asks for the repository first", async ({ page, setupRepo }) => {
    await setupRepo("NO_REPO", {
      prepopulateStore: false, // Don't auto-populate store so welcome card shows
    })

    // Verify the welcome card structure using HTML snapshot
    const welcomeCard = page.locator('[data-testid="welcome-card"]')
    await expect(welcomeCard).toBeVisible()
    // the session of the repository says whether it has a prefix: the prefix step comes with a repository without one
    await expect(welcomeCard.getByTestId("welcome-step-number")).toHaveText(["1"])
    await expect(welcomeCard.getByRole("button", { name: "Select a Git Repository…" })).toBeVisible()
    await expect(welcomeCard.getByRole("textbox", { name: "Branch prefix to save" })).toHaveCount(0)

    await captureHtmlSnapshot(welcomeCard, "welcome-card-new-user")
  })

  for (const activation of ["click", "Enter", "Space"] as const) {
    test(`selects a repository from the welcome step by ${activation}`, { tag: "@webkit" }, async ({ page, setupRepo }) => {
      await setupRepo("simple", { prepopulateStore: false })
      const step = page.getByRole("button", { name: "Select a Git Repository…" })
      await expect(step).toHaveAccessibleDescription("Browse to a local Git repository.")
      if (activation === "click") {
        // The description is part of the target, as the title and the number are.
        await step.getByText("Browse to a local Git repository.").click()
      } else {
        await step.focus()
        await expect(step).toBeFocused()
        await step.press(activation)
      }
      await expect(page.getByTestId("ready-to-sync-card")).toBeVisible()
      await expect(page.getByTestId("welcome-card")).toHaveCount(0)
    })
  }

  test("keeps the welcome step and its focus when the folder dialog is canceled", { tag: "@webkit" }, async ({ page, setupRepo }) => {
    const repo = await setupRepo("NO_REPO", { prepopulateStore: false })
    await repo.setDialogPick(null)
    const step = page.getByRole("button", { name: "Select a Git Repository…" })
    await step.focus()
    await expect(step).toBeFocused()
    await runAndWaitForAnswer(page, "browse_repository", () => step.press("Enter"))
    await expect(step).toBeVisible()
    await expect(step).toBeFocused()
    await expect(page.getByRole("combobox")).toContainText("Select repository…")
    await expect(page.getByTestId("error-alert")).toHaveCount(0)
  })

  test("reports a non-git folder selected from the welcome step", async ({ page, setupRepo }) => {
    await setupRepo("empty-non-git", { prepopulateStore: false })
    await page.getByRole("button", { name: "Select a Git Repository…" }).click()
    await expect(page.getByTestId("error-alert")).toContainText("Not a Git repository")
    await expect(page.getByTestId("welcome-card")).toHaveCount(0)
  })

  test("reports a folder dialog failure from the welcome step", async ({ page, setupRepo }) => {
    await setupRepo("NO_REPO", { prepopulateStore: false })
    await routeCommandError(page, "browse_repository", { code: "internal", message: "Folder dialog unavailable" })
    await page.getByRole("button", { name: "Select a Git Repository…" }).click()
    await expect(page.getByTestId("error-alert")).toContainText("Cannot browse for a repository")
    await expect(page.getByTestId("error-alert")).toContainText("Folder dialog unavailable")
    await expect(page.getByTestId("welcome-card")).toHaveCount(0)
  })

  test("directs users to the repository selector when the host has no folder dialog", async ({ page, setupRepo }) => {
    await useTitleBar(page, { kind: "native", checkForUpdates: false, folderDialog: false })
    await setupRepo("NO_REPO", { prepopulateStore: false })
    const card = page.getByTestId("welcome-card")
    await expect(card.getByRole("heading", { name: "Select a Git repository" })).toBeVisible()
    await expect(card.getByText("Choose a repository in the repository selector above.")).toBeVisible()
    await expect(card.getByRole("button", { name: "Select a Git Repository…" })).toHaveCount(0)
    await expect(page.getByTestId("browse-repository-button")).toHaveCount(0)
  })

  test("opens branch prefix help modal from welcome card", async ({ page, setupRepo }) => {
    // a repository without a prefix: the welcome card asks for one
    await setupRepo("simple_no_prefix", { prepopulateStore: true })

    // Wait for welcome card to be visible
    const welcomeCard = page.locator('[data-testid="welcome-card"]')
    await expect(welcomeCard).toBeVisible()

    // Click the "View guide" button to open modal
    const viewGuideButton = page.getByRole("button", { name: "View guide" })
    await openModal(page, viewGuideButton)

    // Verify the modal structure using HTML snapshot
    await captureModalSnapshot(page, "branch-prefix-help-modal")
  })

  test("opens Settings at the field of the prefix from the guide, and at the one of the repository", async ({ page, setupRepo }) => {
    await setupRepo("simple_no_prefix", { prepopulateStore: true })
    await page.getByRole("button", { name: "View guide" }).click()
    const guide = page.getByTestId("branch-prefix-help-dialog")
    await pressAndExpectSettingsOpened(page, () => guide.getByRole("button", { name: "Open Settings" }).click(), { page: "branches", focus: "branchPrefix" })
    await expectClosed(guide)

    // the prefix of one repository: its page in Settings
    await page.getByRole("button", { name: "View guide" }).click()
    await pressAndExpectSettingsOpened(
      page,
      () => guide.getByRole("button", { name: "Open Repository Settings" }).click(),
      { page: "branches", scope: "repository", focus: "branchPrefix" },
      2,
    )
    await expectClosed(guide)
  })

  test("shows the prefix step as step 1 when the repository has no branch prefix configured", async ({ page, setupRepo }) => {
    await setupRepo("simple_no_prefix", { prepopulateStore: true })

    const welcomeCard = page.locator('[data-testid="welcome-card"]')
    await expect(welcomeCard).toBeVisible()

    // Should show the prefix step (check for the heading specifically), numbered 1: the steps shown count from 1
    await expect(welcomeCard.getByRole("heading", { name: "Choose your branch prefix" })).toBeVisible()
    await expect(welcomeCard.getByTestId("welcome-step-number")).toHaveText(["1"])

    // Should NOT show Step 1 content
    await expect(welcomeCard.getByRole("button", { name: "Select a Git Repository…" })).toHaveCount(0)

    // the suggestion of the repository's git identity (test@example.com) pre-fills the field
    await expect(welcomeCard.getByRole("textbox", { name: "Branch prefix to save" })).toHaveValue("test")

    // Capture HTML snapshot for regression safety
    await captureHtmlSnapshot(welcomeCard, "welcome-card-repo-no-prefix")
  })

  test("keeps Step 2 visible after selecting a repository without branch prefix", async ({ page, setupRepo }) => {
    const welcome = await setupRepo("NO_REPO", { prepopulateStore: false })
    // the repository the user picks in the folder dialog: one without a branch prefix
    const picked = new TestRepositoryBuilder().useTemplate("simple_no_prefix").withPrepopulateStore(false)
    await picked.init()
    try {
      await welcome.setDialogPick(picked.path)

      // Welcome card should be visible initially
      const welcomeCard = page.locator('[data-testid="welcome-card"]')
      await expect(welcomeCard).toBeVisible()

      await welcomeCard.getByRole("button", { name: "Select a Git Repository…" }).click()

      // After repository selection, Step 2 should still be present if no prefix is configured
      // Re-query the card to avoid staleness if DOM re-rendered
      const welcomeCardAfter = page.locator('[data-testid="welcome-card"]')
      await expect(welcomeCardAfter).toBeVisible()
      await expect(welcomeCardAfter.getByRole("heading", { name: "Choose your branch prefix" })).toBeVisible()

      // Step 1 should no longer be shown inside the welcome card (avoid matching tooltip text elsewhere)
      await expect(welcomeCardAfter.getByRole("button", { name: "Select a Git Repository…" })).toHaveCount(0)

      // the field shows the suggestion of the selected repository's git identity (test@example.com)
      await expect(welcomeCardAfter.getByRole("textbox", { name: "Branch prefix to save" })).toHaveValue("test")

      // Capture HTML snapshot for regression safety
      await captureHtmlSnapshot(welcomeCardAfter, "welcome-card-repo-selected-no-prefix")
    } finally {
      // the page first, so it starts no command for the picked repository
      await page.close()
      await picked.cleanup()
    }
  })
})
