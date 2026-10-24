// The layout of the branch view at scale (docs/design/ui-theme.md#branch-columns): the `branch_table_large` template,
// 120 lone branches and three islands of seven, 141 rows whose columns line up across the cards, robots running on ten.
// A text that changes in one cell can change the width of a column of every card: the numbers say what a change costs,
// and decided the mechanism (measured tracks over tables, not one CSS subgrid: 8.5–12.7 ms a text change in WebKit).
//
//   pnpm perf:table                      3 runs, a table on stdout
//   PERF_RUNS=5 pnpm perf:table          more runs
//   pnpm perf:table --project=webkit     in WebKit, the engine of the app on macOS
//
// The table goes to tests/results/perf/branch-table-<project>.json too. Each run is a test of its own, in a page of its
// own: the setup of a repository keeps the page and the rows of the run before, and a sync onto them is no first sync.
// Each metric is the median of its runs:
// - firstRowsMs: from the sync's start until every branch row is in the page;
// - relayoutMs: one layout of the view after its width changed (a resize of the window);
// - widerTextMs: one layout after the status text of a row changed its width (a push's percent, "4%" to "45%"); when it
//   changes the widest item of a track, the frame adds one relayout of the view, as the columns follow it;
// - sameWidthTextMs: one layout after a text changed inside an element of a fixed width (a robot's time that ticks).
//
// The main thread of a sync (Chromium only, the CDP metrics of the page; docs/design/rendering-performance.md): what the
// page spends on the patches of a sync and what they derive, the hold and landing rule of the robots included:
// - firstSyncScriptMs / firstSyncTaskMs: the script time and the task time of the first sync, until its rows and chips;
// - resyncScriptMs / resyncTaskMs: the same of a sync that changes nothing;
// - robotIpc: the requests of the repository view (`get_repository_view`) during both syncs.
//
// The IPC of the page (every engine; `window.__ipcStats` of the Tauri mock, docs/design/e2e-testing.md), each count
// read once the page shows the state and no command is pending:
// - firstSyncPatches / firstSyncInvokes: the patches of the session the first sync sends the page (the channel of
//   `subscribe_view`) and the commands the page invokes until its rows and chips;
// - resyncIpcBytes: the bytes of the parameters, the answers and the patches of a sync that changes nothing;
// - hoverIpc: the commands a rest of the pointer on a branch row invokes (the review read ahead);
// - menuIpc: the commands the "⋯" menu of that row invokes, opened by a right click;
// - popoverIpc: the commands the popover of its robot invokes, opened by a hover of its chip.
// The patches and the bytes of a sync on the Rust side, without the page:
//   cargo nextest run -p test-server --run-ignored only -E 'test(/patch_bytes/)' --no-capture
import { mkdirSync, writeFileSync } from "node:fs"
import path from "node:path"
import type { CDPSession, Page } from "@playwright/test"
import { test, expect } from "../e2e/fixtures/test-fixtures"
import { findBranchRow } from "../e2e/helpers/branch-helpers"
import { availableRobots, robot } from "../e2e/helpers/patronus-helpers"
import { hoverStatusChip } from "../e2e/helpers/review-status-helpers"
import type { SetupRepoOptions } from "../e2e/helpers/setup-options"
import { runAndWaitForSync, syncBranches, waitForBranchesLoaded } from "../e2e/helpers/sync-helpers"

const RUNS = Number(process.env.PERF_RUNS ?? 3)
/** The branch rows of the template: 120 lone branches and three stacks of seven */
const ROWS = 141
/** The layouts timed per run of a metric, their mean is the run's value */
const LAYOUTS = 100

interface Metrics {
  firstRowsMs: number
  relayoutMs: number
  widerTextMs: number
  sameWidthTextMs: number
}

type LayoutKind = "relayout" | "widerText" | "sameWidthText"

/** In the page: the mean time of one layout after a change of `kind`, over `count` changes */
function timeLayouts({ kind, count }: { kind: LayoutKind; count: number }): number {
  const status = document.querySelector<HTMLElement>("tr[data-testid=branch-row] [data-testid=branch-status]")
  // the cards of the branch view, or the page where they stand side by side (a build before the grid)
  const view = document.querySelector<HTMLElement>("[data-testid=branch-view]") ?? document.querySelector<HTMLElement>("main")
  if (status === null || view === null) {
    throw new Error("no branch row with a status text")
  }
  if (kind === "sameWidthText") {
    status.style.display = "inline-block"
    status.style.width = "8rem"
  }
  void document.body.offsetHeight
  const start = performance.now()
  for (let index = 0; index < count; index++) {
    if (kind === "relayout") {
      view.style.width = index % 2 === 0 ? "90%" : ""
      void view.offsetWidth
    } else {
      status.textContent = index % 2 === 0 ? "pushing 4%" : "pushing 45%"
      void status.getBoundingClientRect().width
    }
  }
  const mean = (performance.now() - start) / count
  status.removeAttribute("style")
  view.removeAttribute("style")
  return mean
}

function timeIn(page: Page, kind: LayoutKind): Promise<number> {
  return page.evaluate(timeLayouts, { kind, count: LAYOUTS })
}

function median(values: number[]): number {
  const sorted = [...values].sort((a, b) => a - b)
  return Math.round(sorted[Math.floor(sorted.length / 2)]! * 100) / 100
}

const results: Record<string, Metrics | SyncMetrics | IpcMetrics> = {}

interface SyncMetrics {
  firstSyncScriptMs: number
  firstSyncTaskMs: number
  resyncScriptMs: number
  resyncTaskMs: number
  robotIpc: number
}

interface IpcMetrics {
  firstSyncPatches: number
  firstSyncInvokes: number
  resyncIpcBytes: number
  hoverIpc: number
  menuIpc: number
  popoverIpc: number
}

/** The runs of each benchmark, a test each */
const rowRuns: Metrics[] = []
const syncRuns: SyncMetrics[] = []
const ipcRuns: IpcMetrics[] = []

/** What the Tauri mock counts of the page's commands (`window.__ipcStats`, tests/e2e/setup/tauri-mocks.ts) */
interface IpcStats {
  invokes: Record<string, number>
  requestBytes: number
  responseBytes: number
  channelMessages: Record<string, number>
  channelBytes: number
  pending: number
}

type IpcWindow = { __ipcStats: IpcStats & { reset: () => void } }

/** Zeroes the counts of the page's IPC */
function resetIpc(page: Page): Promise<void> {
  return page.evaluate(() => (window as unknown as IpcWindow).__ipcStats.reset())
}

/** The IPC of the page since its last reset, once no command is pending: a command's answer that starts another counts */
async function settledIpc(page: Page): Promise<IpcStats> {
  await expect.poll(() => page.evaluate(() => (window as unknown as IpcWindow).__ipcStats.pending)).toBe(0)
  return page.evaluate(() => {
    const { invokes, requestBytes, responseBytes, channelMessages, channelBytes, pending } = (window as unknown as IpcWindow).__ipcStats
    return { invokes, requestBytes, responseBytes, channelMessages, channelBytes, pending }
  })
}

/** The commands of all names */
function invokeCount(stats: IpcStats): number {
  return Object.values(stats.invokes).reduce((sum, count) => sum + count, 0)
}

/** Past the rest of the pointer that counts as the intent to look at a row (`INTENT_DWELL_MS` of usePreviewHotkeys) */
const INTENT_DWELL_PAST_MS = 300

/** The template in the fresh page of a test, its robots listed, not synced yet: no row of a run before */
async function setupTable(page: Page, setupRepo: (template: string, options?: SetupRepoOptions) => Promise<unknown>): Promise<void> {
  await page.setViewportSize({ width: 1516, height: 900 })
  await setupRepo("branch_table_large", { patronus: availableRobots(...ROBOTS) })
  await expect(page.getByTestId("branch-row")).toHaveCount(0)
}

/** The script and task time of the page so far, ms (`Performance.getMetrics`: seconds) */
async function mainThreadMs(cdp: CDPSession): Promise<{ script: number; task: number }> {
  const { metrics } = await cdp.send("Performance.getMetrics")
  const valueOf = (name: string) => (metrics.find(metric => metric.name === name)?.value ?? 0) * 1000
  return { script: valueOf("ScriptDuration"), task: valueOf("TaskDuration") }
}

/** The script and task time `action` costs the page, ms */
async function timeMainThread(cdp: CDPSession, action: () => Promise<unknown>): Promise<{ script: number; task: number }> {
  const before = await mainThreadMs(cdp)
  await action()
  const after = await mainThreadMs(cdp)
  return { script: after.script - before.script, task: after.task - before.task }
}

/** The robots of the template: running on ten of its lone branches */
const ROBOTS = Array.from({ length: 10 }, (_, index) => robot(`feature-${String(index * 12).padStart(3, "0")}`, `robot-${index}`))

test.describe.configure({ mode: "serial" })

test.describe("Branch table performance", () => {
  test.afterAll(() => {
    if (rowRuns.length > 0) {
      results["141 rows"] = {
        firstRowsMs: median(rowRuns.map(run => run.firstRowsMs)),
        relayoutMs: median(rowRuns.map(run => run.relayoutMs)),
        widerTextMs: median(rowRuns.map(run => run.widerTextMs)),
        sameWidthTextMs: median(rowRuns.map(run => run.sameWidthTextMs)),
      }
    }
    if (syncRuns.length > 0) {
      results["sync main thread"] = {
        firstSyncScriptMs: median(syncRuns.map(run => run.firstSyncScriptMs)),
        firstSyncTaskMs: median(syncRuns.map(run => run.firstSyncTaskMs)),
        resyncScriptMs: median(syncRuns.map(run => run.resyncScriptMs)),
        resyncTaskMs: median(syncRuns.map(run => run.resyncTaskMs)),
        robotIpc: median(syncRuns.map(run => run.robotIpc)),
      }
    }
    if (ipcRuns.length > 0) {
      results["ipc"] = {
        firstSyncPatches: median(ipcRuns.map(run => run.firstSyncPatches)),
        firstSyncInvokes: median(ipcRuns.map(run => run.firstSyncInvokes)),
        resyncIpcBytes: median(ipcRuns.map(run => run.resyncIpcBytes)),
        hoverIpc: median(ipcRuns.map(run => run.hoverIpc)),
        menuIpc: median(ipcRuns.map(run => run.menuIpc)),
        popoverIpc: median(ipcRuns.map(run => run.popoverIpc)),
      }
    }
    console.table(results)
    const directory = path.join("tests", "results", "perf")
    mkdirSync(directory, { recursive: true })
    writeFileSync(path.join(directory, `branch-table-${test.info().project.name}.json`), `${JSON.stringify(results, null, 2)}\n`)
  })

  for (let run = 1; run <= RUNS; run++) {
    test(`141 branch rows in a table and three islands, robots running on ten of them, run ${run}`, async ({ page, setupRepo }) => {
      await setupTable(page, setupRepo)
      const start = Date.now()
      await syncBranches(page)
      await waitForBranchesLoaded(page)
      await expect(page.getByTestId("branch-row")).toHaveCount(ROWS)
      const firstRowsMs = Date.now() - start
      await expect(page.getByTestId("patronus-chip")).toHaveCount(ROBOTS.length)
      rowRuns.push({
        firstRowsMs,
        relayoutMs: await timeIn(page, "relayout"),
        widerTextMs: await timeIn(page, "widerText"),
        sameWidthTextMs: await timeIn(page, "sameWidthText"),
      })
    })
  }

  for (let run = 1; run <= RUNS; run++) {
    test(`the main thread of a sync of 141 rows with robots running on ten of them, run ${run}`, async ({ page, setupRepo, browserName }) => {
      test.skip(browserName !== "chromium", "the CDP metrics are Chromium's")
      await setupTable(page, setupRepo)
      const cdp = await page.context().newCDPSession(page)
      await cdp.send("Performance.enable")
      await resetIpc(page)
      const first = await timeMainThread(cdp, async () => {
        await syncBranches(page)
        await expect(page.getByTestId("branch-row")).toHaveCount(ROWS)
        await expect(page.getByTestId("patronus-chip")).toHaveCount(ROBOTS.length)
      })
      const resync = await timeMainThread(cdp, () => runAndWaitForSync(page, () => page.getByTestId("sync-button").click()))
      const robotIpc = (await settledIpc(page)).invokes["get_repository_view"] ?? 0
      syncRuns.push({ firstSyncScriptMs: first.script, firstSyncTaskMs: first.task, resyncScriptMs: resync.script, resyncTaskMs: resync.task, robotIpc })
    })
  }

  for (let run = 1; run <= RUNS; run++) {
    test(`the IPC of a sync of 141 rows and of the hover, the menu and the robot popover of a row, run ${run}`, async ({ page, setupRepo }) => {
      await setupTable(page, setupRepo)
      await resetIpc(page)
      await syncBranches(page)
      await expect(page.getByTestId("branch-row")).toHaveCount(ROWS)
      await expect(page.getByTestId("patronus-chip")).toHaveCount(ROBOTS.length)
      const firstSync = await settledIpc(page)

      await resetIpc(page)
      await runAndWaitForSync(page, () => page.getByTestId("sync-button").click())
      const resync = await settledIpc(page)

      // a row with a robot: the hover of its chip rests on the row too, whose review was read ahead already
      const row = findBranchRow(page, "feature-000")
      await resetIpc(page)
      await row.hover()
      await page.clock.fastForward(INTENT_DWELL_PAST_MS)
      const hover = await settledIpc(page)

      await resetIpc(page)
      await row.click({ button: "right" })
      await expect(page.getByRole("menu")).toBeVisible()
      const menu = await settledIpc(page)
      await page.keyboard.press("Escape")
      await expect(page.getByRole("menu")).toHaveCount(0)

      await resetIpc(page)
      await hoverStatusChip(page, row.getByTestId("patronus-chip"), "patronus-popover")
      const popover = await settledIpc(page)

      ipcRuns.push({
        firstSyncPatches: firstSync.channelMessages["subscribe_view"] ?? 0,
        firstSyncInvokes: invokeCount(firstSync),
        resyncIpcBytes: resync.requestBytes + resync.responseBytes + resync.channelBytes,
        hoverIpc: invokeCount(hover),
        menuIpc: invokeCount(menu),
        popoverIpc: invokeCount(popover),
      })
    })
  }
})
