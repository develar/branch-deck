// @spec spec/repository/command-palette.spec.md
import { expect, test, testWithIssueLinks } from "../fixtures/test-fixtures"
import { activityPanel, expectActivityOpened } from "../helpers/activity-helpers"
import { collapseBranch, findBranchRow, getPushButton, getStackChip, verifyBranchState } from "../helpers/branch-helpers"
import { openPalette, palette, runByKeyboard, search } from "../helpers/command-palette-helpers"
import { expectSettingsOpened } from "../helpers/settings-helpers"
import { getOpenedSubWindows } from "../helpers/sub-window-helpers"
import { runAndWaitForSync, runAndWaitForUpdate } from "../helpers/sync-helpers"
import { openMainMenuView, TITLE_BARS, useTitleBar, windowCalls } from "../helpers/title-bar-helpers"
import { recordCommand } from "../helpers/wait-helpers"

// `issue_links`: 7 branches, ABC-123 and JIRA-456 among them; `behind_upstream`: master is behind origin/master

testWithIssueLinks.describe("Command palette", () => {
  testWithIssueLinks("should open from ⌘K of the native menu, list every source and give the focus back on Escape", async ({ page }) => {
    const row = findBranchRow(page, "ABC-123")
    await row.focus()
    await expect(row).toBeFocused()
    await openPalette(page)
    // the window comes to the front: ⌘K of a sub-window opens the palette of the main window
    await expect.poll(() => windowCalls(page)).toContain("set_focus")
    // the actions of the focused branch, named by it, between the commands of the app and the branches
    await expect(palette(page).getByRole("listbox").getByRole("group")).toHaveText([/^Views/, /^Activity/, /^Commands/, /^ABC-123/, /^Branches/])
    // the key of a command of the window, none for the others
    await expect(
      palette(page)
        .getByRole("option", { name: /Sync Branches/ })
        .locator("[data-slot=kbd]"),
    ).toHaveText(["Ctrl", "R"])
    await expect(palette(page).getByRole("option", { name: "Go to ABC-123" }).locator("[data-slot=kbd]")).toHaveCount(0)
    await expect(palette(page).getByRole("option", { name: "Go to ABC-123" })).toBeVisible()

    await page.keyboard.press("Escape")
    await expect(palette(page)).toHaveCount(0)
    await expect(row).toBeFocused()
  })

  testWithIssueLinks("should go to a branch: the row expanded, selected and focused", async ({ page }) => {
    const row = findBranchRow(page, "JIRA-456")
    await collapseBranch(page, row)
    await openPalette(page)
    await runByKeyboard(page, "jira-4", "Go to JIRA-456")
    await verifyBranchState(row, "open")
    await expect(row).toHaveAttribute("data-selected", "true")
    await expect(row).toBeFocused()
  })

  testWithIssueLinks("should show the branches view", async ({ page }) => {
    await openPalette(page)
    await runByKeyboard(page, "branches", "Branches")
    await expect(findBranchRow(page, "ABC-123")).toBeVisible()
  })

  testWithIssueLinks("should open the Activity panel with the focus in it from the keyboard, and the Activity window by a click", async ({ page }) => {
    await openPalette(page)
    await runByKeyboard(page, "activity panel", "Activity Panel")
    await expect(activityPanel(page).getByTestId("activity-list")).toBeFocused()

    await openPalette(page)
    const count = (await getOpenedSubWindows(page)).length
    await palette(page).getByRole("option", { name: "Activity Window" }).click()
    expect((await expectActivityOpened(page, count)).data.kind).toBeNull()
    // the log shows in one place
    await expect(activityPanel(page)).toHaveCount(0)
  })

  testWithIssueLinks("should offer the actions of the focused branch as its menu does and run one: Review Changes opens its review", async ({ page }) => {
    const row = findBranchRow(page, "JIRA-456")
    await row.focus()
    await expect(row).toBeFocused()
    await openPalette(page)
    const actions = palette(page)
      .getByRole("group")
      .filter({ hasText: /^JIRA-456/ })
    // the items of its menu, a submenu's item named with it; a rewrite during no sync acts
    await expect(actions.getByRole("option", { name: /^Review Changes/ })).toBeVisible()
    await expect(actions.getByRole("option", { name: /^Copy › Copy Branch Name/ })).toBeVisible()
    await expect(actions.getByRole("option", { name: /^Move to…/ })).not.toHaveAttribute("aria-disabled", "true")
    const count = (await getOpenedSubWindows(page)).length
    await runByKeyboard(page, "review changes", "Review Changes")
    await expect.poll(async () => (await getOpenedSubWindows(page)).slice(count).map(window => window.windowId)).toEqual(["branch-review"])
  })

  testWithIssueLinks("should run the commands of the app: Sync Branches, Settings… and Install Agent CLI…", async ({ page }) => {
    await openPalette(page)
    await runAndWaitForSync(page, () => runByKeyboard(page, "sync", "Sync Branches"))

    await openPalette(page)
    await runByKeyboard(page, "settings", "Settings…")
    await expectSettingsOpened(page, {})

    await openPalette(page)
    await palette(page).getByRole("option", { name: "Install Agent CLI…" }).click()
    await expect(page.locator("[data-slot=toast]", { hasText: "Agent CLI installed" })).toBeVisible()
  })
})

test.describe("Command palette of a stack", () => {
  test(
    "should go to the branch a search names before an action that names it, and focus its row in a collapsed stack",
    { tag: "@webkit" },
    async ({ page, setupRepo, syncAndWaitForBranches }) => {
      // auth and session stacked, neither pushed: the menu of session has Push session Only, the one of docs Stack On › session
      await setupRepo("stacked_branches")
      await syncAndWaitForBranches()
      const executes = await recordCommand(page, "execute")
      const session = findBranchRow(page, "session")
      const first = palette(page).getByRole("option").first()

      // session is the branch at hand: the focus on its push button (a Tab or a click put it there)
      await getPushButton(session).focus()
      await openPalette(page)
      await search(page).fill("session")
      // its name goes to it; its action that names it comes after, Enter would have pushed session alone
      await expect(first).toHaveText(/^Go to session$/)
      await expect(first).toHaveAttribute("data-highlighted", "")
      await expect(palette(page).getByRole("option", { name: "Push session Only" })).toBeVisible()
      await page.keyboard.press("Enter")
      await expect(palette(page)).toHaveCount(0)
      await expect(session).toBeFocused()
      await expect(session).toHaveAttribute("data-selected", "true")

      // the words of an action find it first
      await openPalette(page)
      await search(page).fill("push")
      await expect(first).toHaveText(/^Push Stack/)
      await page.keyboard.press("Escape")
      await expect(session).toBeFocused()

      // in a collapsed stack: the reveal expands it, then the row takes the focus; docs at hand names session in Stack On
      await getStackChip(findBranchRow(page, "auth")).click()
      await expect(session).toHaveCount(0)
      const docs = findBranchRow(page, "docs")
      await docs.focus()
      await openPalette(page)
      await search(page).fill("session")
      await expect(first).toHaveText(/^Go to session$/)
      await expect(palette(page).getByRole("option", { name: "Stack On › session" })).toBeVisible()
      await page.keyboard.press("Enter")
      await expect(palette(page)).toHaveCount(0)
      await expect(session).toBeFocused()
      await expect(session).toHaveAttribute("data-selected", "true")
      expect(executes).toEqual([])
    },
  )
})

test.describe("Command palette of a custom title bar", () => {
  test("should open from Ctrl+K and from View → Command Palette… of ☰, and run Update Project and Check for Updates…", async ({ page, setupRepo, syncAndWaitForBranches }) => {
    await useTitleBar(page, { ...TITLE_BARS.gnome, checkForUpdates: true })
    await setupRepo("behind_upstream")
    await syncAndWaitForBranches()

    await page.keyboard.press("Control+KeyK")
    await expect(search(page)).toBeFocused()
    await runAndWaitForUpdate(page, () => runByKeyboard(page, "update", "Update Project"))
    await expect(page.getByText("Updated master: rebased 1 commit onto origin/master")).toBeVisible()

    await openMainMenuView(page)
    await page.getByRole("menuitem", { name: /Command Palette…/ }).click()
    await expect(search(page)).toBeFocused()
    await runByKeyboard(page, "updates", "Check for Updates…")
    await expect(page.locator("[data-slot=toast]", { hasText: "is up to date" })).toBeVisible()
  })
})
