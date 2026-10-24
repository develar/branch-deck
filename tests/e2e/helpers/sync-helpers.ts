import type { Page } from "@playwright/test"
import { expect } from "@playwright/test"
import type { RemoteStatusUpdate, SyncEvent } from "../../../src/lib/bindings"

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
  const syncResponse = page.waitForResponse(response => response.url().endsWith("/invoke/sync_branches"))
  await emitTauriEvent(page, "sync_branches")
  await syncResponse
  await expect(page.locator('[data-testid="sync-button"]')).toBeEnabled()
}

/**
 * Gives the window the focus and waits for the auto-sync it starts, which a focus starts only when what a sync reads
 * changed (a reword behind the app's back)
 */
export async function syncOnWindowFocus(page: Page): Promise<void> {
  const syncResponse = page.waitForResponse(response => response.url().endsWith("/invoke/sync_branches"))
  await emitTauriEvent(page, "tauri://focus")
  await syncResponse
  await expect(page.locator('[data-testid="sync-button"]')).toBeEnabled()
}

/**
 * Rewrites the events of the next sync (`sync_branches`, the event stream of the test server) through `rewrite`: null
 * leaves the event out. The syncs after it are the server's as they are.
 */
export async function rewriteNextSyncEvents(page: Page, rewrite: (event: SyncEvent) => SyncEvent | null): Promise<void> {
  await page.route(
    "**/invoke/sync_branches",
    async route => {
      const response = await route.fetch()
      const blocks = (await response.text()).split("\n\n").flatMap(block => {
        const data = block.split("\n").find(line => line.startsWith("data: "))
        if (data === undefined) {
          return [block]
        }
        const event = JSON.parse(data.slice("data: ".length)) as SyncEvent
        const rewritten = rewrite(event)
        // an event kept as it is keeps its block
        return rewritten === null ? [] : rewritten === event ? [block] : [`event: sync\ndata: ${JSON.stringify(rewritten)}`]
      })
      await route.fulfill({ response, body: blocks.join("\n\n") })
    },
    { times: 1 },
  )
}

/**
 * Rewrites the remote status events of the next sync (`sync_branches`, the event stream of the test server, which
 * computes them from a remote the templates don't have) through `rewrite`: null leaves the event out, so the remote
 * status of that branch stays unloaded. The syncs after it are the server's as they are.
 */
export async function rewriteNextRemoteStatus(page: Page, rewrite: (status: RemoteStatusUpdate) => RemoteStatusUpdate | null): Promise<void> {
  await rewriteNextSyncEvents(page, event => {
    if (event.type !== "remoteStatusUpdate") {
      return event
    }
    const status = rewrite(event.data)
    return status === null ? null : { ...event, data: status }
  })
}

/** `branch` on the remote as it is, as the remote status of the next sync says */
export async function pushedOnNextSync(page: Page, branch: string): Promise<void> {
  await rewriteNextRemoteStatus(page, status =>
    status.branchName === branch ? { ...status, remoteExists: true, unpushedCommits: [], myUnpushedCount: 0, commitsBehind: 0, fastForward: true } : status,
  )
}

/** `branch` on the remote with one commit the remote lacks, as the remote status of the next sync says */
export async function oneCommitUnpushedOnNextSync(page: Page, branch: string): Promise<void> {
  await rewriteNextRemoteStatus(page, status =>
    status.branchName === branch
      ? { ...status, remoteExists: true, unpushedCommits: [status.unpushedCommits[0] ?? "new"], myUnpushedCount: 1, commitsBehind: 0, fastForward: true }
      : status,
  )
}
