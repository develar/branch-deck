// @spec spec/repository/onboarding.spec.md
import type { Page } from "@playwright/test"
import { test, expect } from "../fixtures/test-fixtures"
import { branchTree, findBranchRow, releaseTheSyncOfTheUnstack, unstackOauthFromMenuWaiting } from "../helpers/branch-helpers"
import { openPalette, palette } from "../helpers/command-palette-helpers"
import { expectWaitingForTheFirstSync, releaseTheFirstSync, reloadWarm } from "../helpers/common-setup"
import type { FirstRowsIpc } from "../helpers/ipc-helpers"
import { firstRowsAsks, recordIpcAtFirstRows } from "../helpers/ipc-helpers"
import { availablePullRequests, stackPullRequest, withStackPullRequests } from "../helpers/pull-request-helpers"
import { openBranchPreview } from "../helpers/review-status-helpers"
import { openContextMenu } from "../helpers/selection-helpers"
import { emitTauriEvent, finishedSyncCount } from "../helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../helpers/test-repository"
import { holdCommand, recordRewrites, waitForStorePersistence } from "../helpers/wait-helpers"

/** The rows came with the subscription the window made holding the seed and its channel, nothing else asked before them */
async function expectRowsWithTheSubscriptionAlone(page: Page): Promise<void> {
  const asks = await firstRowsAsks(page)
  expect(asks?.invokes).toEqual({ subscribe_view: 1 })
  expect(Object.keys(asks?.channelMessages ?? {}).filter(command => command !== "subscribe_view")).toEqual([])
}

/** What the syncs the page recorded wrote of each repository (`repositoryState`, `recordSynced`) */
type LastSyncs = Record<string, { readonly lastSyncTime?: number; readonly lastBranchCount?: number }>

/** The `repositoryState` the page wrote so far in this test, by repository */
async function lastSyncTimes(page: Page): Promise<LastSyncs> {
  return page.evaluate(() => ((JSON.parse(sessionStorage.getItem("bd-e2e-store-writes") ?? "{}") as Record<string, unknown>).repositoryState ?? {}) as LastSyncs)
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

  test("handles the end of a first sync that committed before the subscription answered as the end of any run", async ({ page, setupRepo }) => {
    // the subscription of the page reaches the backend only once the first sync of the session committed: the catch-up
    // from the seed is idle on both sides
    const releaseSubscription = await holdCommand(page, "subscribe_view")
    const repo = await startWarmWithTheSyncHeld(page, setupRepo)
    const syncedBefore = (await lastSyncTimes(page))[repo.path!]
    expect(syncedBefore).toBeUndefined()
    await repo.holdRemoteAnswers({ release: ["sync"] })
    await expect.poll(async () => (await repo.sessionView())?.cached).toBe(false)
    expect((await repo.sessionView())?.sync).toEqual({ kind: "idle" })

    releaseSubscription()
    await expect(page.locator("[data-cached]")).toHaveCount(0)
    await expect(page.getByTestId("sync-button")).toHaveAttribute("data-sync-count", "1")
    // the window records the run it never saw: the time of the sync and the room of the loading table at the next start
    await waitForStorePersistence(page, "repositoryState", states => (states as LastSyncs | undefined)?.[repo.path!]?.lastBranchCount === FRAME_TREE.length)
    expect(await branchTree(page)).toEqual(FRAME_TREE)
  })

  test("ends the first sync once: a reload of the window after it ends no run", async ({ page, setupRepo }) => {
    const repo = await startWarmWithTheSyncHeld(page, setupRepo)
    await releaseTheFirstSync(page, repo)
    await expect(page.getByTestId("sync-button")).toHaveAttribute("data-sync-count", "1")

    // a reload (F5, a dev reload): the init script hands the page the same seed, whose end the page before it handled
    await page.reload()
    await expect(findBranchRow(page, "oauth")).toBeVisible()
    // the catch-up from the seed to the table of this run, whose end the page before the reload saw: no run ended here
    await expect(page.locator("[data-cached]")).toHaveCount(0)
    await expect(page.getByTestId("sync-button")).toHaveAttribute("data-sync-count", "0")
    expect(await branchTree(page)).toEqual(FRAME_TREE)
  })

  test("says the close of a pull request of a stack that is no more waits for the first sync on the rows of the frame", async ({ page, setupRepo }) => {
    // docs heads no stack, and its stack branch still has #12: the frame keeps the pull requests as GitHub listed them
    const pullRequests = withStackPullRequests(availablePullRequests(), { docs: stackPullRequest("docs", 12) })
    const repo = await setupRepo("stacked_branches", { warmStart: true, pullRequests })
    await expect.poll(async () => (await repo.sessionView())?.facts.pullRequests.stackPullRequests.docs?.number).toBe(12)
    await repo.holdRemoteAnswers({ hold: ["sync"] })
    await reloadWarm(page, repo)
    await expect(page.locator("[data-cached=true]")).toBeVisible()

    const preview = await openBranchPreview(page, "docs")
    const notice = preview.getByTestId("orphaned-stack-pull-request")
    await expect(notice).toContainText("#12 no longer matches the docs stack: it no longer forks.")
    const close = notice.getByRole("button", { name: "Close Pull Request #12…" })
    await expect(close).toBeDisabled()
    // the hover of a disabled button is its wrapper's
    await close.locator("..").hover()
    await expect(page.locator("[data-slot=tooltip-content][data-open]", { hasText: "Waits for the first sync" })).toBeVisible()

    await releaseTheFirstSync(page, repo)
    await expect(close).toBeEnabled()
    await close.click()
    await expect(preview).toHaveCount(0)
    const confirm = page.locator('tr[data-branch-of="docs"]').filter({ has: page.getByRole("button", { name: "Close Pull Request", exact: true }) })
    await expect(confirm).toContainText("Close pull request #12")
  })

  test("says a push of the rows of the frame waits for the first sync, and offers it once the sync committed", async ({ page, setupRepo }) => {
    const repo = await startWarmWithTheSyncHeld(page, setupRepo)
    await openContextMenu(page, findBranchRow(page, "docs"))
    // the one commit of docs to push, and its key
    const push = page.getByRole("menuitem", { name: /^Push Branch \(1\)/ })
    await expectWaitingForTheFirstSync(page, push)

    await releaseTheFirstSync(page, repo)
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
    // the Unstack of its menu, which runs, says it as its row does: in the palette of the row
    await findBranchRow(page, "oauth").focus()
    await openPalette(page)
    const unstack = palette(page)
      .getByRole("group")
      .filter({ hasText: /^oauth/ })
      .getByRole("option", { name: /^Unstack from login/ })
    await expect(unstack).toHaveAttribute("aria-disabled", "true")
    await expect(unstack).toContainText("Unstacking oauth after the sync…")
    await page.keyboard.press("Escape")
    await expect(palette(page)).toHaveCount(0)

    await releaseTheSyncOfTheUnstack(page, repo)
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
    expect(((await (await focus).json()) as { outcome: string }).outcome).toBe("unchanged")
    await expect(page.getByTestId("sync-button")).toHaveAttribute("data-sync-count", "1")
    await expect(page.getByTestId("sync-button")).toBeEnabled()
    expect(await finishedSyncCount(page)).toBe(1)
    // the check came right after the sync: the header says the sync, not the check, also once both aged
    await page.clock.fastForward("01:15")
    await expect(page.getByTestId("last-sync-time")).toHaveText("Last synced 1 minute ago")
  })
})
