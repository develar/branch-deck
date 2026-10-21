// The layout of the branch view at scale (docs/design/ui-theme.md#branch-columns): the `branch_table_large` template,
// 120 lone branches and three islands of seven, 141 rows whose columns line up across the cards, robots running on ten.
// A text that changes in one cell can change the width of a column of every card: the numbers say what a change costs,
// and decided the mechanism (measured tracks over tables, not one CSS subgrid: 8.5–12.7 ms a text change in WebKit).
//
//   pnpm perf:table                      3 runs, a table on stdout
//   PERF_RUNS=5 pnpm perf:table          more runs
//   pnpm perf:table --project=webkit     in WebKit, the engine of the app on macOS
//
// The table goes to tests/results/perf/branch-table-<project>.json too. Each metric is the median of its runs:
// - firstRowsMs: from the sync's start until every branch row is in the page;
// - relayoutMs: one layout of the view after its width changed (a resize of the window);
// - widerTextMs: one layout after the status text of a row changed its width (a push's percent, "4%" to "45%"); when it
//   changes the widest item of a track, the frame adds one relayout of the view, as the columns follow it;
// - sameWidthTextMs: one layout after a text changed inside an element of a fixed width (a robot's time that ticks).
//
// The main thread of a sync (Chromium only, the CDP metrics of the page; docs/design/rendering-performance.md): what the
// page spends on the events of a sync and what they derive, the hold and landing rule of the robots included:
// - firstSyncScriptMs / firstSyncTaskMs: the script time and the task time of the first sync, until its rows and chips;
// - resyncScriptMs / resyncTaskMs: the same of a sync that changes nothing;
// - robotIpc: the requests of the robots' content (`get_robot_*`, `get_repository_view`) during both syncs.
import { mkdirSync, writeFileSync } from "node:fs"
import path from "node:path"
import type { CDPSession, Page } from "@playwright/test"
import { test, expect } from "../e2e/fixtures/test-fixtures"
import { availableRobots, robot } from "../e2e/helpers/patronus-helpers"
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

const results: Record<string, Metrics | SyncMetrics> = {}

interface SyncMetrics {
  firstSyncScriptMs: number
  firstSyncTaskMs: number
  resyncScriptMs: number
  resyncTaskMs: number
  robotIpc: number
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
    console.table(results)
    const directory = path.join("tests", "results", "perf")
    mkdirSync(directory, { recursive: true })
    writeFileSync(path.join(directory, `branch-table-${test.info().project.name}.json`), `${JSON.stringify(results, null, 2)}\n`)
  })

  test("141 branch rows in a table and three islands, robots running on ten of them", async ({ page, setupRepo }) => {
    test.setTimeout(10 * 60_000)
    await page.setViewportSize({ width: 1516, height: 900 })
    const robots = ROBOTS
    const runs: Metrics[] = []
    for (let run = 0; run < RUNS; run++) {
      await setupRepo("branch_table_large", { patronus: availableRobots(...robots) })
      const start = Date.now()
      await syncBranches(page)
      await waitForBranchesLoaded(page)
      await expect(page.getByTestId("branch-row")).toHaveCount(ROWS)
      const firstRowsMs = Date.now() - start
      await expect(page.getByTestId("patronus-chip")).toHaveCount(robots.length)
      runs.push({
        firstRowsMs,
        relayoutMs: await timeIn(page, "relayout"),
        widerTextMs: await timeIn(page, "widerText"),
        sameWidthTextMs: await timeIn(page, "sameWidthText"),
      })
    }
    results["141 rows"] = {
      firstRowsMs: median(runs.map(run => run.firstRowsMs)),
      relayoutMs: median(runs.map(run => run.relayoutMs)),
      widerTextMs: median(runs.map(run => run.widerTextMs)),
      sameWidthTextMs: median(runs.map(run => run.sameWidthTextMs)),
    }
  })

  test("the main thread of a sync of 141 rows with robots running on ten of them", async ({ page, setupRepo, browserName }) => {
    test.skip(browserName !== "chromium", "the CDP metrics are Chromium's")
    test.setTimeout(10 * 60_000)
    await page.setViewportSize({ width: 1516, height: 900 })
    const cdp = await page.context().newCDPSession(page)
    await cdp.send("Performance.enable")
    let robotIpc = 0
    page.on("request", request => {
      if (/\/invoke\/(get_robot_landings|get_robot_spans|get_repository_view)$/.test(request.url())) {
        robotIpc++
      }
    })
    const runs: SyncMetrics[] = []
    for (let run = 0; run < RUNS; run++) {
      await setupRepo("branch_table_large", { patronus: availableRobots(...ROBOTS) })
      robotIpc = 0
      const first = await timeMainThread(cdp, async () => {
        await syncBranches(page)
        await expect(page.getByTestId("branch-row")).toHaveCount(ROWS)
        await expect(page.getByTestId("patronus-chip")).toHaveCount(ROBOTS.length)
      })
      const resync = await timeMainThread(cdp, () => runAndWaitForSync(page, () => page.getByTestId("sync-button").click()))
      runs.push({ firstSyncScriptMs: first.script, firstSyncTaskMs: first.task, resyncScriptMs: resync.script, resyncTaskMs: resync.task, robotIpc })
    }
    results["sync main thread"] = {
      firstSyncScriptMs: median(runs.map(run => run.firstSyncScriptMs)),
      firstSyncTaskMs: median(runs.map(run => run.firstSyncTaskMs)),
      resyncScriptMs: median(runs.map(run => run.resyncScriptMs)),
      resyncTaskMs: median(runs.map(run => run.resyncTaskMs)),
      robotIpc: median(runs.map(run => run.robotIpc)),
    }
  })
})
