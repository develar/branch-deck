// @spec spec/repository/update-project.spec.md
import type { Locator, Page } from "@playwright/test"
import { expect, test } from "../fixtures/test-fixtures"
import { findBranchRow, verifyCommitCount } from "../helpers/branch-helpers"
import { readClipboard } from "../helpers/clipboard-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"

// The `behind_upstream`, `update_conflict` and `update_blocked` templates: `master` tracks `origin/master` of an upstream
// at `.git/upstream.git` that changed line 2 of src/app.txt and added docs/guide.md; already fetched.

function updateButton(page: Page): Locator {
  return page.getByTestId("update-project-button")
}

function refusedDialog(page: Page): Locator {
  return page.getByTestId("update-blocked-dialog")
}

async function expectTooltip(page: Page, text: string | RegExp): Promise<void> {
  await updateButton(page).hover()
  await expect(page.getByTestId("update-project-tooltip")).toContainText(text)
  await page.mouse.move(0, 0)
}

test.describe("Update Project", () => {
  test("should show a clean update, apply it and then be up to date", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    const button = updateButton(page)
    await expect(button).toHaveAttribute("data-state", "clean")
    await expect(button).toHaveText("Update")
    await expectTooltip(page, "Rebases cleanly onto origin/master")

    await button.click()
    await expect(page.getByText("Updated master: rebased 1 commit onto origin/master")).toBeVisible()
    await expect(button).toHaveAttribute("data-state", "upToDate")
    await expectTooltip(page, /Up to date with origin\/master.*Fetched just now/)
    await verifyCommitCount(findBranchRow(page, "feature-auth"), "1 commit")
  })

  test("should show the update state in the footer of the outgoing overview", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    await page.getByTestId("outgoing-indicator").hover()
    const line = page.getByTestId("outgoing-popover").getByTestId("update-state-line")
    await expect(line).toContainText("Incoming from origin/master: rebases cleanly")

    await line.getByTestId("update-state-line-button").click()
    await expect(page.getByText("Updated master: rebased 1 commit onto origin/master")).toBeVisible()
  })

  test("should run from the Update Project menu item", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    await emitTauriEvent(page, "update-project")
    await expect(page.getByText("Updated master: rebased 1 commit onto origin/master")).toBeVisible()
  })

  test("should explain a conflict and copy a prompt for an agent", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("update_conflict")
    await syncAndWaitForBranches()
    const button = updateButton(page)
    await expect(button).toHaveAttribute("data-state", "conflict")
    await expect(button.getByTestId("update-state-icon")).toBeVisible()
    await expectTooltip(page, "Conflicts in 1 file: src/app.txt")

    await button.click()
    const dialog = refusedDialog(page)
    await expect(dialog).toBeVisible()
    await expect(dialog.getByRole("heading")).toHaveText("Your commits conflict with origin/master")
    await expect(dialog.getByTestId("update-conflict-files").locator("li")).toHaveText(["src/app.txt"])
    await expect(dialog.getByTestId("update-conflict-commits").locator("li")).toHaveText([/Add login/, /Change line 2/])

    await dialog.getByTestId("copy-agent-prompt").click()
    await expect(page.getByText("Prompt copied")).toBeVisible()
    const prompt = await readClipboard(page)
    expect(prompt).toContain("Rebase the branch `master` of the Git repository at ")
    expect(prompt).toContain("- src/app.txt")
    expect(prompt).toMatch(/- [0-9a-f]{8} \(feature-auth\) Add login\n- [0-9a-f]{8} \(feature-auth\) Change line 2/)
    expect(prompt).toContain("Don't push.")

    await page.keyboard.press("Escape")
    await expect(dialog).toBeHidden()
    // nothing changed
    await verifyCommitCount(findBranchRow(page, "feature-auth"), "2 commits")
    await expect(button).toHaveAttribute("data-state", "conflict")
  })

  test("should explain uncommitted changes that block the update", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("update_blocked")
    await syncAndWaitForBranches()
    const button = updateButton(page)
    await expect(button).toHaveAttribute("data-state", "blocked")
    await expectTooltip(page, "Your uncommitted changes in src/app.txt overlap origin/master")

    await button.click()
    const dialog = refusedDialog(page)
    await expect(dialog.getByRole("heading")).toHaveText("Your uncommitted changes block the update")
    await expect(dialog.getByTestId("update-blocked-paths").locator("li")).toHaveText(["src/app.txt"])

    await dialog.getByTestId("copy-agent-prompt").click()
    const prompt = await readClipboard(page)
    expect(prompt).toContain("My uncommitted changes that overlap the upstream:\n- src/app.txt")
    expect(prompt).toContain("`git stash push --include-untracked`")
  })

  test("should be muted when up to date", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("simple")
    await syncAndWaitForBranches()
    const button = updateButton(page)
    await expect(button).toHaveAttribute("data-state", "upToDate")
    await expect(button.getByTestId("update-state-icon")).toHaveCount(0)
    await expectTooltip(page, "Up to date with origin/master")
  })
})
