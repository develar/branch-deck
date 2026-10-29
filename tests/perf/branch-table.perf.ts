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
// - firstSyncLayouts / firstSyncLayoutMs and firstSyncStyles / firstSyncStyleMs: the layouts and the style recalcs of
//   the page in that time and what they took (`LayoutCount`, `LayoutDuration`, `RecalcStyleCount`,
//   `RecalcStyleDuration`); a layout or a recalc that a script forces (a read of a size) counts in its script time too,
//   and the task time beyond the three is mostly paint, the compositor's commit and the work of the browser outside them;
// - resyncScriptMs, resyncTaskMs, resyncLayouts, resyncLayoutMs, resyncStyles, resyncStyleMs: the same of a sync that
//   changes nothing (the Patronus view comes in the patches of the session: the page asks no view of the robots), once
//   no step waits for a fact (`waitForStepFacts`).
//
// The renders of a sync (every engine; `countRender` of src/lib/render-counts.ts, which counts only in a page whose init
// script set `window.__bdRenderCounts`, `recordRenders` of tests/e2e/helpers/render-helpers.ts), each read once the
// session settled the sync and the facts it derives over it (`waitForSettledSession`):
// - resyncRows: the branch rows a sync that changes nothing renders (B5: the status of each row changes, `Created` to
//   `Unchanged`);
// - oneBranchSyncRows: the branch rows a sync renders that found one more commit on one branch (B3: that row), with
//   oneBranchSyncTableRenders and oneBranchSyncIslandRenders, the renders of the table card and of the island cards in
//   that sync.
//
// The warm start (docs/design/rendering-performance.md, "Loading"): the app starts again on the frame its sessions
// saved, the robots and the landing needs the first page read included (`reloadWarm`), its first sync held until the
// rows of the frame are in the page:
// - warmFirstRowsMs: from the reload until every row of the frame is in the page;
// - warmFirstSyncScriptMs / warmFirstSyncTaskMs: the script time and the task time of the page from the reload until
//   it holds the view in which the session committed that sync and derived the view of its robots and their landing
//   needs over it (`get_snapshot` of the server, outside the page): the load, the rows of the frame, the answer of
//   Patronus and the first sync;
// - warmFirstSyncPatches / warmFirstSyncBytes: the patches of the session in that time and their bytes;
// - warmStartInvokes: the commands the page invoked before its first row, its protocol aside (`firstRowsAsks`): the
//   subscription alone, whose channel carries the rest (B7).
//
// PERF_PROFILE=1 adds one more first sync under the CPU profiler of CDP (Chromium; sampled every 100 µs), after the
// measured runs, which it would slow: tests/results/perf/first-sync-<project>.cpuprofile, for the Performance panel of
// DevTools or a summary by function (the build has no source maps: map its positions with a `vp build --sourcemap` of
// the same commit).
//
// The IPC of the page (every engine; `window.__ipcStats` of the Tauri mock, docs/design/e2e-testing.md), each count
// read once the page shows the state and no command is pending, the commands of the app's own timers left out
// (`withoutAppTimers`: the check for updates after the start, the writes of the settings):
// - firstSyncPatches / firstSyncInvokes: the patches of the session the first sync sends the page (the channel of
//   `subscribe_view`) and the commands the page invokes until its rows and chips;
// - resyncIpcBytes: the bytes of the parameters, the answers and the patches of a sync that changes nothing, once no
//   step waits for a fact;
// - hoverIpc: the commands a rest of the pointer on a branch row invokes (the review read ahead);
// - menuIpc: the commands the menu of that row invokes, opened by a right click (the one menu of the table);
// - popoverIpc: the commands the popover of its robot invokes, opened by a hover of its chip.
// The bytes of the patches of the view on the Rust side, without the page, are budgets of their own (the first patch,
// one robot that fails, one readiness answer; crates/repository-model/src/patch_bytes_test.rs prints them):
//   cargo nextest run -p repository-model -E 'test(/patch_bytes/)' --no-capture
import { mkdirSync, writeFileSync } from "node:fs"
import path from "node:path"
import type { CDPSession, Page } from "@playwright/test"
import type { Version } from "../../src/lib/bindings"
import { test, expect } from "../e2e/fixtures/test-fixtures"
import { findBranchRow } from "../e2e/helpers/branch-helpers"
import { reloadWarm } from "../e2e/helpers/common-setup"
import type { IpcStats } from "../e2e/helpers/ipc-helpers"
import { INTENT_DWELL_PAST_MS, firstRowsAsks, recordIpcAtFirstRows, resetIpc, settledIpc, withoutAppTimers } from "../e2e/helpers/ipc-helpers"
import { availableRobots, robot } from "../e2e/helpers/patronus-helpers"
import { recordRenders, renderCounts, renderedKeys, resetRenders } from "../e2e/helpers/render-helpers"
import type { RenderCounts } from "../e2e/helpers/render-helpers"
import { hoverStatusChip } from "../e2e/helpers/review-status-helpers"
import type { SetupRepoOptions } from "../e2e/helpers/setup-options"
import { runAndWaitForSync, syncBranches, waitForBranchesLoaded, waitForViewVersion } from "../e2e/helpers/sync-helpers"
import type { TestRepositoryBuilder } from "../e2e/helpers/test-repository"

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

/** The median of each metric over the runs of a benchmark */
function medians(runs: readonly object[]): Record<string, number> {
  return Object.fromEntries(Object.keys(runs[0]!).map(key => [key, median(runs.map(run => (run as Record<string, number>)[key]!))]))
}

/** What the main thread of the page spent, its times in ms, and the layouts and style recalcs it ran */
interface MainThread {
  scriptMs: number
  taskMs: number
  layouts: number
  layoutMs: number
  styles: number
  styleMs: number
}

/** The main thread of one sync, each metric named after the sync (`firstSyncScriptMs`, `resyncLayouts`) */
type SyncMainThread<Sync extends string> = { [Key in keyof MainThread as `${Sync}${Capitalize<Key>}`]: number }

type SyncMetrics = SyncMainThread<"firstSync"> & SyncMainThread<"resync">

function syncMainThread<Sync extends string>(sync: Sync, mainThread: MainThread): SyncMainThread<Sync> {
  return Object.fromEntries(Object.entries(mainThread).map(([key, value]) => [`${sync}${key[0]!.toUpperCase()}${key.slice(1)}`, value])) as SyncMainThread<Sync>
}

interface RenderMetrics {
  resyncRows: number
  oneBranchSyncRows: number
  oneBranchSyncTableRenders: number
  oneBranchSyncIslandRenders: number
}

interface IpcMetrics {
  firstSyncPatches: number
  firstSyncInvokes: number
  resyncIpcBytes: number
  hoverIpc: number
  menuIpc: number
  popoverIpc: number
}

interface WarmMetrics {
  warmFirstRowsMs: number
  warmFirstSyncScriptMs: number
  warmFirstSyncTaskMs: number
  warmFirstSyncPatches: number
  warmFirstSyncBytes: number
  warmStartInvokes: number
}

/** The runs of each benchmark, a test each */
const rowRuns: Metrics[] = []
const syncRuns: SyncMetrics[] = []
const renderRuns: RenderMetrics[] = []
const ipcRuns: IpcMetrics[] = []
const warmRuns: WarmMetrics[] = []

/** The commands of all names but those of the app's own timers, which fire wherever a run is (as the e2e of B6 counts) */
function invokeCount(stats: IpcStats): number {
  return Object.values(withoutAppTimers(stats.invokes)).reduce((sum, count) => sum + count, 0)
}

/** The template in the fresh page of a test, its robots listed, not synced yet: no row of a run before */
async function setupTable(page: Page, setupRepo: (template: string, options?: SetupRepoOptions) => Promise<TestRepositoryBuilder>): Promise<TestRepositoryBuilder> {
  await page.setViewportSize({ width: 1516, height: 900 })
  const repo = await setupRepo("branch_table_large", { patronus: availableRobots(...ROBOTS) })
  await expect(page.getByTestId("branch-row")).toHaveCount(0)
  return repo
}

/** The main thread of the page so far (`Performance.getMetrics`: its durations in seconds) */
async function mainThreadSoFar(cdp: CDPSession): Promise<MainThread> {
  const { metrics } = await cdp.send("Performance.getMetrics")
  const valueOf = (name: string) => metrics.find(metric => metric.name === name)?.value ?? 0
  return {
    scriptMs: valueOf("ScriptDuration") * 1000,
    taskMs: valueOf("TaskDuration") * 1000,
    layouts: valueOf("LayoutCount"),
    layoutMs: valueOf("LayoutDuration") * 1000,
    styles: valueOf("RecalcStyleCount"),
    styleMs: valueOf("RecalcStyleDuration") * 1000,
  }
}

/** What `action` costs the main thread of the page */
async function timeMainThread(cdp: CDPSession, action: () => Promise<unknown>): Promise<MainThread> {
  const before = await mainThreadSoFar(cdp)
  await action()
  const after = await mainThreadSoFar(cdp)
  return {
    scriptMs: after.scriptMs - before.scriptMs,
    taskMs: after.taskMs - before.taskMs,
    layouts: after.layouts - before.layouts,
    layoutMs: after.layoutMs - before.layoutMs,
    styles: after.styles - before.styles,
    styleMs: after.styleMs - before.styleMs,
  }
}

/**
 * No step of a row waits for a fact any more (`data-waits-for` of the slot: the landing needs, which the session sends a
 * patch of the whole Patronus view for after the rows and chips): a sync measured after it is that sync alone
 */
async function waitForStepFacts(page: Page): Promise<void> {
  await expect(page.getByTestId("next-step-slot").and(page.locator("[data-waits-for]"))).toHaveCount(0)
}

/**
 * The session committed its first sync and derived the view of its robots and their landing needs over it, and the
 * page holds that view
 */
async function waitForSettledSession(page: Page, repo: TestRepositoryBuilder): Promise<void> {
  let settled: Version | null = null
  await expect
    .poll(async () => {
      // the view of the session as the server has it (`get_snapshot` in the app of the repository, outside the page)
      const view = await repo.sessionView()
      const fresh = view !== null && !view.cached && view.patronus.state.kind === "fresh" && view.patronus.landingNeeds.kind === "fresh"
      settled = fresh ? view.version : null
      return fresh
    })
    .toBe(true)
  await waitForViewVersion(page, settled!)
}

/**
 * Runs a sync from the sync button and waits until the session settled it and the facts it derives over it: the renders
 * of the page since the last reset (`resetRenders`)
 */
async function settledSyncRenders(page: Page, repo: TestRepositoryBuilder): Promise<RenderCounts> {
  await runAndWaitForSync(page, () => page.getByTestId("sync-button").click())
  await waitForSettledSession(page, repo)
  await waitForStepFacts(page)
  return renderCounts(page)
}

/** The robots of the template: running on ten of its lone branches */
const ROBOTS = Array.from({ length: 10 }, (_, index) => robot(`feature-${String(index * 12).padStart(3, "0")}`, `robot-${index}`))

test.describe.configure({ mode: "serial" })

test.describe("Branch table performance", () => {
  test.afterAll(() => {
    const benchmarks: [string, readonly object[]][] = [
      ["141 rows", rowRuns],
      ["sync main thread", syncRuns],
      ["renders", renderRuns],
      ["warm start", warmRuns],
      ["ipc", ipcRuns],
    ]
    const results: Record<string, Record<string, number>> = {}
    for (const [benchmark, runs] of benchmarks) {
      if (runs.length > 0) {
        results[benchmark] = medians(runs)
      }
    }
    // a table of each benchmark, its metrics as rows: the metrics of all of them would not fit as columns of one
    for (const [benchmark, metrics] of Object.entries(results)) {
      console.log(benchmark)
      console.table(metrics)
    }
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
      await waitForStepFacts(page)
      const resync = await timeMainThread(cdp, () => runAndWaitForSync(page, () => page.getByTestId("sync-button").click()))
      syncRuns.push({ ...syncMainThread("firstSync", first), ...syncMainThread("resync", resync) })
    })
  }

  for (let run = 1; run <= RUNS; run++) {
    test(`the rows a sync of 141 rows renders, of nothing changed and of one more commit on one branch, run ${run}`, async ({ page, setupRepo }) => {
      await recordRenders(page)
      const repo = await setupTable(page, setupRepo)
      await syncBranches(page)
      await expect(page.getByTestId("branch-row")).toHaveCount(ROWS)
      await expect(page.getByTestId("patronus-chip")).toHaveCount(ROBOTS.length)
      await waitForSettledSession(page, repo)
      await waitForStepFacts(page)

      await resetRenders(page)
      const resync = await settledSyncRenders(page, repo)

      // a commit of one lone branch in a terminal: its row shows two commits after the sync
      await resetRenders(page)
      await repo.commit("(feature-050) Keep the selection of the tree when the files of the module change", { "feature-050-tree.txt": "tree\n" }, 1704202800)
      const oneBranch = await settledSyncRenders(page, repo)
      await expect(findBranchRow(page, "feature-050").getByText("2", { exact: true })).toBeVisible()

      renderRuns.push({
        resyncRows: renderedKeys(resync, "row:").length,
        oneBranchSyncRows: renderedKeys(oneBranch, "row:").length,
        oneBranchSyncTableRenders: oneBranch["table"] ?? 0,
        oneBranchSyncIslandRenders: renderedKeys(oneBranch, "island:").reduce((sum, key) => sum + oneBranch[key]!, 0),
      })
    })
  }

  for (let run = 1; run <= RUNS; run++) {
    test(`the warm start of 141 rows with robots running on ten of them, run ${run}`, async ({ page, setupRepo, browserName }) => {
      test.skip(browserName !== "chromium", "the CDP metrics are Chromium's")
      await page.setViewportSize({ width: 1516, height: 900 })
      // the first page: the frame of a sync without a page, the robots and the landing needs read by its first sync
      const repo = await setupRepo("branch_table_large", { warmStart: true, patronus: availableRobots(...ROBOTS), waitForSync: false })
      await expect(page.getByTestId("branch-row")).toHaveCount(ROWS)
      await expect(page.locator("[data-cached]")).toHaveCount(0)
      await expect(page.getByTestId("patronus-chip")).toHaveCount(ROBOTS.length)
      await waitForStepFacts(page)

      // the next start, on the frame the session saved as it closed, its first sync held until the rows are in the page
      await recordIpcAtFirstRows(page)
      await repo.holdRemoteAnswers({ hold: ["sync"] })
      const cdp = await page.context().newCDPSession(page)
      await cdp.send("Performance.enable")
      let warmFirstRowsMs = 0
      const sync = await timeMainThread(cdp, async () => {
        const start = Date.now()
        await reloadWarm(page, repo)
        await expect(page.getByTestId("branch-row")).toHaveCount(ROWS)
        warmFirstRowsMs = Date.now() - start
        await expect(page.locator("[data-cached=true]")).toBeVisible()
        await repo.holdRemoteAnswers({ release: ["sync"] })
        await waitForSettledSession(page, repo)
        await expect(page.locator("[data-cached]")).toHaveCount(0)
        await waitForStepFacts(page)
      })
      const asks = await firstRowsAsks(page)
      const patches = await settledIpc(page)
      warmRuns.push({
        warmFirstRowsMs,
        warmFirstSyncScriptMs: sync.scriptMs,
        warmFirstSyncTaskMs: sync.taskMs,
        warmFirstSyncPatches: patches.channelMessages["subscribe_view"] ?? 0,
        warmFirstSyncBytes: patches.channelBytes,
        warmStartInvokes: Object.values(withoutAppTimers(asks?.invokes ?? {})).reduce((sum, count) => sum + count, 0),
      })
    })
  }

  if (process.env.PERF_PROFILE) {
    test("the CPU profile of the first sync of 141 rows with robots running on ten of them", async ({ page, setupRepo, browserName }) => {
      test.skip(browserName !== "chromium", "the CPU profile is Chromium's")
      await setupTable(page, setupRepo)
      const cdp = await page.context().newCDPSession(page)
      await cdp.send("Profiler.enable")
      await cdp.send("Profiler.setSamplingInterval", { interval: 100 })
      await cdp.send("Profiler.start")
      await syncBranches(page)
      await expect(page.getByTestId("branch-row")).toHaveCount(ROWS)
      await expect(page.getByTestId("patronus-chip")).toHaveCount(ROBOTS.length)
      const { profile } = await cdp.send("Profiler.stop")
      const directory = path.join("tests", "results", "perf")
      mkdirSync(directory, { recursive: true })
      writeFileSync(path.join(directory, `first-sync-${test.info().project.name}.cpuprofile`), JSON.stringify(profile))
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
      await waitForStepFacts(page)

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
