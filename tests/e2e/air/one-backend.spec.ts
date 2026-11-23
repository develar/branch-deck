// @spec openspec/specs/repository/backend-protocol/spec.md
import { closeFakeIde, openFakeIde } from "../air-host/fake-host"
import { test, expect } from "../fixtures/test-fixtures"
import { focusWindow } from "../helpers/sync-helpers"
import { expectClosed } from "../helpers/wait-helpers"

/*
 * The desktop window and the IDE view of one test repository on the one backend of the test server (one host per test
 * repository, `TestRepository::host`): the page mock of the desktop window goes through its connection of the app, the
 * fake IDE (`tests/e2e/air-host/`) through the app link, and both show the one session of the repository. The fake IDE
 * runs in a browser context of its own, without the mock of Tauri, on the clock of the desktop window
 */
test.describe("One backend for the desktop app and the IDE", () => {
  test("the desktop window and the IDE view of one repository say the same check time, and the Command Palette of the IDE opens no palette of the desktop window", async ({
    page,
    browser,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("issue_links")
    await syncAndWaitForBranches()
    const ideContext = await browser.newContext()
    // the clock of the desktop window (`base-test.ts`): an age the IDE words is the one the desktop window words
    await ideContext.clock.install({ time: new Date("2024-01-02T10:00:00Z") })
    const idePage = await ideContext.newPage()
    const ide = await openFakeIde(idePage, { repoId: repo.id, repositoryPath: repo.path })
    const main = ide.frame("main:1")
    try {
      await expect(main.locator('[data-testid="branch-row"]').first()).toBeVisible()
      const lastSync = page.getByTestId("last-sync-time")
      const ideLastSync = main.getByTestId("last-sync-time")

      // a check of the desktop window a minute after the sync: the session says it, so the IDE says it too
      expect((await focusWindow(page))?.outcome).toBe("unchanged")
      await page.clock.fastForward("01:15")
      await idePage.clock.fastForward("01:15")
      expect((await focusWindow(page))?.outcome).toBe("unchanged")
      await expect(lastSync).toHaveText("Checked just now")
      await expect(ideLastSync).toHaveText("Checked just now")

      // the action of the app file of the IDE opens the palette of its main view, an event of its own connection: the
      // desktop window opens none
      await ide.runAction("command-palette", "open_command_palette")
      await expect(main.getByTestId("command-palette")).toBeVisible()
      await expect(page.getByTestId("command-palette")).toHaveCount(0)
      await main.getByTestId("command-palette").press("Escape")
      await expectClosed(main.getByTestId("command-palette"))
    } finally {
      const refusals = await closeFakeIde(idePage)
      await ideContext.close()
      expect(refusals).toEqual([])
    }
  })
})
