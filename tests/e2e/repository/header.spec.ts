// @spec spec/repository/update-project.spec.md
// @spec spec/repository/settings.spec.md
// @spec spec/repository/activity.spec.md
import type { Page } from "@playwright/test"
import { expect, test } from "../fixtures/test-fixtures"
import { activityButton, activityEntry, openActivityWindow } from "../helpers/activity-helpers"
import { pushFromRowMenu } from "../helpers/branch-helpers"
import type { Box } from "../helpers/header-helpers"
import { boxOf, expectHeaderControlsOnOneLine, header, HEADER_CONTROLS, HEADER_HEIGHT, headerBoxes, MIN_WINDOW } from "../helpers/header-helpers"
import { availableRobots } from "../helpers/patronus-helpers"
import { expectPushedToast } from "../helpers/push-helpers"
import { expectSettingsOpened } from "../helpers/settings-helpers"
import { emitTauriEvent, finishedSyncCount, focusWindow, nextAnswer, waitForSyncComplete } from "../helpers/sync-helpers"
import { TITLE_BARS, useTitleBar } from "../helpers/title-bar-helpers"
import { holdCommand, holdIntent } from "../helpers/wait-helpers"

// The `behind_upstream` template: the header has every control, the Update button (a clean update) and the outgoing
// indicator (feature-auth is not pushed).

/** The controls whose position moved on every sync before: they must stay where they are */
async function actionBoxes(page: Page): Promise<Box[]> {
  return Promise.all(["update-project-button", "landing-indicator", "sync-button"].map(id => boxOf(page.getByTestId(id))))
}

test.describe("Header", () => {
  test.beforeEach(async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await page.setViewportSize(MIN_WINDOW)
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
  })

  test("should keep its controls apart on one line at the minimum window width", async ({ page }) => {
    await expectHeaderControlsOnOneLine(page, HEADER_CONTROLS)
  })

  test("should keep its controls apart while an update runs", async ({ page }) => {
    const update = await holdIntent(page, "updateProject")
    await page.getByTestId("update-project-button").click()
    await update.send({ kind: "progress", op: "held", percent: 99, transferred: null, speed: null, line: "Receiving objects:  99% (99/100)" })
    await expect(page.getByTestId("update-project-button")).toHaveText(/^Updating \d+%$/)
    await expectHeaderControlsOnOneLine(page, HEADER_CONTROLS)
    update.release()
    await expect(page.getByTestId("update-project-button")).toHaveText("Update")
  })

  test("should not move while a sync runs", async ({ page }) => {
    const lastSync = page.getByTestId("last-sync-time")
    await expect(lastSync.getByText(/^Last synced/)).toBeVisible()
    const before = await actionBoxes(page)

    // hold the sync until the layout is measured
    const releaseSync = await holdCommand(page, "sync")
    // the reasons of the sync requests, and their answers
    const syncRequests: string[] = []
    const syncAnswers: string[] = []
    page.on("request", request => {
      if (request.url().endsWith("/invoke/sync")) {
        syncRequests.push((request.postDataJSON() as { reason: string }).reason)
      }
    })
    page.on("response", response => {
      if (response.url().endsWith("/invoke/sync")) {
        syncAnswers.push(response.url())
      }
    })
    const previousCount = await finishedSyncCount(page)
    const syncButton = page.getByTestId("sync-button")
    await syncButton.click()
    await expect(syncButton).toHaveAttribute("aria-busy", "true")

    // the spinner of Sync is the one indicator: the time of the last sync stays as it is, no "Syncing…" beside it
    await expect(lastSync).toHaveText(/^Last synced/)
    expect((await boxOf(header(page))).height).toBeCloseTo(HEADER_HEIGHT, 0)
    expect(await actionBoxes(page)).toEqual(before)
    // its tooltip names the step, without the shortcut that does nothing now; the click closed it, the next hover opens it
    await page.mouse.move(0, 0)
    await syncButton.hover()
    await expect(page.getByTestId("sync-tooltip")).toHaveText("Syncing branches…")

    // ⌘R while it runs starts no second sync, only a check of the session: nothing changed, so it syncs nothing
    await emitTauriEvent(page, "sync_branches")
    await expect.poll(() => syncRequests).toEqual(["manual", "focus"])
    releaseSync()
    await waitForSyncComplete(page, previousCount)
    await expect.poll(() => syncAnswers.length).toBe(2)
    await expect(syncButton).toHaveAttribute("data-sync-count", String(previousCount + 1))
    // the check after it came in the minute of the sync: it says nothing "Last synced just now" does not
    await expect(lastSync).toHaveText("Last synced just now")
  })

  test("should name the shortcut of Sync and keep the folder button of the selected repository without a tooltip", async ({ page }) => {
    // the click of the sync before closed the tooltip; it opens on the next hover
    await page.mouse.move(0, 0)
    await page.getByTestId("sync-button").hover()
    await expect(page.getByTestId("sync-tooltip")).toHaveText(/^Sync branches(⌘R|Ctrl\+R)$/)

    // with a tooltip delay of 0, a tooltip of the selector would open before the one of Sync is gone
    await page.getByTestId("browse-repository-button").hover()
    await expect(page.getByRole("tooltip")).toHaveCount(0)
  })

  test("should open Settings from the button after Sync, its tooltip naming the key", async ({ page }) => {
    const button = header(page).getByRole("button", { name: "Settings", exact: true })
    await page.mouse.move(0, 0)
    await button.hover()
    await expect(page.getByRole("tooltip")).toHaveText(/^Settings(⌘,|Ctrl\+,)$/)
    await button.click()
    await expectSettingsOpened(page, {})
  })
})

test.describe("Repository selector", () => {
  /** The path the trigger shows, which must be a form of `path`: a tail of it from a separator behind "…" */
  async function expectFormOf(page: Page, path: string): Promise<string> {
    const shown = header(page).getByTestId("repository-path")
    const text = (await shown.textContent())!
    expect(text.startsWith("…/") && path.endsWith(text.slice(1)), `"${text}" is a tail of ${path} behind …`).toBe(true)
    // it fits its room: no character is cut off
    expect(await shown.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true)
    return text
  }

  test("should cut the path at its separators behind one …, more of it as the room grows", async ({ page, setupRepo }) => {
    const repo = await setupRepo("NO_REPO", { prepopulateStore: false })
    // a path of short segments that the folder dialog picks: no repository, so the selector shows it with its error
    const path = "/Users/ann/projects/notes.local/scratch/demo"
    await repo.setDialogPick(path)
    await page.getByTestId("browse-repository-button").click()
    const shown = header(page).getByTestId("repository-path")
    await expect(shown).toHaveText(/^…\//)
    const wide = await expectFormOf(page, path)

    // a narrower window drops whole leading segments, and a wider one brings them back
    await page.setViewportSize({ width: 350, height: MIN_WINDOW.height })
    await expect.poll(async () => (await shown.textContent())!.length).toBeLessThan(wide.length)
    const narrow = await expectFormOf(page, path)
    expect(wide.endsWith(narrow.slice(1))).toBe(true)
    await page.setViewportSize(MIN_WINDOW)
    await expect(shown).toHaveText(wide)
  })

  test("should keep its width while the time of the last sync ages, the whole path in its tooltip", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // beside the traffic lights at the minimum width the selector gives way to the actions: a wider time took its room
    await useTitleBar(page, TITLE_BARS.macos)
    await page.setViewportSize(MIN_WINDOW)
    const repo = await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    const trigger = header(page).getByRole("combobox")
    const lastSync = header(page).getByTestId("last-sync-time")
    await expect(lastSync).toHaveText("Last synced just now")
    const before = await boxOf(trigger)
    expect(before.width, "the selector is narrower than its 16rem").toBeLessThan(256)

    for (const [forward, text] of [
      ["01:15", "Last synced 1 minute ago"],
      ["10:00", "Last synced 11 minutes ago"],
    ] as const) {
      await page.clock.fastForward(forward)
      await expect(lastSync).toHaveText(text)
      expect(await boxOf(trigger), text).toEqual(before)
    }
    expect((await focusWindow(page))?.outcome).toBe("unchanged")
    await expect(lastSync).toHaveText("Checked just now")
    expect(await boxOf(trigger)).toEqual(before)

    // the path of the template is longer than the room: the trigger cuts it, its tooltip says it whole
    await page.mouse.move(0, 0)
    await trigger.hover()
    await expect(page.getByRole("tooltip")).toHaveText(repo.path)
  })
})

test.describe("Header beside the traffic lights", () => {
  test("should keep its controls in place when the first entry of Activity comes, its button opening the empty log before", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    // at the minimum width beside the traffic lights the repository selector gives way to the actions
    await useTitleBar(page, TITLE_BARS.macos)
    await page.setViewportSize(MIN_WINDOW)
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()
    const before = await headerBoxes(page, HEADER_CONTROLS)
    const trigger = await boxOf(header(page).getByRole("combobox"))

    // nothing happened yet: the button opens the log, which says so (the window: the panel has no room here)
    const empty = await openActivityWindow(page, () => activityButton(page).click())
    await expect(empty.getByTestId("activity-empty")).toHaveText("Nothing yet: what Branch Deck does in the repository shows here")

    // the first entry: the push of feature-auth, which the main window reads once its journal changed
    const read = nextAnswer(page, "read_activity")
    await pushFromRowMenu(page, "feature-auth")
    await expectPushedToast(page, "feature-auth")
    await read
    expect(await headerBoxes(page, HEADER_CONTROLS)).toEqual(before)
    expect(await boxOf(header(page).getByRole("combobox"))).toEqual(trigger)
    const window = await openActivityWindow(page, () => activityButton(page).click())
    await expect(activityEntry(window, "Pushed branch feature-auth")).toBeVisible()
  })
})

test.describe("Header without a repository", () => {
  test("should offer Settings before a repository is chosen", async ({ page, setupRepo }) => {
    await setupRepo("NO_REPO", { prepopulateStore: false })
    await expect(page.getByTestId("welcome-card")).toBeVisible()
    await header(page).getByTestId("settings-button").click()
    await expectSettingsOpened(page, {})
  })
})

test.describe("Header before the answers", () => {
  test("should keep its controls in place when Patronus answers after the sync", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await page.setViewportSize(MIN_WINDOW)
    const repo = await setupRepo("behind_upstream", { patronus: availableRobots() })
    await repo.holdRemoteAnswers({ hold: ["robots"] })
    await syncAndWaitForBranches()
    // the page asked Patronus, whose answer decides the user's move: none is counted before it
    await expect.poll(() => repo.getArrivedAnswers()).toContain("robots")
    await expect(header(page).getByTestId("landing-indicator")).toBeVisible()
    await expect(header(page).getByTestId("landing-moves")).toHaveCount(0)
    const before = await headerBoxes(page, HEADER_CONTROLS)

    // Patronus answered: the move of feature-auth counts in the place the count kept
    await repo.holdRemoteAnswers({ release: ["robots"] })
    await expect(header(page).getByTestId("landing-moves")).toHaveText("1")
    expect(await headerBoxes(page, HEADER_CONTROLS)).toEqual(before)
  })
})
