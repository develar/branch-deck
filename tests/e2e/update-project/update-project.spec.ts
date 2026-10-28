// @spec spec/repository/update-project.spec.md
import type { Locator, Page } from "@playwright/test"
import { expect, test } from "../fixtures/test-fixtures"
import { findBranchRow, getMoreButton, verifyCommitCount } from "../helpers/branch-helpers"
import { readClipboard } from "../helpers/clipboard-helpers"
import { getDockCalls } from "../helpers/header-helpers"
import { dropAt, startDrag } from "../helpers/selection-helpers"
import { emitTauriEvent, runAndWaitForSync, runAndWaitForUpdate } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { animationsEnded, holdIntent, recordCommand, recordRewrites, routeCommandError } from "../helpers/wait-helpers"

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

/** Presses Update and waits until the update waits (behind a rewrite, or for the repository lock): its steps, hovered */
async function updateWaiting(page: Page): Promise<Locator> {
  await updateButton(page).click()
  await expect(page.getByTestId("update-progress")).toHaveAttribute("data-step", "waiting")
  await page.mouse.move(0, 0)
  await updateButton(page).hover()
  return page.getByTestId("update-steps")
}

/** Drops feature-auth onto the park zone, which waits in the session: the hover of the "⋯" of its busy row */
async function parkWaiting(page: Page): Promise<Locator> {
  await startDrag(page, findBranchRow(page, "feature-auth"))
  await dropAt(page, page.getByTestId("drag-target-bar").getByTestId("park-drop-zone"))
  const more = getMoreButton(findBranchRow(page, "feature-auth"))
  await expect(more).toHaveAttribute("aria-busy", "true")
  await more.hover()
  return page.getByRole("tooltip")
}

/** The agent releases the repository lock: the update and the park waiting in the session both run, none refused */
async function expectUpdatedAndParked(page: Page, repo: TestRepositoryBuilder): Promise<void> {
  await runAndWaitForUpdate(page, () => repo.holdRepositoryLock(false))
  await expect(page.getByText("Updated master: rebased 1 commit onto origin/master")).toBeVisible()
  await expect(page.locator("[data-slot=toast]", { hasText: "Parked feature-auth" })).toBeVisible()
  await expect(page.getByText(/^Cannot /)).toHaveCount(0)
}

test.describe("Update Project", () => {
  test("should show a clean update, apply it and then be up to date", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    const button = updateButton(page)
    await expect(button).toHaveAttribute("data-status", "clean")
    await expect(button).toHaveText("Update")
    await expectTooltip(page, "Rebases cleanly onto origin/master")

    await runAndWaitForUpdate(page, () => button.click())
    await expect(page.getByText("Updated master: rebased 1 commit onto origin/master")).toBeVisible()
    await expect(button).toHaveAttribute("data-status", "upToDate")
    await expectTooltip(page, /Up to date with origin\/master.*Fetched just now/)
    await verifyCommitCount(findBranchRow(page, "feature-auth"), 1)
  })

  test("should rebase an update whose conflict of git resolves by words, and say so", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // `update_words`: the local commit capitalizes line 2, to which the upstream adds a word; git conflicts, the words resolve
    await setupRepo("update_words")
    await syncAndWaitForBranches()
    const button = updateButton(page)
    await expect(button).toHaveAttribute("data-status", "clean")
    await expectTooltip(page, "Rebases onto origin/master, resolving 1 conflict by words")

    await runAndWaitForUpdate(page, () => button.click())
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
    const repo = await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    // the update stays in the page until released: the test reports the steps of its operation as the session does
    const update = await holdIntent(page, "updateProject")
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
    await update.send({ kind: "progress", op: "held", percent: 20, transferred: "1.20 MiB", speed: null, line: "Receiving objects:  20% (2/10)" })
    await expect(steps.first()).toHaveText("Receiving objects:  20% (2/10)")
    // the button grew to its running width when the run started: measured once its transition ended, which under load
    // can still run its last frames
    await animationsEnded(button)
    const width = (await button.boundingBox())!.width
    const atTwenty = Number(await progress.getAttribute("aria-valuenow"))
    await update.send({ kind: "progress", op: "held", percent: 90, transferred: "5.79 MiB", speed: null, line: "Receiving objects:  90% (9/10)" })
    await expect.poll(async () => Number(await progress.getAttribute("aria-valuenow"))).toBeGreaterThan(atTwenty)
    // the Dock shows the update, the user may wait for it in another app
    await expect.poll(async () => (await getDockCalls(page)).at(-1)).toMatchObject({ progressBar: { status: "normal" } })

    // the fetch ends; the check runs before the repository lock, which another holder has: the wait is told, not hidden
    // in the check
    await update.send({ kind: "step", op: "held", step: { kind: "checking" } })
    await expect(steps.nth(1)).toHaveText("Checking the update onto origin/master…")
    const holder = { tool: "bd", command: "bd pick --follow", pid: 4242, held_for: 3 }
    await update.send({ kind: "step", op: "held", step: { kind: "waitingForLock", holder } })
    await expect(progress).toHaveAttribute("data-step", "waiting")
    await expect(steps).toHaveText([
      "Fetched origin/master · 5.79 MiB",
      "Checked the update onto origin/master",
      "Waiting for `bd pick --follow` (pid 4242, for 3 s)…",
      "Update to origin/master",
      "Sync branches",
    ])
    await expect(page.getByTestId("stop-waiting")).toBeVisible()
    // one word for the whole run: the button keeps its width from step to step
    await expect.poll(async () => (await button.boundingBox())!.width).toBe(width)

    // the sync of the applied update is its last step, held in the session: the toast says what happened at once, the
    // bar goes on
    await repo.holdRemoteAnswers({ hold: ["sync"] })
    update.release()
    await expect(page.getByText("Updated master: rebased 1 commit onto origin/master")).toBeVisible()
    await expect(progress).toHaveAttribute("data-step", "sync")
    await expect(button).toHaveAttribute("aria-busy", "true")
    await expect(page.getByTestId("sync-button")).toHaveAttribute("aria-busy", "true")
    await page.mouse.move(0, 0)
    await button.hover()
    await expect(steps.last()).toHaveText("Syncing branches…")

    await repo.holdRemoteAnswers({ release: ["sync"] })
    await expect(button).toHaveText("Update")
    await expect(progress).toHaveCount(0)
    // no robot runs: the Dock has no progress again
    await expect.poll(async () => (await getDockCalls(page)).at(-1)).toEqual({ progressBar: { status: "none" } })
  })

  test("should let a rewrite run during the fetch of an update, which checks and applies after it", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    // an agent's `bd update` fetches the baseline: the fetch of the update waits for it in the session
    await repo.holdFetchLock(true)
    const executions = await recordCommand(page, "execute")
    const parks = await recordRewrites(page, "park")
    await updateButton(page).click()
    await expect(page.getByTestId("update-progress")).toHaveAttribute("data-step", "fetch")
    await expect(page.getByTestId("update-state-line-button")).toHaveCount(0)

    // the fetch holds no HEAD: a park during it runs at once, its toast before the update ends
    await startDrag(page, findBranchRow(page, "feature-auth"))
    await runAndWaitForSync(page, () => dropAt(page, page.getByTestId("drag-target-bar").getByTestId("park-drop-zone")))
    expect(parks).toHaveLength(1)
    await expect(page.locator("[data-slot=toast]", { hasText: "Parked feature-auth" })).toBeVisible()
    await expect(page.locator("[data-slot=toast]", { hasText: /^Cannot park/ })).toHaveCount(0)
    await expect(page.getByTestId("update-progress")).toHaveAttribute("data-step", "fetch")

    // the update checks and applies once its fetch went, on what the park left
    await runAndWaitForUpdate(page, () => repo.holdFetchLock(false))
    await expect(page.getByText("Updated master: rebased 1 commit onto origin/master")).toBeVisible()
    expect(executions.map(call => (call.intent as { kind: string }).kind)).toEqual(["updateProject", "rewrite"])
  })

  test("should run an update pressed during a rewrite after it, waiting for it in the queue of the session", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    // an agent holds the repository lock: the park waits for it in the session, holding HEAD
    await repo.holdRepositoryLock(true)
    await expect(await parkWaiting(page)).toHaveText(/^Waiting for `bd pick --follow`/)

    // ⌘T: the update fetches, then waits for the park in the queue of the session, never refused
    await expect((await updateWaiting(page)).getByRole("listitem")).toHaveText([
      "Fetched origin/master",
      "Waiting for Branch Deck or bd to finish…",
      "Check the update",
      "Update to origin/master",
      "Sync branches",
    ])

    // the holder is done: the park runs, then the update on what it left
    await expectUpdatedAndParked(page, repo)
  })

  test("should run a rewrite pressed during the check of an update after it, on what the update left", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    // an agent holds the repository lock: the update waits for it after its check, holding HEAD
    await repo.holdRepositoryLock(true)
    await expect(await updateWaiting(page)).toContainText(/Waiting for `bd pick --follow`/)

    // a park meanwhile waits behind the update in the queue of the session, never refused
    await expect(await parkWaiting(page)).toHaveText("Waits for Updating the project…")
    await expectUpdatedAndParked(page, repo)
  })

  test("should say whom an update waits for after its check, whose Stop Waiting changes nothing but the fetch", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    const head = await repo.revision("HEAD")
    await repo.holdRepositoryLock(true)
    await expect(await updateWaiting(page)).toContainText(/Waiting for `bd pick --follow` \(pid \d+, for \d+ s\)…/)

    const cancels = await recordCommand(page, "cancel_operation")
    await runAndWaitForUpdate(page, () => page.getByTestId("stop-waiting").click())
    expect(cancels).toHaveLength(1)
    await expect(page.getByText("Canceled the update before it started")).toBeVisible()
    await expect(page.getByText("Nothing changed; the fetch stays")).toBeVisible()
    await expect(page.getByText(/^Cannot update/)).toHaveCount(0)
    expect(await repo.revision("HEAD")).toBe(head)
    await repo.holdRepositoryLock(false)
  })

  test("should end the progress at the result when there is nothing to apply, its sync on Sync", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    await runAndWaitForUpdate(page, () => updateButton(page).click())
    await expect(page.getByText("Updated master: rebased 1 commit onto origin/master")).toBeVisible()
    await expect(updateButton(page)).toHaveText("Update")

    // up to date now: the tooltip still lists every step, the check decides what runs
    const update = await holdIntent(page, "updateProject")
    await updateButton(page).click()
    await page.mouse.move(0, 0)
    await updateButton(page).hover()
    const steps = page.getByTestId("update-steps").getByRole("listitem")
    await expect(steps).toHaveText(["Fetching origin/master…", "Check the update", "Update to origin/master", "Sync branches"])
    await update.send({ kind: "step", op: "held", step: { kind: "checking" } })
    await expect(steps).toHaveText(["Fetched origin/master", "Checking the update onto origin/master…", "Update to origin/master", "Sync branches"])

    // the update ends with its check, the sync the session asks after its fetch is Sync's
    await repo.holdRemoteAnswers({ hold: ["sync"] })
    update.release()
    await expect(page.getByText("master is already up to date with origin/master")).toBeVisible()
    await expect(updateButton(page)).toHaveText("Update")
    await expect(updateButton(page)).not.toHaveAttribute("aria-busy")
    await expect(page.getByTestId("sync-button")).toHaveAttribute("aria-busy", "true")
    await repo.holdRemoteAnswers({ release: ["sync"] })
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
    // `bd push auth` held the repository for the whole wait of the update
    await routeCommandError(
      page,
      "execute",
      {
        code: "locked",
        message: "`bd push auth` (pid 4242) is changing this repository; try again when it is done",
        details: { type: "locked", holder: { tool: "bd", command: "bd push auth", pid: 4242, held_for: 40 } },
        retry: "same",
      },
      { times: 1 },
    )
    const executions = await recordCommand(page, "execute")

    await runAndWaitForUpdate(page, () => updateButton(page).click())
    const toast = page.locator("[data-slot=toast]").filter({ hasText: "Cannot update the project" })
    await expect(toast).toContainText("`bd push auth` (pid 4242) has held the repository for 40 s")
    await expect(toast).not.toContainText("branch-deck.lock")

    await runAndWaitForUpdate(page, () => toast.getByRole("button", { name: "Retry Without Fetch" }).click())
    await expect(page.getByText("Updated master: rebased 1 commit onto origin/master")).toBeVisible()
    // the update checks and applies against the baseline the last fetch got
    expect(executions.map(call => call.intent)).toEqual([
      { kind: "updateProject", fetch: true },
      { kind: "updateProject", fetch: false },
    ])
  })

  test("should offer no retry when the branch moved under the update", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    await routeCommandError(page, "execute", { code: "head_moved", message: "master moved during the update: nothing was changed", retry: "after_reread" })

    await runAndWaitForUpdate(page, () => updateButton(page).click())
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

    await runAndWaitForUpdate(page, () => button.click())
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
    // the first commit that conflicts, where the rebase stops; the commit before it replays and is not named
    await expect(dialog.getByTestId("update-first-conflict")).toHaveText(/^First conflict[0-9a-f]{8}feature-auth Change line 2$/)
    await expect(dialog).not.toContainText("Add login")

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
    expect(prompt).toMatch(/First conflict: [0-9a-f]{8} \(feature-auth\) Change line 2: src\/app\.txt[.,]/)
    // the protocol is bd's own; each round names the upstream commits of its files
    expect(prompt).toContain("Run `bd help update` first and follow it")
    expect(prompt).not.toContain("Upstream commits of these files")
    expect(prompt).toMatch(/bd is `\/Users\/test\/\.local\/bin\/bd -C \S+`; below, `bd` stands for it\./)
    expect(prompt).toContain("1. `bd update --no-fetch`.")
    expect(prompt).toContain("edit only the copies at `edit`")
    expect(prompt).toContain("When an output or its `error` has `ask_user: true`")
    expect(prompt).toContain("never edit a conflicting file in the working tree.")
    expect(prompt).toContain("Don't push.")
    expect(prompt).not.toContain("git stash push")

    await page.keyboard.press("Escape")
    await expect(dialog).toBeHidden()
    // nothing changed
    await verifyCommitCount(findBranchRow(page, "feature-auth"), 2)
    await expect(button).toHaveAttribute("data-status", "conflict")
  })

  test("should list the first files of a wide conflict and count the rest", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // `update_conflict_wide`: 60 files that the local commit adds and each of 7 upstream commits changes
    await setupRepo("update_conflict_wide")
    await syncAndWaitForBranches()
    const button = updateButton(page)
    await expect(button).toHaveAttribute("data-status", "conflict")
    await expectTooltip(page, "Conflicts in 60 files: src/wide/file00.txt, src/wide/file01.txt, src/wide/file02.txt and 57 more")

    await runAndWaitForUpdate(page, () => button.click())
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
    expect(prompt).toContain("src/wide/file19.txt, and 40 more files.")
    expect(prompt.length).toBeLessThan(8000)
  })

  test("should explain uncommitted changes that block the update", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("update_blocked")
    await syncAndWaitForBranches()
    const button = updateButton(page)
    await expect(button).toHaveAttribute("data-status", "blocked")
    await expectTooltip(page, "Your uncommitted changes in src/app.txt overlap origin/master")

    await runAndWaitForUpdate(page, () => button.click())
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
