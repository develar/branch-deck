// @spec openspec/specs/repository/backend-protocol/spec.md
import type { Browser, FrameLocator, Page } from "@playwright/test"
import type { PatronusRobotProfile } from "../../../src/lib/bindings"
import { closeFakeIde, openFakeIde } from "../air-host/fake-host"
import type { FakeIde } from "../air-host/fake-host"
import { test, expect } from "../fixtures/test-fixtures"
import { findBranchRow } from "../helpers/branch-helpers"
import { updateIntoLandedIsland } from "../helpers/landed-helpers"
import { availableRobots, robot } from "../helpers/patronus-helpers"
import { EARLIER_COMMIT } from "../helpers/review-status-helpers"
import { focusWindow } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { expectClosed } from "../helpers/wait-helpers"

/*
 * The desktop window and the IDE view of one test repository on the one backend of the test server (one host per test
 * repository, `TestRepository::host`): the page mock of the desktop window goes through its connection of the app, the
 * fake IDE (`tests/e2e/air-host/`) through the bridge, and both show the one session of the repository. The fake IDE
 * runs in a browser context of its own, without the mock of Tauri, on the clock of the desktop window
 */

/** The time of the clock of the desktop window (`base-test.ts`), 2024-01-02 10:00 UTC */
const NOW = 1704189600

/** The main view of the fake IDE on `repo`, in a browser context of its own on the clock of the desktop window */
async function openIdeOn(browser: Browser, repo: TestRepositoryBuilder): Promise<{ ide: FakeIde; page: Page; main: FrameLocator; close: () => Promise<void> }> {
  const context = await browser.newContext()
  // an age the IDE words is the one the desktop window words
  await context.clock.install({ time: new Date(NOW * 1000) })
  const page = await context.newPage()
  const ide = await openFakeIde(page, { repoId: repo.id, repositoryPath: repo.path })
  const close = async () => {
    const refusals = await closeFakeIde(page)
    await context.close()
    expect(refusals).toEqual([])
  }
  return { ide, page, main: ide.frame("main:1"), close }
}

test.describe("One backend for the desktop app and the IDE", () => {
  test("the desktop window and the IDE view of one repository say the same check time, and the Command Palette of the IDE opens no palette of the desktop window", async ({
    page,
    browser,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const repo = await setupRepo("issue_links")
    await syncAndWaitForBranches()
    const ide = await openIdeOn(browser, repo)
    try {
      await expect(ide.main.locator('[data-testid="branch-row"]').first()).toBeVisible()
      const lastSync = page.getByTestId("last-sync-time")
      const ideLastSync = ide.main.getByTestId("last-sync-time")

      // a check of the desktop window a minute after the sync: the session says it, so the IDE says it too
      expect((await focusWindow(page))?.outcome).toBe("unchanged")
      await page.clock.fastForward("01:15")
      await ide.page.clock.fastForward("01:15")
      expect((await focusWindow(page))?.outcome).toBe("unchanged")
      await expect(lastSync).toHaveText("Checked just now")
      await expect(ideLastSync).toHaveText("Checked just now")

      // the action of the app file of the IDE opens the palette of its main view, an event of its own connection: the
      // desktop window opens none
      await ide.ide.runAction("command-palette", "open_command_palette")
      await expect(ide.main.getByTestId("command-palette")).toBeVisible()
      await expect(page.getByTestId("command-palette")).toHaveCount(0)
      await ide.main.getByTestId("command-palette").press("Escape")
      await expectClosed(ide.main.getByTestId("command-palette"))
    } finally {
      await ide.close()
    }
  })

  test("the desktop window and the IDE view of one repository show one Landed island, which the sight of the desktop window hides in both", async ({
    page,
    browser,
    setupRepo,
    syncAndWaitForBranches,
  }) => {
    const { repo, card } = await updateIntoLandedIsland(page, setupRepo, syncAndWaitForBranches)
    const ide = await openIdeOn(browser, repo)
    try {
      const ideCard = ide.main.getByTestId("landed-card")
      await expect(ideCard).toBeVisible()
      // the desktop window saw it long enough on the clock of the session: the next sync hides it for the repository,
      // in the IDE too
      await repo.advanceBackendClock(11_000)
      await syncAndWaitForBranches()
      await expect(card).toHaveCount(0)
      await expect(ideCard).toHaveCount(0)
    } finally {
      await ide.close()
    }
  })

  test("the desktop window and the IDE view of one repository say the same estimate of a running robot", async ({ page, browser, setupRepo, syncAndWaitForBranches }) => {
    const LSC: PatronusRobotProfile = { id: "lsc", name: "Large Scale Change", skipsReview: null }
    // three Large Scale Changes of 110 minutes, of branches merged since: the session estimates JIRA-456 by them
    const finished = (id: string) =>
      robot(`old-${id}`, `old-${id}`, { status: "success", sourceRevision: EARLIER_COMMIT, startTime: NOW - 86400, finishTime: NOW - 86400 + 110 * 60, profile: LSC })
    const repo = await setupRepo("issue_links", {
      patronus: availableRobots(robot("JIRA-456", "lsc-robot", { startTime: NOW - 18 * 60, profile: LSC }), finished("a"), finished("b"), finished("c")),
    })
    await syncAndWaitForBranches()
    const chip = findBranchRow(page, "JIRA-456").getByTestId("patronus-chip")
    await expect(chip).toHaveText("18m · ~1h 32m")
    const ide = await openIdeOn(browser, repo)
    try {
      const ideChip = ide.main.locator('[data-testid="branch-row"][data-branch-name="JIRA-456"]').getByTestId("patronus-chip")
      await expect(ideChip).toHaveText("18m · ~1h 32m")
    } finally {
      await ide.close()
    }
  })
})
