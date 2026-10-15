import type { Page } from "@playwright/test"
import { testWithUnassigned, expect } from "../fixtures/test-fixtures"
import type { SetupRepoOptions } from "../helpers/setup-options"
import { selectCommit, clickGroupIntoBranchButton } from "../helpers/selection-helpers"
import { inlineBranchCreator, openBranchCreatorForFirstCommit } from "../helpers/inline-form-helpers"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { getOpenedUrls } from "../helpers/pull-request-helpers"
import {
  getAIStatusIcon,
  getSuggestionButtons,
  getGeneratingPlaceholders,
  getSuggestionHint,
  clickEnableAI,
  clickNotNow,
  expectAIEnabled,
  expectAIDisabled,
  getEnableButton,
  getNotNowButton,
} from "../helpers/ai-test-helpers"

const test = testWithUnassigned

const AI_ENABLED_SETUP: SetupRepoOptions = {
  initialStoreValues: {
    ai: { aiMode: "claude" },
  },
}

function suggestionErrorToast(page: Page) {
  return page.locator("[data-slot=toast]").filter({ hasText: "Cannot suggest branch names" })
}

function aiEnabledWith(branchNameSuggestions: NonNullable<SetupRepoOptions["branchNameSuggestions"]>): SetupRepoOptions {
  return { ...AI_ENABLED_SETUP, branchNameSuggestions }
}

test.describe("Inline Branch Creator - AI Features", () => {
  test("should ask before anything is sent to Claude", async ({ page, setupUnassignedRepo }) => {
    const repo = await setupUnassignedRepo()

    await openBranchCreatorForFirstCommit(page)

    await expect(getEnableButton(page)).toBeVisible()
    await expect(getNotNowButton(page)).toBeVisible()
    const suggestions = inlineBranchCreator.getPortal(page).getByTestId("branch-name-suggestions")
    await expect(suggestions.locator("text=Suggestions:")).toBeVisible()
    await expect(suggestions.getByTestId("ai-consent-note")).toHaveText(
      "Sends the commit messages and changed file names to Claude (Haiku) through your Claude Code",
    )
    await expect(getAIStatusIcon(page)).toBeVisible()
    expect(await repo.getBranchNameSuggestionCalls()).toBe(0)

    await captureHtmlSnapshot(inlineBranchCreator.getPortal(page), "inline-branch-creator-ai-initial-prompt")
  })

  test("should disable AI when clicking 'Not now' in initial prompt", async ({ page, setupUnassignedRepo }) => {
    const repo = await setupUnassignedRepo()

    await openBranchCreatorForFirstCommit(page)
    await clickNotNow(page)

    await expectAIDisabled(page)
    expect(await repo.getBranchNameSuggestionCalls()).toBe(0)

    await captureHtmlSnapshot(inlineBranchCreator.getPortal(page), "inline-branch-creator-ai-disabled")
  })

  test("should suggest names once AI is enabled", async ({ page, setupUnassignedRepo }) => {
    const repo = await setupUnassignedRepo()

    await openBranchCreatorForFirstCommit(page)
    await clickEnableAI(page)

    await expectAIEnabled(page)
    await expect(getSuggestionButtons(page)).toHaveCount(2)
    // the first one fills the empty input
    const first = await getSuggestionButtons(page).first().textContent()
    await expect(inlineBranchCreator.getInput(page)).toHaveValue(first ?? "")
    expect(await repo.getBranchNameSuggestionCalls()).toBe(1)
  })

  test("should ask again after the consent to the on-device model", async ({ page, setupUnassignedRepo }) => {
    // `enabled` was the answer for the on-device model that Claude Code replaced
    const repo = await setupUnassignedRepo({ initialStoreValues: { ai: { aiMode: "enabled" } } })

    await openBranchCreatorForFirstCommit(page)

    await expect(getEnableButton(page)).toBeVisible()
    await expect(getSuggestionButtons(page)).toHaveCount(0)
    expect(await repo.getBranchNameSuggestionCalls()).toBe(0)
  })

  test("should show placeholders while Claude answers", async ({ page, setupUnassignedRepo }) => {
    await setupUnassignedRepo(aiEnabledWith({ delayMs: 1500 }))

    await openBranchCreatorForFirstCommit(page)

    await expect(getGeneratingPlaceholders(page)).toHaveCount(2)
    await expect(getGeneratingPlaceholders(page).first()).toBeDisabled()
    await expect(getSuggestionButtons(page)).toHaveCount(2)
    await expect(getGeneratingPlaceholders(page)).toHaveCount(0)
  })

  test("should offer to install Claude Code when it is missing", async ({ page, setupUnassignedRepo }) => {
    await setupUnassignedRepo(aiEnabledWith({ outcome: { status: "claudeMissing" } }))

    await openBranchCreatorForFirstCommit(page)

    await expect(getSuggestionHint(page)).toHaveText("Install Claude Code to get suggestions")
    await expect(getSuggestionButtons(page)).toHaveCount(0)
    // a quiet hint, not an error
    await expect(suggestionErrorToast(page)).toHaveCount(0)
    await expectAIEnabled(page)

    await captureHtmlSnapshot(inlineBranchCreator.getPortal(page), "inline-branch-creator-ai-claude-missing")

    await inlineBranchCreator.getPortal(page).getByRole("button", { name: "Install Claude Code" }).click()
    await expect.poll(() => getOpenedUrls(page)).toEqual(["https://code.claude.com/docs/en/setup"])
  })

  test("should say how to log in to Claude Code", async ({ page, setupUnassignedRepo }) => {
    await setupUnassignedRepo(aiEnabledWith({ outcome: { status: "notLoggedIn" } }))

    await openBranchCreatorForFirstCommit(page)

    await expect(getSuggestionHint(page)).toHaveText("Log in to Claude Code to get suggestions: run claude, then /login")
    await expect(suggestionErrorToast(page)).toHaveCount(0)
  })

  test("should report a failure of Claude", async ({ page, setupUnassignedRepo }) => {
    await setupUnassignedRepo(aiEnabledWith({ outcome: { status: "error", message: "API Error: 529 Overloaded" } }))

    await openBranchCreatorForFirstCommit(page)

    const toast = suggestionErrorToast(page)
    await expect(toast).toContainText("API Error: 529 Overloaded")
    await expect(getAIStatusIcon(page)).toHaveClass(/text-destructive/)

    await getAIStatusIcon(page).hover()
    await expect(page.locator("[data-slot=popover-content]")).toContainText("AI suggestions failed")
  })

  test("should toggle AI on/off via sparkles icon", async ({ page, setupUnassignedRepo }) => {
    await setupUnassignedRepo(AI_ENABLED_SETUP)

    await openBranchCreatorForFirstCommit(page)

    const aiStatusIcon = getAIStatusIcon(page)
    await expect(aiStatusIcon).toHaveClass(/text-primary/)
    const suggestionButtons = getSuggestionButtons(page)
    await expect(suggestionButtons).toHaveCount(2)
    await captureHtmlSnapshot(inlineBranchCreator.getPortal(page), "inline-branch-creator-ai-enabled")

    await aiStatusIcon.click()
    await expect(aiStatusIcon).toHaveClass(/text-muted/)
    await expect(suggestionButtons).toHaveCount(0)
    await captureHtmlSnapshot(inlineBranchCreator.getPortal(page), "inline-branch-creator-ai-disabled-after-toggle")

    await aiStatusIcon.click()
    await expect(aiStatusIcon).toHaveClass(/text-primary/)
    await expect(suggestionButtons).toHaveCount(2)
    await captureHtmlSnapshot(inlineBranchCreator.getPortal(page), "inline-branch-creator-ai-reenabled")
  })

  test("should say in the help popover what the AI button does and what it sends", async ({ page, setupUnassignedRepo }) => {
    await setupUnassignedRepo(AI_ENABLED_SETUP)

    await openBranchCreatorForFirstCommit(page)
    await expectAIEnabled(page)

    await getAIStatusIcon(page).hover()
    const help = page.locator("[data-slot=popover-content]")
    await expect(help).toContainText("AI suggestions are on. The button turns them off.")
    await expect(help).toContainText("the commit messages and changed file names go to Anthropic. No diffs or file contents are sent.")
    await expect(help).not.toContainText(/click/i)
  })

  test("should populate branch name input with selected suggestion", async ({ page, setupUnassignedRepo }) => {
    await setupUnassignedRepo(AI_ENABLED_SETUP)

    await openBranchCreatorForFirstCommit(page)

    const suggestionButtons = getSuggestionButtons(page)
    await expect(suggestionButtons).toHaveCount(2)
    await captureHtmlSnapshot(inlineBranchCreator.getPortal(page), "inline-branch-creator-ai-with-suggestions")

    const input = inlineBranchCreator.getInput(page)
    await expect(input).toHaveValue(/^fix-/)
    // the chip of the name in the input is the chosen one
    await expect(suggestionButtons.first()).toHaveAttribute("aria-pressed", "true")
    await expect(suggestionButtons.nth(1)).toHaveAttribute("aria-pressed", "false")

    await suggestionButtons.nth(1).click()
    await expect(input).toHaveValue(/^update-/)
    await expect(suggestionButtons.nth(1)).toHaveAttribute("aria-pressed", "true")
    await expect(suggestionButtons.first()).toHaveAttribute("aria-pressed", "false")
  })

  test("should regenerate suggestions when commit selection changes", async ({ page, setupUnassignedRepo }) => {
    const repo = await setupUnassignedRepo(AI_ENABLED_SETUP)

    await openBranchCreatorForFirstCommit(page)
    const suggestionButtons = getSuggestionButtons(page)
    await expect(suggestionButtons).toHaveCount(2)
    const firstSuggestionText = await suggestionButtons.first().textContent()

    await inlineBranchCreator.getCancelButton(page).click()
    await inlineBranchCreator.waitForHidden(page)

    await selectCommit(page, page.locator("[data-row-id]").nth(1))
    await clickGroupIntoBranchButton(page)
    await inlineBranchCreator.waitForVisible(page)

    // the test server names them after the words of the commit messages
    await expect(suggestionButtons).toHaveCount(2)
    await expect(suggestionButtons.first()).not.toHaveText(firstSuggestionText ?? "")
    expect(await repo.getBranchNameSuggestionCalls()).toBe(2)
  })

  test("should persist AI state across form open/close", async ({ page, setupUnassignedRepo }) => {
    await setupUnassignedRepo()

    await openBranchCreatorForFirstCommit(page)
    await clickNotNow(page)
    await expectAIDisabled(page)

    await inlineBranchCreator.getCancelButton(page).click()
    await inlineBranchCreator.waitForHidden(page)
    await clickGroupIntoBranchButton(page)
    await inlineBranchCreator.waitForVisible(page)

    await expectAIDisabled(page)
    await expect(getEnableButton(page)).not.toBeVisible()
    await expect(getNotNowButton(page)).not.toBeVisible()
  })
})
