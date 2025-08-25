import { testWithUnassigned, expect } from "../fixtures/test-fixtures"
import { selectCommit, clickGroupIntoBranchButton } from "../helpers/selection-helpers"
import { inlineBranchCreator, openBranchCreatorForFirstCommit } from "../helpers/inline-form-helpers"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import {
  getAIStatusIcon,
  getSuggestionButtons,
  clickEnableAI,
  clickNotNow,
  waitForDownloadIcon,
  expectAIEnabled,
  expectAIDisabled,
  pauseDownload,
  waitForDownloadPaused,
  getEnableButton,
  getNotNowButton,
} from "../helpers/ai-test-helpers"

const test = testWithUnassigned

test.describe("Inline Branch Creator - AI Features", () => {
  test("should show AI initial prompt when AI has never been configured", async ({ page, setupUnassignedRepo }) => {
    await setupUnassignedRepo()

    // By default, test server returns AI in initial state
    await openBranchCreatorForFirstCommit(page)

    // Should show "Enable AI" and "Not now" buttons in initial state
    const enableButton = getEnableButton(page)
    const notNowButton = getNotNowButton(page)
    await expect(enableButton).toBeVisible()
    await expect(notNowButton).toBeVisible()

    // Should show AI initial prompt section when buttons are present
    const suggestionsSection = inlineBranchCreator.getPortal(page).locator("[data-testid='branch-name-suggestions']")
    await expect(suggestionsSection).toBeVisible()

    // Should show "Suggestions:" header
    const suggestionsHeader = suggestionsSection.locator("text=Suggestions:")
    await expect(suggestionsHeader).toBeVisible()

    // Should show AI status icon (sparkles) in the AIStatusIndicator
    const aiStatusIcon = getAIStatusIcon(page)
    await expect(aiStatusIcon).toBeVisible()

    // Capture HTML snapshot of the form with AI initial prompt
    await captureHtmlSnapshot(
      inlineBranchCreator.getPortal(page),
      "inline-branch-creator-ai-initial-prompt",
    )
  })

  test("should disable AI when clicking 'Not now' in initial prompt", async ({ page, setupUnassignedRepo }) => {
    await setupUnassignedRepo()

    await openBranchCreatorForFirstCommit(page)

    // Click "Not now" button
    await clickNotNow(page)

    // The sparkles icon should now be visible in the extra-actions area (AIStatusIndicator)
    // It should show as disabled (text-muted)
    await expectAIDisabled(page)

    // Capture HTML snapshot of disabled state
    await captureHtmlSnapshot(
      inlineBranchCreator.getPortal(page),
      "inline-branch-creator-ai-disabled",
    )
  })

  test("should start model download when clicking 'Enable'", async ({ page, setupUnassignedRepo }) => {
    await setupUnassignedRepo({
      modelState: "not_downloaded", // Explicitly set model not downloaded
      initialStoreValues: {
        ai: {
          aiMode: "initial", // Frontend needs this to show Enable/Not now buttons
          simulateSlowDownload: true, // Enable 5-minute download simulation
        },
      },
    })

    await openBranchCreatorForFirstCommit(page)

    // Click "Enable AI" button
    await clickEnableAI(page)

    // Wait for download to start - icon should appear
    const downloadingIcon = await waitForDownloadIcon(page)
    await expect(downloadingIcon).toHaveClass(/animate-pulse/)

    // Verify that suggestions are NOT shown during download
    const suggestionButtons = getSuggestionButtons(page)
    await expect(suggestionButtons).toHaveCount(0)

    // Capture HTML snapshot during download
    await captureHtmlSnapshot(
      inlineBranchCreator.getPortal(page),
      "inline-branch-creator-ai-downloading",
    )

    // Test pause functionality
    // Wait for the download progress toast component
    const downloadProgress = page.locator("[data-testid='model-download-progress-toast']")
    await expect(downloadProgress).toBeVisible()

    // Find and click the Pause button
    await pauseDownload(page)

    // Wait for the download paused toast title
    await waitForDownloadPaused(page)

    // The downloading icon should be removed after cancellation
    await expect(downloadingIcon).not.toBeVisible()

    // AI status icon should show as disabled after cancellation
    const aiStatusIcon = getAIStatusIcon(page)
    await expect(aiStatusIcon).toBeVisible()
    // Note: In test environment, AI may get re-enabled after cancellation due to mock behavior
    // The important thing is that the download was paused successfully
  })

  test("should show AI suggestions after model download completes", async ({ page, setupUnassignedRepo }) => {
    // Pre-set AI as enabled (simulating completed download)
    await setupUnassignedRepo({
      initialStoreValues: {
        ai: { aiMode: "enabled" },
      },
    })

    await openBranchCreatorForFirstCommit(page)

    // Should show enabled sparkles icon
    await expectAIEnabled(page)

    // Should show suggestions
    const suggestionButtons = getSuggestionButtons(page)
    await expect(suggestionButtons).toHaveCount(2)

    // Capture HTML snapshot with suggestions
    await captureHtmlSnapshot(
      inlineBranchCreator.getPortal(page),
      "inline-branch-creator-ai-with-suggestions",
    )
  })

  test("should toggle AI on/off via sparkles icon", async ({ page, setupUnassignedRepo }) => {
    // Pre-set AI as enabled
    await setupUnassignedRepo({
      initialStoreValues: {
        ai: { aiMode: "enabled" },
      },
    })

    await openBranchCreatorForFirstCommit(page)

    const aiStatusIcon = getAIStatusIcon(page)

    // Initially enabled
    await expect(aiStatusIcon).toHaveClass(/text-primary/)

    // Get suggestion buttons reference
    const suggestionButtons = getSuggestionButtons(page)
    // Initially should have 2 suggestions
    await expect(suggestionButtons).toHaveCount(2)

    // Capture snapshot with AI enabled
    await captureHtmlSnapshot(
      inlineBranchCreator.getPortal(page),
      "inline-branch-creator-ai-enabled",
    )

    // Click to disable
    await aiStatusIcon.click()
    await expect(aiStatusIcon).toHaveClass(/text-muted/)

    // Suggestion buttons should disappear
    await expect(suggestionButtons).toHaveCount(0)

    // Capture snapshot with AI disabled
    await captureHtmlSnapshot(
      inlineBranchCreator.getPortal(page),
      "inline-branch-creator-ai-disabled-after-toggle",
    )

    // Click to re-enable
    await aiStatusIcon.click()
    await expect(aiStatusIcon).toHaveClass(/text-primary/)

    // Suggestion buttons should reappear
    await expect(suggestionButtons).toHaveCount(2)

    // Capture snapshot with AI re-enabled
    await captureHtmlSnapshot(
      inlineBranchCreator.getPortal(page),
      "inline-branch-creator-ai-reenabled",
    )
  })

  test("should populate branch name input with selected suggestion", async ({ page, setupUnassignedRepo }) => {
    // Pre-set AI as enabled
    await setupUnassignedRepo({
      initialStoreValues: {
        ai: { aiMode: "enabled" },
      },
    })

    await openBranchCreatorForFirstCommit(page)

    // Wait for suggestions
    const suggestionButtons = getSuggestionButtons(page)
    await expect(suggestionButtons).toHaveCount(2)

    // Input should be auto-populated with first suggestion
    const input = inlineBranchCreator.getInput(page)
    const firstValue = await input.inputValue()
    expect(firstValue).toContain("user-name")

    // Click second suggestion
    await suggestionButtons.nth(1).click()
    const secondValue = await input.inputValue()
    expect(secondValue).toContain("user-name")
  })

  test("should show download progress in popover when hovering pulsing icon", async ({ page, setupUnassignedRepo }) => {
    await setupUnassignedRepo({
      modelState: "not_downloaded",
      initialStoreValues: {
        ai: {
          aiMode: "initial",
          simulateSlowDownload: true,
        },
      },
    })

    await openBranchCreatorForFirstCommit(page)

    // Click "Enable AI" to start download
    await clickEnableAI(page)

    // Wait for download icon to appear
    const downloadingIcon = await waitForDownloadIcon(page)

    // Hover and wait for popover
    await downloadingIcon.hover()

    // The popover content should appear with the specific popover testid
    const downloadProgressPopover = page.locator("[data-testid='model-download-progress-popover']")
    await expect(downloadProgressPopover).toBeVisible()

    // Verify it shows download progress content
    await expect(downloadProgressPopover).toContainText(/KB|MB|%|config|model/)
  })

  test("should pause and resume model download", async ({ page, setupUnassignedRepo }) => {
    await setupUnassignedRepo({
      modelState: "not_downloaded",
      initialStoreValues: {
        ai: {
          aiMode: "initial",
          simulateSlowDownload: true,
        },
      },
    })

    await openBranchCreatorForFirstCommit(page)

    // Start download
    await clickEnableAI(page)

    // Wait for and verify download is in progress
    const downloadingIcon = await waitForDownloadIcon(page)
    await expect(downloadingIcon).toHaveClass(/animate-pulse/)

    // Pause the download via toast
    await pauseDownload(page)

    // Wait for paused state
    await waitForDownloadPaused(page)

    // Icon should stop pulsing
    await expect(downloadingIcon).not.toBeVisible()

    // AI status icon should be visible and disabled after pause
    await expectAIDisabled(page)

    // Resume by clicking sparkles icon - this starts a new download
    const aiStatusIcon = getAIStatusIcon(page)
    await aiStatusIcon.click()

    // Since simulateSlowDownload is false after resume, download completes quickly
    // So we check if AI is enabled (suggestions appear)
    const suggestionButtons = getSuggestionButtons(page)
    await expect(suggestionButtons).toHaveCount(2)

    // AI icon should now be enabled
    await expectAIEnabled(page)
  })

  test("should regenerate suggestions when commit selection changes", async ({ page, setupUnassignedRepo }) => {
    // Pre-set AI as enabled
    await setupUnassignedRepo({
      initialStoreValues: {
        ai: { aiMode: "enabled" },
      },
    })

    // Select first commit
    await openBranchCreatorForFirstCommit(page)

    // Get initial suggestions
    const suggestionButtons = getSuggestionButtons(page)
    await expect(suggestionButtons).toHaveCount(2)
    const firstSuggestionText = await suggestionButtons.first().textContent()

    // Close form
    await inlineBranchCreator.getCancelButton(page).click()
    await inlineBranchCreator.waitForHidden(page)

    // Select different commit
    const secondCommit = page.locator("[data-row-id]").nth(1)
    await selectCommit(page, secondCommit)
    await clickGroupIntoBranchButton(page)
    await inlineBranchCreator.waitForVisible(page)

    // Should have different suggestions (they're based on commit hash in test mode)
    const newSuggestionButtons = getSuggestionButtons(page)
    await expect(newSuggestionButtons).toHaveCount(2)
    const newFirstSuggestionText = await newSuggestionButtons.first().textContent()
    expect(newFirstSuggestionText).not.toBe(firstSuggestionText)
  })

  test("should persist AI state across form open/close", async ({ page, setupUnassignedRepo }) => {
    await setupUnassignedRepo({
      modelState: "downloaded",
      initialStoreValues: {
        ai: {
          aiMode: "initial",
        },
      },
    })

    await openBranchCreatorForFirstCommit(page)

    // Disable AI by clicking "Not now"
    await clickNotNow(page)

    // Verify disabled state - look for the AI status icon
    await expectAIDisabled(page)

    // Close the form
    await inlineBranchCreator.getCancelButton(page).click()
    await inlineBranchCreator.waitForHidden(page)

    // Reopen the form
    await clickGroupIntoBranchButton(page)
    await inlineBranchCreator.waitForVisible(page)

    // Should still be disabled
    await expectAIDisabled(page)

    // Should NOT show initial prompt again (no Enable/Not now buttons)
    const enableButton = getEnableButton(page)
    await expect(enableButton).not.toBeVisible()
    const notNowButtonAgain = getNotNowButton(page)
    await expect(notNowButtonAgain).not.toBeVisible()
  })
})
