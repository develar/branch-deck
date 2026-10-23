// @spec spec/repository/update-project.spec.md
import type { Locator, Page } from "@playwright/test"
import { expect, test } from "../fixtures/test-fixtures"
import { findBranchRow, verifyCommitCount } from "../helpers/branch-helpers"
import { readClipboard } from "../helpers/clipboard-helpers"
import { getDockCalls } from "../helpers/header-helpers"
import { dropAt, startDrag } from "../helpers/selection-helpers"
import { emitTauriEvent, runAndWaitForSync } from "../helpers/sync-helpers"
import { holdCommand, holdCommandCalls, holdWithProgress, recordCommand, routeCommandError } from "../helpers/wait-helpers"

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

  test("should rebase an update whose conflict of git resolves by words, and say so", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // `update_words`: the local commit capitalizes line 2, to which the upstream adds a word; git conflicts, the words resolve
    await setupRepo("update_words")
    await syncAndWaitForBranches()
    const button = updateButton(page)
    await expect(button).toHaveAttribute("data-status", "clean")
    await expectTooltip(page, "Rebases onto origin/master, resolving 1 conflict by words")

    await button.click()
    await expect(page.getByText("Updated master: rebased 2 commits onto origin/master")).toBeVisible()
    await expect(page.getByText("Resolved 1 conflict by words in “(feature-auth) Capitalize line 2”")).toBeVisible()
    await expect(button).toHaveAttribute("data-status", "upToDate")
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

  test("should show one progress from the click to the end of its sync, its steps in the tooltip", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    // the update stays in its fetch, then in its check, until released: the test reports its steps as the backend does
    const fetch = await holdWithProgress(page, "fetch_baseline")
    const update = await holdWithProgress(page, "update_project")
    const button = updateButton(page)
    const progress = page.getByTestId("update-progress")
    await button.click()
    await expect(button).toHaveAttribute("aria-busy", "true")
    await expect(button).toHaveText(/^Updating/)
    await expect(progress).toHaveAttribute("data-step", "fetch")

    // every step of a clean update, the running one first: never the state of the last check, nor "fetch and update"
    await page.mouse.move(0, 0)
    await button.hover()
    const steps = page.getByTestId("update-steps").getByRole("listitem")
    await expect(steps).toHaveText(["Fetching origin/master…", "Check the update", "Update to origin/master", "Sync branches"])
    await expect(page.getByTestId("update-project-tooltip")).not.toContainText("Rebases cleanly")
    await expect(page.getByTestId("update-project-tooltip")).not.toContainText("fetch and update")

    // git's percent moves the bar on
    await fetch.send({ percent: 20, transferred: "1.20 MiB", speed: null, line: "Receiving objects:  20% (2/10)" })
    await expect(steps.first()).toHaveText("Receiving objects:  20% (2/10)")
    // the button grew to its running width when the run started (its transition is over by now)
    const width = (await button.boundingBox())!.width
    const atTwenty = Number(await progress.getAttribute("aria-valuenow"))
    await fetch.send({ percent: 90, transferred: "5.79 MiB", speed: null, line: "Receiving objects:  90% (9/10)" })
    await expect.poll(async () => Number(await progress.getAttribute("aria-valuenow"))).toBeGreaterThan(atTwenty)
    // the Dock shows the update, the user may wait for it in another app
    await expect.poll(async () => (await getDockCalls(page)).at(-1)).toMatchObject({ progressBar: { status: "normal" } })

    // the fetch ends; the check runs before the repository lock
    fetch.release()
    await update.send({ phase: "checking", fetch: null })
    await expect(steps.nth(1)).toHaveText("Checking the update onto origin/master…")
    // another operation holds the repository: the wait for the move is told, not hidden in the check
    await update.send({ phase: "waiting", fetch: null })
    await expect(progress).toHaveAttribute("data-step", "waiting")
    await expect(steps).toHaveText([
      "Fetched origin/master · 5.79 MiB",
      "Checked the update onto origin/master",
      "Waiting for Branch Deck or bd to finish…",
      "Update to origin/master",
      "Sync branches",
    ])
    // one word for the whole run: the button keeps its width from step to step
    await expect.poll(async () => (await button.boundingBox())!.width).toBe(width)

    // the sync of the applied update is its last step: the toast says what happened at once, the bar goes on
    const releaseSync = await holdCommand(page, "sync_branches")
    update.release()
    await expect(page.getByText("Updated master: rebased 1 commit onto origin/master")).toBeVisible()
    await expect(progress).toHaveAttribute("data-step", "sync")
    await expect(button).toHaveAttribute("aria-busy", "true")
    await expect(page.getByTestId("sync-button")).toHaveAttribute("aria-busy", "true")
    await page.mouse.move(0, 0)
    await button.hover()
    await expect(steps.last()).toHaveText("Syncing branches…")

    releaseSync()
    await expect(button).toHaveText("Update")
    await expect(progress).toHaveCount(0)
    // no robot runs: the Dock has no progress again
    await expect.poll(async () => (await getDockCalls(page)).at(-1)).toEqual({ progressBar: { status: "none" } })
  })

  test("should let a rewrite run during the fetch of an update, which checks and applies after it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    const fetch = await holdCommandCalls(page, "fetch_baseline")
    const updates = await recordCommand(page, "update_project")
    const parks = await recordCommand(page, "park_branches")
    await updateButton(page).click()
    await expect.poll(() => fetch.calls.length).toBe(1)
    await expect(page.getByTestId("update-progress")).toHaveAttribute("data-step", "fetch")

    // the fetch holds no slot of the rewrites: a park during it runs at once, its toast before the update ends
    await startDrag(page, findBranchRow(page, "feature-auth"))
    await runAndWaitForSync(page, () => dropAt(page, page.getByTestId("drag-target-bar").getByTestId("park-drop-zone")))
    expect(parks).toHaveLength(1)
    await expect(page.locator("[data-slot=toast]", { hasText: "Parked feature-auth" })).toBeVisible()
    await expect(page.locator("[data-slot=toast]", { hasText: /^Cannot park/ })).toHaveCount(0)
    expect(updates).toHaveLength(0)

    // the update checks and applies once its fetch ended, without a fetch of its own
    fetch.release()
    await expect(page.getByText("Updated master: rebased 1 commit onto origin/master")).toBeVisible()
    expect(updates).toHaveLength(1)
    expect(updates[0]).toMatchObject({ fetch: false })
    expect(fetch.calls).toHaveLength(1)
  })

  test("should end the progress at the result when there is nothing to apply, its sync on Sync", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    await updateButton(page).click()
    await expect(page.getByText("Updated master: rebased 1 commit onto origin/master")).toBeVisible()
    await expect(updateButton(page)).toHaveText("Update")

    // up to date now: the tooltip still lists every step, the check decides what runs
    const fetch = await holdCommandCalls(page, "fetch_baseline")
    const update = await holdWithProgress(page, "update_project")
    await updateButton(page).click()
    await page.mouse.move(0, 0)
    await updateButton(page).hover()
    const steps = page.getByTestId("update-steps").getByRole("listitem")
    await expect(steps).toHaveText(["Fetching origin/master…", "Check the update", "Update to origin/master", "Sync branches"])
    fetch.release()
    await update.send({ phase: "checking", fetch: null })
    await expect(steps).toHaveText(["Fetched origin/master", "Checking the update onto origin/master…", "Update to origin/master", "Sync branches"])

    // the update ends with its check, the sync that follows it is Sync's
    const releaseSync = await holdCommand(page, "sync_branches")
    update.release()
    await expect(page.getByText("master is already up to date with origin/master")).toBeVisible()
    await expect(updateButton(page)).toHaveText("Update")
    await expect(updateButton(page)).not.toHaveAttribute("aria-busy")
    await expect(page.getByTestId("sync-button")).toHaveAttribute("aria-busy", "true")
    releaseSync()
  })

  test("should show the update state in the footer of the landing overview", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    await page.getByTestId("landing-indicator").hover()
    const line = page.getByTestId("landing-popover").getByTestId("update-state-line")
    await expect(line).toContainText("Incoming from origin/master: rebases cleanly")
    // the icon of the button: a clean update without landed commits is the check
    await expect(line.getByTestId("update-state-icon")).toBeVisible()
    await expect(line.getByTestId("update-state-icon")).not.toHaveAttribute("data-landing")

    await line.getByTestId("update-state-line-button").click()
    await expect(page.getByText("Updated master: rebased 1 commit onto origin/master")).toBeVisible()
  })

  test("should name the holder of the repository lock and retry the update without a fetch", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    // `bd push auth` holds the repository for the whole wait of the update, after its fetch; then it is done
    await routeCommandError(
      page,
      "update_project",
      {
        code: "locked",
        message: "`bd push auth` (pid 4242) is changing this repository; try again when it is done",
        details: { type: "locked", holder: { tool: "bd", command: "bd push auth", pid: 4242, held_for: 40 } },
        retry: "same",
      },
      { times: 1 },
    )
    const fetches = await recordCommand(page, "fetch_baseline")
    const updates = await recordCommand(page, "update_project")

    await updateButton(page).click()
    const toast = page.locator("[data-slot=toast]").filter({ hasText: "Cannot update the project" })
    await expect(toast).toContainText("`bd push auth` (pid 4242) has held the repository for 40 s")
    await expect(toast).not.toContainText("branch-deck.lock")
    expect(fetches).toHaveLength(1)

    await toast.getByRole("button", { name: "Retry Without Fetch" }).click()
    await expect(page.getByText("Updated master: rebased 1 commit onto origin/master")).toBeVisible()
    // the update checks and applies against the fetch it already did
    expect(updates.map(call => call.fetch)).toEqual([false, false])
    expect(fetches).toHaveLength(1)
  })

  test("should offer no retry when the branch moved under the update", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    await routeCommandError(page, "update_project", { code: "head_moved", message: "master moved during the update: nothing was changed", retry: "after_reread" })

    await updateButton(page).click()
    const toast = page.locator("[data-slot=toast]").filter({ hasText: "Cannot update the project" })
    await expect(toast).toContainText("master moved during the update: nothing was changed")
    // the toast is there, without a retry: the request is read again, not repeated
    await expect(toast.getByRole("button", { name: "Retry Without Fetch" })).toHaveCount(0)
  })

  test("should run from the Update Project menu item", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    await emitTauriEvent(page, "update_project")
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
    // the protocol is bd's own; each round names the upstream commits of its files
    expect(prompt).toContain("Run `bd help update` first and follow it")
    expect(prompt).not.toContain("Upstream commits of these files")
    expect(prompt).toMatch(/bd is `\/Users\/test\/\.local\/bin\/bd -C \S+`; below, `bd` stands for it\./)
    expect(prompt).toContain("1. `bd update --no-fetch`.")
    expect(prompt).toContain("editing only the copy at `edit`")
    expect(prompt).toContain("never edit a conflicting file in the working tree.")
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
