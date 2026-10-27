// @spec spec/repository/activity.spec.md
import type { Page } from "@playwright/test"
import { expect, test, testWithIssueLinks } from "../fixtures/test-fixtures"
import { activityButton, activityEntry, activityPanel, expectActivityOpened, openActivityWindow, openWindowFromPanel } from "../helpers/activity-helpers"
import {
  collapseBranch,
  expandBranch,
  expectNothingToPush,
  findBranchRow,
  getBranchCommitRow,
  getBranchDetailsRow,
  getPushButton,
  verifyBranchState,
  verifyCommitCount,
} from "../helpers/branch-helpers"
import { HEADER_HEIGHT, MIN_WINDOW } from "../helpers/header-helpers"
import { confirmForcePush, expectPushedToRemote, virtualBranch } from "../helpers/push-helpers"
import { multiSelectCommit, selectCommit } from "../helpers/selection-helpers"
import { getOpenedSubWindows, reopenLinkedSubWindow } from "../helpers/sub-window-helpers"
import { emitTauriEvent, runAndWaitForSync, runAndWaitForUpdate, syncFromMenu, waitForTauriListener } from "../helpers/sync-helpers"
import { openMainMenuView, TITLE_BARS, useTitleBar, windowCalls } from "../helpers/title-bar-helpers"
import { waitForStorePersistence } from "../helpers/wait-helpers"

// `issue_links`: 7 branches, none pushed, ABC-123 has 4 commits. `behind_upstream` / `update_conflict`: master is behind
// origin/master, the update rebases cleanly / conflicts in src/app.txt. `landed_upstream`: a robot pushed feature-auth
// and the first of the two search commits to origin/master.

testWithIssueLinks.describe("Activity", () => {
  testWithIssueLinks("should record the pushes with the remote commits a force push dropped, and show the branch", async ({ page, issueLinksRepo }) => {
    // nothing happened yet: no button
    await expect(activityButton(page)).toHaveCount(0)
    const row = findBranchRow(page, "ABC-123")
    await getPushButton(row).click()
    await expectNothingToPush(row)
    // a commit of ABC-123 made since, and one someone else pushed onto its remote branch, which a fetch brought
    await issueLinksRepo.commit("(ABC-123) Note the session ttl", { "session.md": "ttl\n" }, 1704182460)
    const theirs = await issueLinksRepo.commitOnRemote(virtualBranch("ABC-123"), {
      subject: "Fix login",
      author: "Ann <ann@example.com>",
      timestamp: Date.parse("2024-01-02T07:00:00Z") / 1000,
      files: { "login.txt": "login\n" },
    })
    await issueLinksRepo.fetchInTerminal()
    await syncFromMenu(page)
    // it drops Ann's commit: the click asks first, the Force Push in the description pushes
    await row.getByRole("button", { name: "Force Push (1)" }).click()
    await confirmForcePush(page, "Force Push (1)")
    await expectNothingToPush(row)
    await expectPushedToRemote(issueLinksRepo, ["ABC-123"])

    const window = await openActivityWindow(page)
    // one day: the backend dates the journal by its own clock, the page of a test runs on a fixed one, so the day is
    // not "Today" here (the labels of the days: `groupActivityByDay` of activity.test.ts)
    await expect(window.getByTestId("activity-day")).toHaveCount(1)
    // newest first
    await expect(window.getByTestId("activity-title")).toHaveText(["Force pushed ABC-123", "Pushed ABC-123"])
    await expect(activityEntry(window, "Pushed ABC-123").getByTestId("activity-description")).toHaveText("4 commits · created the remote branch")
    const forced = activityEntry(window, "Force pushed ABC-123")
    await expect(forced).toHaveAttribute("data-kind", "push")
    await expect(forced.getByTestId("activity-description")).toHaveText("1 commit · dropped 1 remote commit")
    const dropped = forced.getByTestId("dropped-commits")
    await expect(dropped).toContainText("Fix login")
    await expect(dropped).toContainText(`Ann · ${theirs.slice(0, 8)} · 3 hours ago`)
    // the window names its repository
    expect((await getOpenedSubWindows(page)).at(-1)?.title).toMatch(/^Activity — /)

    // the branch name brings the main window to the front and shows its row; the window stays
    await collapseBranch(page, row)
    await forced.getByTestId("activity-branch").filter({ hasText: "ABC-123" }).click()
    await verifyBranchState(row, "open")
    await expect.poll(() => windowCalls(page)).toContain("set_focus")
    await expect(window.getByTestId("activity-window")).toBeVisible()
  })

  testWithIssueLinks("should filter by the chips of the kinds and the search, an open starting over at its chip", async ({ page }) => {
    await getPushButton(findBranchRow(page, "feature-auth")).click()
    await getPushButton(findBranchRow(page, "ABC-123")).click()
    const window = await openActivityWindow(page)
    const filter = window.getByTestId("activity-filter")
    // a chip shows while it counts an entry: no robot, no update yet
    await expect(filter.getByRole("button")).toHaveText(["All2", "Pushes2"])
    await expect(filter.getByRole("button", { name: /All/ })).toHaveAttribute("aria-pressed", "true")

    // the search: in the titles and descriptions, ignoring case
    await window.getByRole("textbox", { name: "Filter by branch or text" }).fill("abc")
    await expect(window.getByTestId("activity-title")).toHaveText(["Pushed ABC-123"])
    await expect(filter.getByRole("button", { name: /Pushes/ })).toHaveText("Pushes1")
    await window.getByRole("textbox", { name: "Filter by branch or text" }).fill("nothing like it")
    await expect(window.getByTestId("activity-empty")).toHaveText("No match for “nothing like it”Clear Filter")
    await window.getByTestId("activity-empty").getByRole("button", { name: "Clear Filter" }).click()
    await expect(window.getByTestId("activity-title")).toHaveCount(2)

    // a chip again goes back to All
    await filter.getByRole("button", { name: /Pushes/ }).click()
    await expect(filter.getByRole("button", { name: /Pushes/ })).toHaveAttribute("aria-pressed", "true")
    await filter.getByRole("button", { name: /Pushes/ }).click()
    await expect(filter.getByRole("button", { name: /All/ })).toHaveAttribute("aria-pressed", "true")

    // the next open, of the open window, starts over: its chip, an empty search
    await window.getByRole("textbox", { name: "Filter by branch or text" }).fill("abc")
    const count = (await getOpenedSubWindows(page)).length
    await openWindowFromPanel(page)
    await reopenLinkedSubWindow(window, await expectActivityOpened(page, count))
    await expect(window.getByRole("textbox", { name: "Filter by branch or text" })).toHaveValue("")
    await expect(window.getByTestId("activity-title")).toHaveCount(2)
  })

  const squashAndUndo = async ({ page }: { page: Page }, surface: "window" | "panel") => {
    const row = findBranchRow(page, "ABC-123")
    await expandBranch(page, row)
    await selectCommit(getBranchCommitRow(page, "ABC-123", "Add session management"))
    await multiSelectCommit(getBranchCommitRow(page, "ABC-123", "Add authentication tests"))
    await getBranchDetailsRow(page, "ABC-123").getByTestId("selection-actions").getByRole("button", { name: "Squash" }).click()
    const dialog = page.getByTestId("squash-dialog")
    await expect(dialog).toBeVisible()
    // the shortcut is the editor's: pressed before the dialog moved the focus there, it does nothing
    await expect(dialog.getByTestId("squash-message-editor")).toBeFocused()
    // a rewrite shows its result with the sync it starts
    await runAndWaitForSync(page, () => page.keyboard.press("ControlOrMeta+Enter"))
    await expect(dialog).toBeHidden()
    await verifyCommitCount(row, 3)

    // the Undo of the window is run by the main window, the panel's there itself; both are the toast's: it undoes once
    // and closes the toast
    let scope
    if (surface === "window") {
      scope = await openActivityWindow(page)
    } else {
      await activityButton(page).click()
      scope = activityPanel(page)
    }
    const entry = activityEntry(scope, "Squashed 2 commits")
    await expect(entry).toHaveAttribute("data-kind", "rewrite")
    await runAndWaitForSync(page, () => entry.getByTestId("activity-undo").click())
    await expect(entry.getByTestId("activity-undone")).toHaveText("undone")
    await expect(entry.getByTestId("activity-undo")).toHaveCount(0)
    await verifyCommitCount(row, 4)
    await expect(page.getByRole("button", { name: "Undo" })).toHaveCount(0)
  }

  testWithIssueLinks("should offer the Undo of a squash while the toast's lasts, in the window", async ({ page }) => {
    await squashAndUndo({ page }, "window")
  })

  testWithIssueLinks("should offer the Undo of a squash while the toast's lasts, in the panel", async ({ page }) => {
    await squashAndUndo({ page }, "panel")
  })

  testWithIssueLinks("should read the log from the journal of the repository after a reload", async ({ page, syncAndWaitForBranches }) => {
    const row = findBranchRow(page, "feature-auth")
    await getPushButton(row).click()
    await expectNothingToPush(row)
    // the push is in the journal of the repository, which a new page reads: no window keeps the log
    await page.reload()
    await syncAndWaitForBranches()
    const window = await openActivityWindow(page)
    await expect(window.getByTestId("activity-title")).toHaveText(["Pushed feature-auth"])
    await expect(activityEntry(window, "Pushed feature-auth").getByTestId("activity-description")).toHaveText("1 commit · created the remote branch")
  })

  testWithIssueLinks("should clear the log of the repository", async ({ page }) => {
    await getPushButton(findBranchRow(page, "feature-auth")).click()
    const window = await openActivityWindow(page)
    await expect(activityEntry(window, "Pushed feature-auth")).toBeVisible()
    await window.getByTestId("activity-clear").click()
    // the main window forgot it: no button, and the window says it
    await expect(activityButton(page)).toHaveCount(0)
    await expect(window.getByTestId("activity-empty")).toHaveText("Nothing yet: what Branch Deck does in the repository shows here")
    await expect(window.getByTestId("activity-clear")).toBeDisabled()
  })
})

testWithIssueLinks.describe("Activity side panel", () => {
  testWithIssueLinks("should toggle the side panel from the header button, show a branch from it, and remember it open", async ({ page }) => {
    // a real push, to the template's own origin
    const row = findBranchRow(page, "ABC-123")
    await getPushButton(row).click()
    await expectNothingToPush(row)

    await activityButton(page).click()
    const panel = activityPanel(page)
    await expect(panel.getByRole("heading", { name: "Activity" })).toBeVisible()
    const entry = activityEntry(panel, "Pushed ABC-123")
    await expect(entry.getByTestId("activity-description")).toHaveText("4 commits · created the remote branch")
    // beside the islands, in the main window: no window opens
    expect(await getOpenedSubWindows(page)).toEqual([])
    await waitForStorePersistence(page, "appSettings", settings => (settings as { activityPanel?: boolean } | undefined)?.activityPanel === true)

    // the branch name shows its row, the panel stays
    await collapseBranch(page, row)
    await entry.getByTestId("activity-branch").filter({ hasText: "ABC-123" }).click()
    await verifyBranchState(row, "open")
    await expect(panel).toBeVisible()

    // the button again closes it, and Close does too
    await activityButton(page).click()
    await expect(panel).toHaveCount(0)
    // a default is not written: closed, the setting goes
    await waitForStorePersistence(page, "appSettings", settings => (settings as { activityPanel?: boolean } | undefined)?.activityPanel !== true)
    await activityButton(page).click()
    await panel.getByRole("button", { name: "Close Activity" }).click()
    await expect(panel).toHaveCount(0)
  })

  testWithIssueLinks("should take the focus into the panel only when the keyboard opened it, and give it back on Escape", async ({ page }) => {
    // a real push, to the template's own origin
    const row = findBranchRow(page, "feature-auth")
    await getPushButton(row).click()
    await expectNothingToPush(row)
    const panel = activityPanel(page)

    // a click: the focus stays on the button, so Escape is not the panel's
    await activityButton(page).click()
    await expect(activityEntry(panel, "Pushed feature-auth")).toBeVisible()
    await expect(activityButton(page)).toBeFocused()
    await page.keyboard.press("Escape")
    await expect(panel).toBeVisible()
    await activityButton(page).click()
    await expect(panel).toHaveCount(0)

    // the keyboard: the focus goes into the log, Escape closes the panel and gives it back to the button
    await activityButton(page).focus()
    await page.keyboard.press("Enter")
    await expect(panel.getByTestId("activity-list")).toBeFocused()
    await page.keyboard.press("Escape")
    await expect(panel).toHaveCount(0)
    await expect(activityButton(page)).toBeFocused()
  })
})

test.describe("Activity of the selected repository", () => {
  test("should follow the repository the main window shows", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    const repo = await setupRepo("simple", { prepopulateStore: true })
    // a second recent project, which git cannot read: a log of its own, empty
    const otherPath = "/nonexistent/branch-deck-e2e-other"
    await page.addInitScript(path => {
      ;(window as unknown as { __TAURI_STORE__: { recentProjects: { path: string }[] } }).__TAURI_STORE__.recentProjects.push({ path })
    }, otherPath)
    await page.reload()
    await syncAndWaitForBranches()
    await getPushButton(findBranchRow(page, "test-branch")).click()
    const window = await openActivityWindow(page)
    await expect(activityEntry(window, "Pushed test-branch")).toBeVisible()
    const titles = (await windowCalls(window)).filter(call => call === "set_title").length

    await page.getByRole("combobox").click()
    await page.getByRole("option", { name: "branch-deck-e2e-other" }).click()
    await expect(window.getByTestId("activity-empty")).toHaveText("Nothing yet: what Branch Deck does in the repository shows here")
    // the title names the repository shown now
    await expect.poll(async () => (await windowCalls(window)).filter(call => call === "set_title").length).toBeGreaterThan(titles)

    await page.getByRole("combobox").click()
    await page.getByRole("option", { name: repo.path.split("/").at(-1)! }).click()
    await expect(activityEntry(window, "Pushed test-branch")).toBeVisible()
  })
})

test.describe("Activity of updates", () => {
  test("should record an applied update, the button on the header line at the minimum window width", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await page.setViewportSize(MIN_WINDOW)
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    await runAndWaitForUpdate(page, () => page.getByTestId("update-project-button").click())
    await expect(page.getByText("Updated master: rebased 1 commit onto origin/master")).toBeVisible()

    const header = await page.getByTestId("configuration-header").boundingBox()
    const button = await activityButton(page).boundingBox()
    const landing = await page.getByTestId("landing-indicator").boundingBox()
    expect(header!.height).toBeCloseTo(HEADER_HEIGHT, 0)
    expect(button!.y).toBeGreaterThanOrEqual(header!.y)
    expect(button!.y + button!.height).toBeLessThanOrEqual(header!.y + header!.height)
    // after the landing indicator, apart from it
    expect(button!.x).toBeGreaterThanOrEqual(landing!.x + landing!.width)

    // no room for the side panel beside the islands: the button opens the window
    const window = await openActivityWindow(page, () => activityButton(page).click())
    await expect(activityEntry(window, "Updated master: rebased 1 commit onto origin/master")).toHaveAttribute("data-kind", "update")
    await expect(activityPanel(page)).toHaveCount(0)
  })

  test("should toggle the side panel from the View menu of ☰ and from the native menu", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await useTitleBar(page, TITLE_BARS.gnome)
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    await runAndWaitForUpdate(page, () => page.getByTestId("update-project-button").click())
    await expect(page.getByText("Updated master: rebased 1 commit onto origin/master")).toBeVisible()

    await openMainMenuView(page)
    await page.getByRole("menuitem", { name: "Activity" }).click()
    await expect(page.getByTestId("main-menu")).toHaveCount(0)
    const panel = activityPanel(page)
    await expect(activityEntry(panel, "Updated master: rebased 1 commit onto origin/master")).toBeVisible()
    // a click leaves the focus outside the panel
    await expect(panel.getByTestId("activity-list")).not.toBeFocused()

    // the native item (macOS: Window → Activity) sends its command to the main window: it closes the open panel, then
    // opens it again; no window opens
    await waitForTauriListener(page, "open_activity")
    await emitTauriEvent(page, "open_activity")
    await expect(panel).toHaveCount(0)
    await emitTauriEvent(page, "open_activity")
    await expect(activityEntry(panel, "Updated master: rebased 1 commit onto origin/master")).toBeVisible()
    expect(await getOpenedSubWindows(page)).toEqual([])
  })

  test("should record a refused update", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("update_conflict")
    await syncAndWaitForBranches()
    await runAndWaitForUpdate(page, () => page.getByTestId("update-project-button").click())
    const dialog = page.getByTestId("update-blocked-dialog")
    await expect(dialog).toBeVisible()
    await page.keyboard.press("Escape")
    await expect(dialog).toBeHidden()

    const window = await openActivityWindow(page)
    const entry = activityEntry(window, "Didn't update master")
    await expect(entry.getByTestId("activity-description")).toHaveText("Conflicts in 1 file: src/app.txt. Nothing was changed.")
  })

  const keepLandedReport = async (
    { page, setupRepo, syncAndWaitForBranches, testStore }: Pick<Parameters<Parameters<typeof test>[2]>[0], "page" | "setupRepo" | "syncAndWaitForBranches" | "testStore">,
    surface: "window" | "panel",
  ) => {
    await setupRepo("landed_upstream")
    await syncAndWaitForBranches()
    await runAndWaitForUpdate(page, () => page.getByTestId("update-project-button").click())
    const card = page.getByTestId("landed-card")
    await expect(card).toBeVisible()

    // the user saw the island for less than 10 s: a sync keeps it
    await syncAndWaitForBranches()
    await expect(card).toBeVisible()
    // seen long enough: the next sync hides it, for good (remembered for the repository); the chips stay until Dismiss
    await page.clock.fastForward("00:11")
    await syncAndWaitForBranches()
    await expect(card).toHaveCount(0)
    // in what the syncs record of the repository (repository-state.json), not in the user's settings
    await expect
      .poll(async () => Object.values(((await testStore.get("repositoryState")) ?? {}) as Record<string, { landedHiddenUntil?: number }>)[0]?.landedHiddenUntil ?? 0)
      .toBeGreaterThan(0)
    expect(((await testStore.get("recentProjects")) as { landedHiddenUntil?: number }[])[0]?.landedHiddenUntil).toBeUndefined()
    const chip = findBranchRow(page, "search").getByTestId("landed-chip")
    await expect(chip).toHaveText("1 landed · 1 local")

    // the entry of the update keeps the report, with the rows of the island
    const scope =
      surface === "window"
        ? await openActivityWindow(page)
        : await activityButton(page)
            .click()
            .then(() => activityPanel(page))
    const entry = activityEntry(scope, "2 branches landed in origin/master")
    await expect(entry.getByTestId("activity-description")).toHaveText("feature-auth and search · rebased 2 commits")
    await entry.getByTestId("activity-landed-toggle").click()
    const search = entry.getByTestId("activity-landed-report").locator('[data-testid="landed-branch"][data-branch-name="search"]')
    await expect(search.getByTestId("landed-count")).toHaveText("1 landed · 1 stays local")
    await search.getByRole("button", { expanded: false }).click()
    await expect(search.getByTestId("landed-commits")).toContainText("Add index")

    // Dismiss from the entry, run by the main window (the panel is in it), clears the chips; the entry stays
    await entry.getByTestId("activity-landed-dismiss").click()
    await expect(chip).toHaveCount(0)
    await expect(entry.getByTestId("activity-landed-dismiss")).toHaveCount(0)
    await expect(entry).toBeVisible()
  }

  test("should keep the landed report after a sync hid the island, in the window", async ({ page, setupRepo, syncAndWaitForBranches, testStore }) => {
    await keepLandedReport({ page, setupRepo, syncAndWaitForBranches, testStore }, "window")
  })

  test("should keep the landed report after a sync hid the island, in the panel", async ({ page, setupRepo, syncAndWaitForBranches, testStore }) => {
    await keepLandedReport({ page, setupRepo, syncAndWaitForBranches, testStore }, "panel")
  })
})
