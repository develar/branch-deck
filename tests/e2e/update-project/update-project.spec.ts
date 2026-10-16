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
    await expect(button).toHaveAttribute("data-status", "clean")
    await expect(button).toHaveText("Update")
    await expectTooltip(page, "Rebases cleanly onto origin/master")

    await button.click()
    await expect(page.getByText("Updated master: rebased 1 commit onto origin/master")).toBeVisible()
    await expect(button).toHaveAttribute("data-status", "upToDate")
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
    await expect(button).toHaveAttribute("data-status", "conflict")
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

    const copy = dialog.getByTestId("copy-agent-prompt")
    await copy.click()
    // the button says it in place, no toast
    await expect(copy).toHaveText("Copied")
    await expect(page.locator("[data-slot=toast]")).toHaveCount(0)
    await page.clock.fastForward(2000)
    await expect(copy).toHaveText("Copy Prompt for Agent")
    // the prompt sends the agent to the app's own bd (`agent_cli_command`): its resolve session, never the working tree
    await expect(dialog).toContainText("the prompt resolves each commit with bd, outside the working tree.")
    const prompt = await readClipboard(page)
    expect(prompt).toContain("Update the branch `master` of the Git repository at ")
    expect(prompt).toMatch(/- Likely conflict: src\/app.txt \(mine: [0-9a-f]{8}; upstream: [0-9a-f]{8}\)/)
    expect(prompt).toMatch(/Upstream commits of these files, newest first:\n- [0-9a-f]{8} Change line 2 upstream, Dana Lee\n/)
    expect(prompt).toMatch(/bd is `\/Users\/test\/\.local\/bin\/bd -C \S+`; below, `bd` stands for it\./)
    expect(prompt).toContain("1. `bd update --no-fetch`.")
    expect(prompt).toContain("Edit only the files at `edit`, never the same path in the working tree.")
    expect(prompt).toContain("Don't push.")
    expect(prompt).not.toContain("git stash push")

    await page.keyboard.press("Escape")
    await expect(dialog).toBeHidden()
    // nothing changed
    await verifyCommitCount(findBranchRow(page, "feature-auth"), "2 commits")
    await expect(button).toHaveAttribute("data-status", "conflict")
  })

  test("should list the first files of a wide conflict and count the rest", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // `update_conflict_wide`: 60 files that the local commit adds and each of 7 upstream commits changes
    await setupRepo("update_conflict_wide")
    await syncAndWaitForBranches()
    const button = updateButton(page)
    await expect(button).toHaveAttribute("data-status", "conflict")
    await expectTooltip(page, "Conflicts in 60 files: src/wide/file00.txt, src/wide/file01.txt, src/wide/file02.txt and 57 more")

    await button.click()
    const dialog = refusedDialog(page)
    await expect(dialog.getByRole("heading")).toHaveText("60 files conflict with origin/master")
    // the check reports the files it attributes commits to, and counts the rest
    await expect(dialog.getByTestId("update-conflict-file")).toHaveCount(50)
    await expect(dialog.getByTestId("update-conflict-more-files")).toHaveText("… and 10 more files")
    // the newest 5 of the 7 upstream commits of a file
    const file = dialog.getByTestId("update-conflict-file").first()
    await expect(file.locator("li[data-side=local]")).toHaveCount(1)
    await expect(file.locator("li[data-side=upstream]")).toHaveText([/round 2/, /round 3/, /round 4/, /round 5/, /round 6/])
    await expect(file.getByTestId("update-conflict-more-commits")).toHaveText("… and 2 more")

    await dialog.getByTestId("copy-agent-prompt").click()
    const prompt = await readClipboard(page)
    expect(prompt).toContain("- … and 40 more files: bd names the files of each round")
    expect(prompt.length).toBeLessThan(8000)
  })

  test("should explain uncommitted changes that block the update", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("update_blocked")
    await syncAndWaitForBranches()
    const button = updateButton(page)
    await expect(button).toHaveAttribute("data-status", "blocked")
    await expectTooltip(page, "Your uncommitted changes in src/app.txt overlap origin/master")

    await button.click()
    const dialog = refusedDialog(page)
    await expect(dialog.getByRole("heading")).toHaveText("Your uncommitted changes block the update")
    await expect(dialog.getByTestId("update-blocked-paths").locator("li")).toHaveAttribute("data-path", "src/app.txt")

    // the prompt sends the agent to the user, then to the app's own bd (`agent_cli_command`), never to a stash
    await expect(dialog).toContainText("the prompt asks you about each file, then updates with bd.")
    await dialog.getByTestId("copy-agent-prompt").click()
    const prompt = await readClipboard(page)
    expect(prompt).toContain("my uncommitted changes are in files that origin/master also changes:\n- src/app.txt\n")
    expect(prompt).toContain("Don't stash, reset, check out or commit these files. Ask me what to do with each file.")
    expect(prompt).toMatch(/run `\/Users\/test\/\.local\/bin\/bd -C \S+ update --no-fetch` and continue as its output says\./)
    expect(prompt).not.toContain("git stash")
  })

  test("should be muted when up to date", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("simple")
    await syncAndWaitForBranches()
    const button = updateButton(page)
    await expect(button).toHaveAttribute("data-status", "upToDate")
    await expect(button.getByTestId("update-state-icon")).toHaveCount(0)
    // the `simple` template never fetched: the tooltip doesn't claim a state nobody asked the remote for ("Up to date
    // with origin/master" after a fetch: the first test)
    await expectTooltip(page, /^origin\/master not fetched yet/)
  })
})
