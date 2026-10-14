// @spec spec/repository/update-project.spec.md
import type { Locator, Page } from "@playwright/test"
import { expect, test } from "../fixtures/test-fixtures"
import { findBranchRow, verifyCommitCount } from "../helpers/branch-helpers"
import { readClipboard } from "../helpers/clipboard-helpers"
import { emitTauriEvent } from "../helpers/sync-helpers"
import { holdCommand } from "../helpers/wait-helpers"

// The `behind_upstream`, `update_conflict` and `update_blocked` templates: `master` tracks `origin/master` of an upstream
// at `.git/upstream.git` that changed line 2 of src/app.txt (by Dana Lee, authored 2024-01-01, landed 2024-01-02) and
// added docs/guide.md; already fetched.

function updateButton(page: Page): Locator {
  return page.getByTestId("update-project-button")
}

function refusedDialog(page: Page): Locator {
  return page.getByTestId("update-blocked-dialog")
}

async function expectTooltip(page: Page, text: string | RegExp): Promise<void> {
  // a click on the button closed its tooltip, which opens again on the next hover (the button stays in place)
  await page.mouse.move(0, 0)
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

  test("should age the last sync next to the Update button while it is shown", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // the fetch time of the templates is the real one (FETCH_HEAD), not the page's clock: the last sync shows the tick
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    const lastSynced = page.getByText(/^Last synced /)
    await expect(lastSynced).toHaveText("Last synced just now")

    // nothing but the clock moves: no sync
    await page.clock.fastForward("01:15")
    await expect(lastSynced).toHaveText("Last synced 1 minute ago")
  })

  test("should name the running step in the tooltip, not the last state", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    // hold the update in its fetch: the mock forwards no progress, as before git reports the first stage
    const releaseUpdate = await holdCommand(page, "update_project")
    const button = updateButton(page)
    await button.click()
    await expect(button).toHaveText("Fetching…")
    // the click closed the tooltip; it opens again on the next hover, and the disabled button stays hoverable
    await page.mouse.move(0, 0)
    await button.hover()
    const tooltip = page.getByTestId("update-project-tooltip")
    await expect(tooltip).toContainText("Fetching origin/master…")
    await expect(tooltip).not.toContainText("Rebases cleanly")
    await expect(tooltip).not.toContainText("fetch and update")

    releaseUpdate()
    await expect(page.getByText("Updated master: rebased 1 commit onto origin/master")).toBeVisible()
    await expect(button).toHaveText("Update")
  })

  test("should show the update state in the footer of the outgoing overview", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    await page.getByTestId("outgoing-indicator").hover()
    const line = page.getByTestId("outgoing-popover").getByTestId("update-state-line")
    await expect(line).toContainText("Incoming from origin/master: rebases cleanly")
    // the icon of the button: a clean update without landed commits is the check
    await expect(line.getByTestId("update-state-icon")).toBeVisible()
    await expect(line.getByTestId("update-state-icon")).not.toHaveAttribute("data-landing")

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
    await expect(dialog.getByRole("heading")).toHaveText("1 file conflicts with origin/master")
    // each file with the commits of both sides that change it
    const file = dialog.getByTestId("update-conflict-file")
    await expect(file).toHaveAttribute("data-path", "src/app.txt")
    // yours with the author time; the upstream one landed 15 hours after it was written: both times (the page clock is
    // 2024-01-02 10:00 UTC)
    await expect(file.locator("li[data-side=local]")).toHaveText([/^yours[0-9a-f]{8}feature-auth Change line 219 hours ago$/])
    await expect(file.locator("li[data-side=upstream]")).toHaveText([/^origin\/master[0-9a-f]{8}Change line 2 upstreamDana Lee · authored 19 hours ago · landed 4 hours ago$/])
    // every commit to rebase, collapsed
    const commits = dialog.getByTestId("update-conflict-commits")
    await expect(commits).toBeHidden()
    await dialog.getByRole("button", { name: "All 2 commits to rebase, oldest first" }).click()
    await expect(commits.locator("li")).toHaveText([/Add login/, /Change line 2/])
    await expect(commits.locator("li").nth(1).getByLabel("Changes a likely conflicting file")).toBeVisible()

    await dialog.getByTestId("copy-agent-prompt").click()
    await expect(page.getByText("Prompt copied")).toBeVisible()
    const prompt = await readClipboard(page)
    expect(prompt).toContain("Rebase the branch `master` of the Git repository at ")
    expect(prompt).toMatch(/- src\/app.txt\n {2}- changed by my [0-9a-f]{8}\n {2}- changed upstream by [0-9a-f]{8} Change line 2 upstream \(Dana Lee, authored 2024-01-01, in origin\/master since 2024-01-02\)/)
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
    await expect(dialog.getByTestId("update-blocked-paths").locator("li")).toHaveAttribute("data-path", "src/app.txt")

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
    // the `simple` template never fetched: the tooltip doesn't claim a state nobody asked the remote for ("Up to date
    // with origin/master" after a fetch: the first test)
    await expectTooltip(page, /^origin\/master not fetched yet/)
  })
})
