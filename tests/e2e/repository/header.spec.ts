// @spec openspec/specs/repository/update-project/spec.md
// @spec openspec/specs/repository/settings/spec.md
// @spec openspec/specs/repository/activity/spec.md
// @spec openspec/specs/repository/title-bar/spec.md
import type { Page } from "@playwright/test"
import { expect, test } from "../fixtures/test-fixtures"
import { activityButton, activityEntry, openActivityWindow } from "../helpers/activity-helpers"
import { captureHtmlSnapshot } from "../helpers/aria-snapshot-helpers"
import { findBranchRow, pushFromRowMenu } from "../helpers/branch-helpers"
import type { Box } from "../helpers/header-helpers"
import { boxOf, expectHeaderControlsOnOneLine, header, HEADER_CONTROLS, HEADER_HEIGHT, headerBoxes, MIN_WINDOW } from "../helpers/header-helpers"
import { settledIpc } from "../helpers/ipc-helpers"
import { availableRobots } from "../helpers/patronus-helpers"
import { expectPushedToast } from "../helpers/push-helpers"
import { pressAndExpectSettingsOpened } from "../helpers/settings-helpers"
import { emitTauriEvent, finishedSyncCount, focusWindow, nextAnswer, runAndWaitForAnswer, runAndWaitForUpdate, syncFromMenu, waitForSyncComplete } from "../helpers/sync-helpers"
import { TITLE_BARS, useTitleBar } from "../helpers/title-bar-helpers"
import { holdCommand, holdIntent, expectClosed, openTooltip } from "../helpers/wait-helpers"

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
    // the update is git on a real repository: its end is its answer, however long the machine takes
    await runAndWaitForUpdate(page, async () => update.release())
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
    await expectClosed(openTooltip(page))
  })

  test("should open Settings from the button after Sync, its tooltip naming the key", async ({ page }) => {
    const button = header(page).getByRole("button", { name: "Settings", exact: true })
    await page.mouse.move(0, 0)
    await button.hover()
    await expect(openTooltip(page)).toHaveText(/^Settings(⌘,|Ctrl\+,)$/)
    await pressAndExpectSettingsOpened(page, () => button.click(), {})
  })
})

test.describe("Repository selector", () => {
  for (const activation of ["click", "ArrowDown"] as const) {
    test(`should explain the empty repository selector opened by ${activation} and return focus on Escape`, { tag: "@webkit" }, async ({ page, setupRepo }) => {
      await page.setViewportSize(MIN_WINDOW)
      await setupRepo("NO_REPO", { prepopulateStore: false })
      await expect(page.getByTestId("welcome-card")).toBeVisible()
      const selector = header(page).getByRole("combobox", { name: "Repository", exact: true })
      if (activation === "click") {
        await selector.click()
      } else {
        await selector.focus()
        await expect(selector).toBeFocused()
        await selector.press(activation)
      }
      const list = page.getByRole("listbox")
      const title = list.getByText("No recent repositories", { exact: true })
      await expect(title).toBeVisible()
      await expect(list.getByText("Use Add Repository… to open a Git repository. Repositories you open appear here.")).toBeInViewport({ ratio: 1 })
      await expect(selector).toHaveAccessibleDescription("No recent repositories Use Add Repository… to open a Git repository. Repositories you open appear here.")
      await expect(list.getByRole("option")).toHaveCount(0)
      // Playwright's click checks that the welcome card does not cover the message; it selects no fake repository.
      await title.click()
      await expect(selector).toHaveAttribute("aria-expanded", "true")
      await expect(selector).toContainText("Select repository…")
      const popup = page.getByTestId("repository-selector-popup")
      const add = popup.getByRole("button", { name: "Add Repository…", exact: true })
      await expect(add).toBeInViewport({ ratio: 1 })
      await captureHtmlSnapshot(popup, "repository-selector-empty")
      await add.focus()
      await expect(add).toBeFocused()
      await page.keyboard.press("Escape")
      await expectClosed(list)
      await expect(selector).toBeFocused()
    })
  }

  for (const activation of ["click", "Enter", "Space"] as const) {
    test(`should add the first repository from the selector footer by ${activation}`, { tag: "@webkit" }, async ({ page, setupRepo }) => {
      const repo = await setupRepo("simple", { prepopulateStore: false })
      const selector = header(page).getByRole("combobox", { name: "Repository", exact: true })
      await selector.focus()
      await selector.press("ArrowDown")
      const popup = page.getByTestId("repository-selector-popup")
      await expect(popup.getByText("No recent repositories", { exact: true })).toBeVisible()
      const add = popup.getByRole("button", { name: "Add Repository…", exact: true })
      if (activation === "click") {
        await add.click()
      } else {
        // With no options, the popup gives its footer button the focus once it opened: a look before that saw the
        // focus still on the selector, and a Tab then left the popup for the folder button
        await expect(add).toBeFocused()
        await page.keyboard.press(activation)
      }
      await expectClosed(popup)
      await expect(page.getByTestId("ready-to-sync-card")).toBeVisible()
      await expect(selector).toContainText(repo.path.split("/").at(-1)!)
      await selector.click()
      const list = page.getByRole("listbox")
      await expect(list.getByRole("option")).toHaveCount(1)
      await expect(list.getByRole("option", { selected: true })).toContainText(repo.path.split("/").at(-1)!)
      await expect(list.getByText("No recent repositories")).toHaveCount(0)
      await expect(list.getByRole("button")).toHaveCount(0)
    })
  }

  test("should add another repository from the selector footer", { tag: "@webkit" }, async ({ page, setupRepo, otherRepository }) => {
    const repo = await setupRepo("simple")
    await repo.setDialogPick(otherRepository)
    const selector = header(page).getByRole("combobox", { name: "Repository", exact: true })
    await selector.click()
    await expect(page.getByRole("option")).toHaveCount(1)
    await page.getByRole("button", { name: "Add Repository…", exact: true }).click()
    await expectClosed(page.getByTestId("repository-selector-popup"))
    await expect(selector).toContainText(otherRepository.split("/").at(-1)!)
    await selector.click()
    await expect(page.getByRole("option")).toHaveCount(2)
    await expect(page.getByRole("option", { selected: true })).toContainText(otherRepository.split("/").at(-1)!)
  })

  for (const activation of ["Enter", "Space"] as const) {
    test(`should keep the selected repository when adding from the footer by ${activation} is cancelled`, { tag: "@webkit" }, async ({ page, setupRepo }) => {
      const repo = await setupRepo("simple")
      await repo.setDialogPick(null)
      const selector = header(page).getByRole("combobox", { name: "Repository", exact: true })
      const selectedPath = await selector.textContent()
      await selector.focus()
      await selector.press("ArrowDown")
      await expect(page.getByRole("option", { selected: true })).toBeFocused()
      await page.keyboard.press("Tab")
      await expect(page.getByRole("button", { name: "Add Repository…", exact: true })).toBeFocused()
      await runAndWaitForAnswer(page, "browse_repository", () => page.keyboard.press(activation))
      await expectClosed(page.getByTestId("repository-selector-popup"))
      await expect(selector).toHaveText(selectedPath!)
      await expect(selector).toBeFocused()
      await selector.press("ArrowDown")
      await expect(page.getByRole("option")).toHaveCount(1)
      await expect(page.getByRole("option", { selected: true })).toContainText(repo.path.split("/").at(-1)!)
    })
  }

  test("should report an invalid folder chosen from the selector footer", { tag: "@webkit" }, async ({ page, setupRepo }) => {
    await setupRepo("empty-non-git", { prepopulateStore: false })
    await header(page).getByRole("combobox", { name: "Repository", exact: true }).click()
    await page.getByRole("button", { name: "Add Repository…", exact: true }).click()
    await expectClosed(page.getByTestId("repository-selector-popup"))
    await expect(page.getByTestId("error-alert")).toContainText("Not a Git repository")
    await expect(header(page).getByRole("combobox", { name: "Repository", exact: true })).toHaveAttribute("aria-invalid", "true")
  })

  test("should keep the add footer visible while repository options scroll", { tag: "@webkit" }, async ({ page, setupRepo }) => {
    const repo = await setupRepo("simple")
    await repo.setDialogPick(null)
    await page.setViewportSize({ width: MIN_WINDOW.width, height: 400 })
    await page.addInitScript(() => {
      const store = (window as unknown as { __TAURI_STORE__: { recentProjects: { path: string }[] } }).__TAURI_STORE__
      store.recentProjects.push(...Array.from({ length: 30 }, (_, index) => ({ path: `/repositories/project-${index}` })))
    })
    await settledIpc(page)
    await page.reload()
    const selector = header(page).getByRole("combobox", { name: "Repository", exact: true })
    await selector.click()
    const options = page.getByRole("listbox").getByRole("option")
    await expect(options).toHaveCount(31)
    const add = page.getByRole("button", { name: "Add Repository…", exact: true })
    // An option is as wide as the list, whose width is fractional (267.44 px), and WebKit cuts the scrollport of the list
    // at a whole pixel (267 px): such an option is never whole there. 0.99 still fails a cut of a third of a pixel of its
    // 32 px height, so the option shows its whole height
    const wholeHeight = { ratio: 0.99 }
    await expect(options.first()).toBeInViewport(wholeHeight)
    await expect(options.last()).not.toBeInViewport()
    await expect(add).toBeInViewport({ ratio: 1 })
    const footerBox = await boxOf(add)
    expect(footerBox.height).toBe(24)
    // The down arrow of a scrollable list must not cover the footer before scrolling to the end.
    await add.click({ trial: true })
    await page.keyboard.press("End")
    await expect(options.last()).toBeFocused()
    await expect(options.last()).toBeInViewport(wholeHeight)
    await expect(options.first()).not.toBeInViewport()
    await expect(add).toBeInViewport({ ratio: 1 })
    expect(await boxOf(add)).toEqual(footerBox)
    await runAndWaitForAnswer(page, "browse_repository", () => add.click())
    await expectClosed(page.getByTestId("repository-selector-popup"))
    await expect(selector).toContainText(repo.path.split("/").at(-1)!)
  })

  test("should replace the empty selector with the repository opened through the folder button", async ({ page, setupRepo }) => {
    await setupRepo("simple", { prepopulateStore: false })
    const selector = header(page).getByRole("combobox", { name: "Repository", exact: true })
    await selector.click()
    await expect(page.getByRole("listbox").getByText("No recent repositories")).toBeVisible()
    await expect(selector).toHaveAccessibleDescription(/No recent repositories/)
    await page.keyboard.press("Escape")
    await expectClosed(page.getByRole("listbox"))
    await page.getByTestId("browse-repository-button").click()
    await expect(page.getByTestId("ready-to-sync-card")).toBeVisible()
    await selector.click()
    const list = page.getByRole("listbox")
    await expect(list.getByRole("option")).toHaveCount(1)
    await expect(list.getByText("No recent repositories")).toHaveCount(0)
    await expect(selector).not.toHaveAccessibleDescription(/No recent repositories/)
  })

  test("should explain an empty repository selector without offering a folder dialog on an IDE host", async ({ page, setupRepo }) => {
    await useTitleBar(page, { kind: "native", checkForUpdates: false, folderDialog: false })
    await setupRepo("NO_REPO", { prepopulateStore: false })
    await header(page).getByRole("combobox", { name: "Repository", exact: true }).click()
    const list = page.getByRole("listbox")
    await expect(list.getByText("No repositories available")).toBeVisible()
    await expect(list.getByText("Open a Git repository in your IDE project.")).toBeVisible()
    await expect(list.getByRole("option")).toHaveCount(0)
    await expect(page.getByTestId("browse-repository-button")).toHaveCount(0)
    await expect(page.getByRole("button", { name: "Add Repository…", exact: true })).toHaveCount(0)
  })

  /** The path the trigger shows, which must be a form of `path`: a tail of it from a separator behind "…" */
  async function expectFormOf(page: Page, path: string): Promise<string> {
    const shown = header(page).getByTestId("repository-path")
    const text = (await shown.textContent())!
    expect(text.startsWith("…/") && path.endsWith(text.slice(1)), `"${text}" is a tail of ${path} behind …`).toBe(true)
    // it fits its room: no character is cut off
    expect(await shown.evaluate(element => element.scrollWidth <= element.clientWidth)).toBe(true)
    return text
  }

  test("should show the unlabelled recent-project list when there are no companions", async ({ page, setupRepo }) => {
    await setupRepo("behind_upstream")
    await header(page).getByRole("combobox").click()
    const list = page.getByRole("listbox")
    await expect(list.getByRole("option")).toHaveCount(1)
    await captureHtmlSnapshot(page.getByTestId("repository-selector-popup"), "repository-selector-without-companions")
  })

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
    await expect(openTooltip(page)).toHaveText(repo.path)
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
    await runAndWaitForAnswer(page, "execute", () => pushFromRowMenu(page, "feature-auth"))
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
    await pressAndExpectSettingsOpened(page, () => header(page).getByTestId("settings-button").click(), {})
  })
})

test.describe("Header before the answers", () => {
  test("should keep its controls in place when Patronus answers after the sync", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await page.setViewportSize(MIN_WINDOW)
    const repo = await setupRepo("behind_upstream", { patronus: availableRobots() })
    await repo.holdRemoteAnswers({ hold: ["robots"] })
    await syncAndWaitForBranches()
    // the page asked Patronus, whose answer decides the user's move: none is counted before it
    await repo.arrival("robots")
    await expect(header(page).getByTestId("landing-indicator")).toBeVisible()
    await expect(header(page).getByTestId("landing-moves")).toHaveCount(0)
    const before = await headerBoxes(page, HEADER_CONTROLS)

    // Patronus answered: the move of feature-auth counts in the place the count kept
    await repo.holdRemoteAnswers({ release: ["robots"] })
    await expect(header(page).getByTestId("landing-moves")).toHaveText("1")
    expect(await headerBoxes(page, HEADER_CONTROLS)).toEqual(before)
  })
})

test.describe("Header as the first sync comes", () => {
  // the header stands at its end: Update comes on the left of what it would move, and the time of the last sync keeps
  // its room from before the first sync, so neither moves Activity or the landing indicator from under a still pointer
  test(
    "should keep Activity under the pointer, its tooltip open, as the first sync brings Update and the time of the last sync",
    { tag: "@webkit" },
    async ({ page, setupRepo }) => {
      // wide enough for the time of the last sync, which a narrow header hides
      await page.setViewportSize({ width: 1280, height: 800 })
      await setupRepo("behind_upstream")
      await expect(page.getByTestId("ready-to-sync-card")).toBeVisible()
      const activity = activityButton(page)
      await activity.hover()
      await expect(openTooltip(page)).toHaveText("Activity")
      const before = await boxOf(activity)
      // empty, loading or with work: the one button of the landing indicator
      const landing = header(page).getByTestId(/^landing-indicator(-empty|-loading)?$/)
      const indicator = await boxOf(landing)
      await expect(header(page).getByTestId("update-project-button")).toHaveCount(0)
      await expect(header(page).getByTestId("last-sync-time")).toHaveCount(0)

      // the first sync, from the menu: the pointer stays where it is
      await syncFromMenu(page)
      await expect(header(page).getByTestId("update-project-button")).toBeVisible()
      await expect(header(page).getByTestId("last-sync-time")).toBeVisible()
      expect(await boxOf(activity)).toEqual(before)
      expect(await boxOf(landing)).toEqual(indicator)
      await expect(openTooltip(page)).toHaveText("Activity")
    },
  )
})

test.describe("Header as work comes on its way", () => {
  // the header stands at its end: the landing indicator keeps its place while nothing is on the way, so the first work
  // that comes (a robot or a pull request from another window, a branch of brd) moves Update from under no pointer
  test("should keep Update under the pointer, its tooltip open, as the first work on the way comes and fills the place of the landing indicator", async ({
    page,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    await page.setViewportSize(MIN_WINDOW)
    const repo = await setupRepo("simple")
    await syncAndWaitForBranches()
    // test-branch parked: nothing on the way, the indicator empty in its place
    await findBranchRow(page, "test-branch").click({ button: "right" })
    await page.getByRole("menuitem", { name: "Park Branch…" }).click()
    await expect(page.getByTestId("inline-park-input").getByRole("textbox")).toBeFocused()
    await page.keyboard.press("Enter")
    const empty = header(page).getByTestId("landing-indicator-empty")
    await expect(empty).toHaveAccessibleName("Nothing on the way to master")
    await expect(empty).toBeDisabled()

    const update = header(page).getByTestId("update-project-button")
    await update.hover()
    const tooltip = page.getByTestId("update-project-tooltip").and(page.locator("[data-open]"))
    await expect(tooltip).toBeVisible()
    const before = await boxOf(update)

    // a branch brd made behind the app's back: its sync brings the work on its way
    await repo.commit("(fresh) Add the fresh file", { "fresh.txt": "fresh\n" }, 1704110400)
    expect(await repo.touchStampAndAnswer()).toBe("synced")
    await expect(header(page).getByTestId("landing-indicator")).toHaveAccessibleName("On the way to master: 1 move is yours")
    await expect(empty).toHaveCount(0)
    expect(await boxOf(update)).toEqual(before)
    await expect(tooltip).toBeVisible()
  })
})
