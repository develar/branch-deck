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
import { mkdirSync, writeFileSync } from "node:fs"
import path from "node:path"
import type { Page } from "@playwright/test"
import { test, expect } from "../e2e/fixtures/test-fixtures"
import { availableRobots, robot } from "../e2e/helpers/patronus-helpers"
import { syncBranches, waitForBranchesLoaded } from "../e2e/helpers/sync-helpers"

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

const results: Record<string, Metrics> = {}

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
    const robots = Array.from({ length: 10 }, (_, index) => robot(`feature-${String(index * 12).padStart(3, "0")}`, `robot-${index}`))
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
})
