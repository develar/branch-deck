// @spec spec/repository/onboarding.spec.md
import type { Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { branchTree, findBranchRow, unstackOauthFromMenuWaiting } from "../helpers/branch-helpers"
import { reloadWarm } from "../helpers/common-setup"
import { openContextMenu } from "../helpers/selection-helpers"
import { emitTauriEvent, finishedSyncCount } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { recordRewrites } from "../helpers/wait-helpers"

/** What the page asked of the backend when its first branch row came into the document (`__ipcStats` of the mock) */
interface FirstRowsIpc {
  readonly invokes: Record<string, number>
  readonly channelMessages: Record<string, number>
}

/** Records the IPC of the page at its first branch row, for every load from now on (`firstRowsAsks`) */
async function recordIpcAtFirstRows(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const observer = new MutationObserver(() => {
      if (document.querySelector('[data-testid="branch-row"]') === null) {
        return
      }
      observer.disconnect()
      const stats = (window as unknown as { __ipcStats: FirstRowsIpc }).__ipcStats
      Object.assign(window, { __e2eFirstRowsIpc: { invokes: { ...stats.invokes }, channelMessages: { ...stats.channelMessages } } })
    })
    observer.observe(document, { childList: true, subtree: true })
  })
}

/**
 * What every page tells the backend as it loads, which reads nothing of the repository: that it loaded (the show of
 * the window), the changes of the settings after the snapshot it was built with, and the demand of its subscription
 * once the subscription answered
 */
const PAGE_PROTOCOL: ReadonlySet<string> = new Set(["sub_window_loaded", "get_settings_changes", "set_demand"])

/** The commands the page asked before its first branch row, beside `PAGE_PROTOCOL`, and the channels of their answers */
async function firstRowsAsks(page: Page): Promise<FirstRowsIpc | null> {
  const ipc = await page.evaluate(() => (window as unknown as { __e2eFirstRowsIpc?: FirstRowsIpc }).__e2eFirstRowsIpc ?? null)
  return ipc === null ? null : { ...ipc, invokes: Object.fromEntries(Object.entries(ipc.invokes).filter(([command]) => !PAGE_PROTOCOL.has(command))) }
}

/** The rows came with the subscription the window made holding the seed and its channel, nothing else asked before them */
async function expectRowsWithTheSubscriptionAlone(page: Page): Promise<void> {
  const asks = await firstRowsAsks(page)
  expect(asks?.invokes).toEqual({ subscribe_view: 1 })
  expect(Object.keys(asks?.channelMessages ?? {}).filter(command => command !== "subscribe_view")).toEqual([])
}

/** The rows of `stacked_branches` as the frame of its last sync has them */
const FRAME_TREE = ["docs@0", "auth@0", "session@1", "login@1", "oauth@2"]

/**
 * The app starts warm on `stacked_branches` with its first sync held in the session: the rows of the frame show, the
 * table marked cached
 */
async function startWarmWithTheSyncHeld(page: Page, setupRepo: (template: string, options: object) => Promise<TestRepositoryBuilder>): Promise<TestRepositoryBuilder> {
  const repo = await setupRepo("stacked_branches", { warmStart: true, remoteAnswers: { hold: ["sync"] }, waitForSync: false })
  await expect(page.locator("[data-cached=true]")).toBeVisible()
  expect(await branchTree(page)).toEqual(FRAME_TREE)
  return repo
}

test.describe("The warm start", () => {
  test("shows the rows of the frame before the first sync, asking only for the subscription, and the first sync's table after it", async ({ page, setupRepo }) => {
    await recordIpcAtFirstRows(page)
    const repo = await startWarmWithTheSyncHeld(page, setupRepo)
    await expectRowsWithTheSubscriptionAlone(page)
    // the first sync runs, held: no run ended in this window
    await expect(page.getByTestId("sync-button")).toHaveAttribute("data-sync-count", "0")
    await expect(page.getByTestId("ready-to-sync-card")).toHaveCount(0)

    await repo.holdRemoteAnswers({ release: ["sync"] })
    await expect(page.getByTestId("sync-button")).toHaveAttribute("data-sync-count", "1")
    await expect(page.locator("[data-cached]")).toHaveCount(0)
    expect(await branchTree(page)).toEqual(FRAME_TREE)
    // after the rows too, the page reads no prefix of its own: the view of the session names it
    const asked = await page.evaluate(() => Object.keys((window as unknown as { __ipcStats: FirstRowsIpc }).__ipcStats.invokes))
    expect(asked).toContain("subscribe_view")
    expect(asked).not.toContain("resolve_branch_prefix")

    // the next start shows the frame the sync saved, the same way: with its first sync held, as a sync that ends before
    // the rows are in the document is recorded by the page (`set_setting` of `repositoryState`) before them
    await repo.holdRemoteAnswers({ hold: ["sync"] })
    await reloadWarm(page, repo)
    await expect(findBranchRow(page, "oauth")).toBeVisible()
    await expect(page.locator("[data-cached=true]")).toBeVisible()
    await expectRowsWithTheSubscriptionAlone(page)
    await repo.holdRemoteAnswers({ release: ["sync"] })
    await expect(page.getByTestId("sync-button")).toHaveAttribute("data-sync-count", "1")
    await expect(page.locator("[data-cached]")).toHaveCount(0)
  })

  test("says a push of the rows of the frame waits for the first sync, and offers it once the sync committed", async ({ page, setupRepo }) => {
    const repo = await startWarmWithTheSyncHeld(page, setupRepo)
    await openContextMenu(page, findBranchRow(page, "docs"))
    // the one commit of docs to push, and its key
    const push = page.getByRole("menuitem", { name: /^Push Branch \(1\)/ })
    await expect(push).toHaveAttribute("aria-disabled", "true")
    await push.hover()
    await expect(page.locator("[data-slot=tooltip-content][data-open]", { hasText: "Waits for the first sync" })).toBeVisible()
    await page.keyboard.press("Escape")

    await repo.holdRemoteAnswers({ release: ["sync"] })
    await expect(page.locator("[data-cached]")).toHaveCount(0)
    await openContextMenu(page, findBranchRow(page, "docs"))
    await expect(push).toBeVisible()
    await expect(push).not.toHaveAttribute("aria-disabled", "true")
  })

  test("runs an unstack pressed on the rows of the frame once the first sync committed, waiting for it until then", async ({ page, setupRepo }) => {
    const repo = await startWarmWithTheSyncHeld(page, setupRepo)
    const unstacks = await recordRewrites(page, "unstack")
    // the row says it waits for the sync, and nothing moved
    const more = await unstackOauthFromMenuWaiting(page, "Unstacking oauth after the sync")
    expect(await branchTree(page)).toEqual(FRAME_TREE)
    await expect.poll(() => unstacks.length).toBe(1)

    await repo.holdRemoteAnswers({ release: ["sync"] })
    await expect(page.getByText(/^Unstacked oauth/)).toBeVisible()
    await expect.poll(() => branchTree(page)).toContain("oauth@0")
    await expect(more).not.toHaveAttribute("aria-busy")
    // the first sync, then the one of the unstack: the press reached the backend once
    expect(await finishedSyncCount(page)).toBe(2)
    expect(unstacks).toHaveLength(1)
  })

  test("checks a focus during the first sync once after it, with no second run", async ({ page, setupRepo }) => {
    const repo = await startWarmWithTheSyncHeld(page, setupRepo)
    const isFocus = (url: string, body: unknown) => url.endsWith("/invoke/set_window_focus") && (body as { focused: boolean }).focused
    const asked = page.waitForRequest(request => isFocus(request.url(), request.postDataJSON()))
    const focus = page.waitForResponse(response => isFocus(response.url(), response.request().postDataJSON()))
    await emitTauriEvent(page, "tauri://focus")
    // the focus reached the session, which answers it after the run it is checked behind
    await asked

    await repo.holdRemoteAnswers({ release: ["sync"] })
    await focus
    await expect(page.getByTestId("sync-button")).toHaveAttribute("data-sync-count", "1")
    await expect(page.getByTestId("sync-button")).toBeEnabled()
    expect(await finishedSyncCount(page)).toBe(1)
  })
})
