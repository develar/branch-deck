import type { Page } from "@playwright/test"
import { expect } from "@playwright/test"
import { findBranchRow } from "./branch-helpers"
import type { SyncOverrides, TestRepositoryBuilder } from "./test-repository"

/** The epoch of the view the main window holds, from `data-view-version` (`epoch:seq`) of Sync; undefined without one */
export async function viewEpoch(page: Page): Promise<string | undefined> {
  return (await page.getByTestId("sync-button").getAttribute("data-view-version"))?.split(":")[0]
}

/**
 * Waits until the main window holds another session than the one of `epoch`, the session of `repo` under the prefix
 * `team`, and synced it once as an opened repository: test-branch shows, Sync is idle, the one sync the window asked
 * since `syncs` began recording is `open`, and the branch is built under the new prefix
 */
export async function expectTeamSessionSyncedOnOpen(page: Page, repo: TestRepositoryBuilder, epoch: string | undefined, syncs: Record<string, unknown>[]): Promise<void> {
  await expect.poll(() => viewEpoch(page)).not.toBe(epoch)
  await expect(findBranchRow(page, "test-branch")).toBeVisible()
  await expect(page.getByTestId("sync-button")).toBeEnabled()
  expect(syncs).toEqual([{ repositoryPath: repo.path, reason: "open" }])
  expect(await repo.revision("team/virtual/test-branch")).toMatch(/^[0-9a-f]{40}$/)
}

/**
 * Sync branches and wait for specific elements to appear
 * @param waitForSelectors - Optional selectors to wait for after starting sync
 */
export async function syncBranches(page: Page, waitForSelectors?: string[]): Promise<void> {
  // Click sync button to load branches
  const syncButton = page.locator('[data-testid="sync-button"]')

  // Wait for the button to be enabled
  await expect(syncButton).toBeEnabled()

  await syncButton.click()

  // If specific selectors are provided, wait for them
  if (waitForSelectors && waitForSelectors.length > 0) {
    // Wait for all specified selectors in parallel
    await Promise.all(
      waitForSelectors.map(selector =>
        page.waitForSelector(selector, { timeout: 15000 }).catch(err => {
          console.error(`[Sync] Failed to find selector: ${selector}`, err.message)
          throw err
        }),
      ),
    )
    // what the selectors name comes with the first patches of the sync: the rest moves it until the sync ends
    await expect(syncButton).toBeEnabled({ timeout: 15000 })
  } else {
    // Default behavior: wait for sync to complete
    // Wait for sync to complete (button becomes enabled again)
    await expect(syncButton).toBeEnabled({ timeout: 15000 })

    // Wait for sync results - look for either branch rows or unassigned commits
    // This replaces the need for an arbitrary timeout as it waits for actual content
    await page.waitForFunction(
      () => {
        const branchRows = document.querySelectorAll('[data-testid="branch-row"]').length
        const unassignedSection = Array.from(document.querySelectorAll("h2")).find(h => h.textContent?.includes("Unassigned Commits"))
        const emptyState = document.querySelector('[data-testid="empty-state-alert"]')
        console.log(
          `[Sync waitForFunction] Branch rows: ${branchRows}, Unassigned section: ${unassignedSection ? "found" : "not found"}, Empty state: ${emptyState ? "found" : "not found"}`,
        )
        return branchRows > 0 || unassignedSection !== null || emptyState !== null
      },
      { timeout: 10000 },
    )
  }
}

/**
 * Wait for branches to finish loading
 */
export async function waitForBranchesLoaded(page: Page, timeout = 10000): Promise<void> {
  // Wait for branch rows to appear
  await page.waitForSelector('[data-testid="branch-row"]', { timeout })
}

/**
 * Wait for a specific branch to finish syncing (no progress bar)
 */
export async function waitForBranchSyncComplete(page: Page, branchName: string): Promise<void> {
  const branchRow = page.locator(`[data-testid="branch-row"][data-branch-name="${branchName}"]`)

  // Wait for the branch row to be visible
  await expect(branchRow).toBeVisible()

  // Wait for the progress bar to disappear (sync complete)
  const statusCell = branchRow.locator("td").nth(2) // Third cell is status
  await expect(statusCell.locator('[role="progressbar"]')).not.toBeVisible()

  // Now wait for the status badge to appear and verify it's not "syncing"
  const statusBadge = statusCell.getByTestId("branch-status")
  await expect(statusBadge).toBeVisible()

  // Verify the status is not "syncing" - it should be "created", "updated", etc.
  const statusText = await statusBadge.textContent()
  expect(statusText?.toLowerCase()).not.toBe("syncing")
}

/**
 * Get unassigned commits within the unassigned card
 */
export function getUnassignedCommits(page: Page) {
  // Find the unassigned commits card - it contains "Unassigned Commits" text
  const unassignedCard = page.locator(".overflow-hidden").filter({ hasText: "Unassigned Commits" })

  // Find commits within the unassigned card
  return unassignedCard.locator("[data-row-id]")
}

/** The number of finished syncs; read it before the action that starts the sync to wait for */
export async function finishedSyncCount(page: Page): Promise<number> {
  return Number(await page.locator('[data-testid="sync-button"]').getAttribute("data-sync-count"))
}

/**
 * Wait for a sync that finishes after `finishedSyncCount` returned `previousCount`. Watching the button go disabled
 * and enabled again misses a sync that is over before the first check, which happens under parallel load.
 */
export async function waitForSyncComplete(page: Page, previousCount: number, timeout = 10000): Promise<void> {
  const syncButton = page.locator('[data-testid="sync-button"]')
  await expect(syncButton).not.toHaveAttribute("data-sync-count", String(previousCount), { timeout })
  await expect(syncButton).toBeEnabled({ timeout })
}

/**
 * Runs an action that runs a command and then the sync of its result (a move, a rename, an undo), and waits for that
 * sync to finish: what the assertions after it read comes with the sync, and under load the command and the sync
 * together take longer than one assertion waits.
 */
export async function runAndWaitForSync(page: Page, action: () => Promise<unknown>): Promise<void> {
  const previousCount = await finishedSyncCount(page)
  await action()
  await waitForSyncComplete(page, previousCount)
}

/**
 * The `count`-th answer of the page's next `command` (`/invoke/<command>`), however long the test lets it take; ask for it before
 * the action that starts the command. It is the event of one step of a chain of commands, each of which takes seconds
 * under load (Update Project: the fetch, the update, then the sync), so the assertion after it waits only for the page
 * to show the answer, never for the whole chain within its 5 s
 */
export function nextAnswer(page: Page, command: string, count = 1): Promise<unknown> {
  let answered = 0
  return page.waitForResponse(response => response.url().endsWith(`/invoke/${command}`) && ++answered === count, { timeout: 0 })
}

/** Runs `action`, which starts `command` at the end of a chain of commands, and waits for the answer of that command */
export async function runAndWaitForAnswer(page: Page, command: string, action: () => Promise<unknown>): Promise<void> {
  const answered = nextAnswer(page, command)
  await action()
  await answered
}

/**
 * Presses Update Project with `press` (its button, the menu item, the palette) and waits for the answer of the update,
 * which comes after the fetch of the baseline and brings its toast and the state of the button; the sync of a clean
 * update follows it (`runAndWaitForSync` around this for an assertion on the rebuilt rows)
 */
export async function runAndWaitForUpdate(page: Page, press: () => Promise<unknown>): Promise<void> {
  await runAndWaitForAnswer(page, "update_project", press)
}

/** Waits until the page listens to `event`: one fired before a lazy page registered its listener is lost */
export async function waitForTauriListener(page: Page, event: string): Promise<void> {
  await expect
    .poll(() => page.evaluate(name => (window as unknown as { __e2eTauriListenerCount: (event: string) => number }).__e2eTauriListenerCount(name), event))
    .toBeGreaterThan(0)
}

/**
 * Fires the Tauri event listeners of the page mock, e.g. `tauri://focus` (the window gained focus, auto-sync listens to
 * it). `label` names the window of the event, for a listener of one window: `tauri://destroyed` of `pull-request-review`.
 */
export async function emitTauriEvent(page: Page, event: string, payload: unknown = null, label: string | null = null): Promise<void> {
  await page.evaluate(
    ([event, payload, label]) => {
      ;(window as unknown as { __e2eEmitTauriEvent: (event: string, payload: unknown, label: string | null) => void }).__e2eEmitTauriEvent(
        event as string,
        payload,
        label as string | null,
      )
    },
    [event, payload, label] as const,
  )
}

/**
 * Turns "Auto-sync on Focus" on, where a test turned it off (it is on by default), as its item of the File menu does:
 * the native menu sends its new state to the main window
 */
export async function enableAutoSyncOnFocus(page: Page): Promise<void> {
  await emitTauriEvent(page, "auto_sync_on_focus_toggled", true)
}

/** Syncs as the Sync menu item (⌘R) does, also under a dialog, and waits for that sync */
export async function syncFromMenu(page: Page): Promise<void> {
  const syncResponse = page.waitForResponse(response => response.url().endsWith("/invoke/sync"))
  await emitTauriEvent(page, "sync_branches")
  await syncResponse
  await expect(page.locator('[data-testid="sync-button"]')).toBeEnabled()
}

/** What the session answered a gained focus of the window (`set_window_focus`): null when it asked nothing */
export interface FocusAnswer {
  outcome: string
  version: { epoch: number; seq: number }
}

/**
 * Gives the window the focus and waits for what the session answers (`set_window_focus`, which syncs by Auto-sync on
 * Focus and only when what a sync reads changed, a reword behind the app's back) and for its last patch in the page
 */
export async function focusWindow(page: Page): Promise<FocusAnswer | null> {
  const answer = (await focusWindowAnswer(page)) as FocusAnswer | null
  if (answer !== null) {
    await waitForViewVersion(page, answer.version)
  }
  return answer
}

/**
 * Gives the window the focus and answers the error the session refused it with: a focus after a sync that failed or
 * waited for git, which finds what that sync read, answers its error without a run
 */
export async function focusWindowRefused(page: Page): Promise<{ code: string; message: string }> {
  const answer = (await focusWindowAnswer(page)) as { status?: string; error?: { code: string; message: string } } | null
  expect(answer?.status).toBe("error")
  return answer!.error!
}

/** Gives the window the focus and answers the body of the session's answer to it (`set_window_focus`) */
async function focusWindowAnswer(page: Page): Promise<unknown> {
  const focusResponse = page.waitForResponse(response => response.url().endsWith("/invoke/set_window_focus") && (response.request().postDataJSON() as { focused: boolean }).focused)
  await emitTauriEvent(page, "tauri://focus")
  return (await focusResponse).json()
}

/** Waits until the page holds the view of its session at `version` or past it (`data-view-version` of Sync) */
export async function waitForViewVersion(page: Page, version: { epoch: number; seq: number }): Promise<void> {
  const syncButton = page.locator('[data-testid="sync-button"]')
  await expect
    .poll(async () => {
      const shown = await syncButton.getAttribute("data-view-version")
      if (shown === null) {
        return false
      }
      const [epoch, seq] = shown.split(":").map(Number)
      return epoch !== version.epoch || seq! >= version.seq
    })
    .toBe(true)
  await expect(syncButton).toBeEnabled()
}

/** Gives the window the focus and waits for the sync it starts, which a focus starts only when what a sync reads changed */
export async function syncOnWindowFocus(page: Page): Promise<void> {
  const answer = await focusWindow(page)
  expect(answer?.outcome).toBe("synced")
}

/** What the next sync reports of `branch`: on the remote as it is (`SyncOverrides.remote`) */
export function pushedRemote(branch: string): SyncOverrides {
  return { remote: { [branch]: { remoteExists: true, unpushedCommits: [], myUnpushedCount: 0, commitsBehind: 0, fastForward: true } } }
}

/** What the next sync reports of `branch`: on the remote, which lacks only the newest of the commits the sync found unpushed */
export function oneCommitUnpushedRemote(branch: string): SyncOverrides {
  return { remote: { [branch]: { remoteExists: true, keepUnpushed: 1, myUnpushedCount: 1, commitsBehind: 0, fastForward: true } } }
}
