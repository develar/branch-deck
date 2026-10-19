// The time of the review window over large synthetic commits (docs/design/diff-view.md, "Performance Targets"): the
// review root mounted, the first diff rows, the first screen highlighted, the long tasks of the main thread until it
// is quiet, the DOM nodes and the heap, and the bytes of the worker messages, for an open and for the expansion of the
// unchanged lines of a large file; then the frames of a scroll at 3 and at 20 viewports per second.
//
//   pnpm perf:diff                      3 runs of each scenario, a table on stdout
//   PERF_RUNS=5 pnpm perf:diff          more runs
//   pnpm perf:diff --project=webkit     in WebKit, the engine of the app on macOS
//
// The table goes to tests/results/perf/diff-review.json too. The commits are routed answers of the review's commands
// (`get_commit_files`, `get_file_diffs` (as SSE, the way the test server streams its channel), `get_blob_texts`) with the blocks of the diff engine; the window is the
// production build that the e2e setup builds. Bytes are the length of the strings and the typed arrays of each
// message, counted in a run of their own, because counting them costs main-thread time.
import { mkdirSync, writeFileSync } from "node:fs"
import path from "node:path"
import type { Page } from "@playwright/test"
import type { BlobText } from "../../src/lib/bindings"
import type { BranchReviewWindowData } from "../../src/types/sub-window"
import { test, expect } from "../e2e/fixtures/test-fixtures"
import { expandBranch, findBranchRow, getBranchCommitRow } from "../e2e/helpers/branch-helpers"
import { openCommitPreview } from "../e2e/helpers/preview-helpers"
import { getOpenedSubWindows, openLinkedSubWindow } from "../e2e/helpers/sub-window-helpers"
import type { OpenedSubWindow } from "../e2e/helpers/sub-window-helpers"
import { hugeFileCommit, largeCommit } from "./diff-fixtures"
import type { SyntheticFile } from "./diff-fixtures"

const RUNS = Number(process.env.PERF_RUNS ?? 3)
const EMPTY_TASKS = { quietMs: 0, longTasks: 0, longTaskMs: 0, maxLongTaskMs: 0 }
/** The window is quiet once no long task started for this long */
const QUIET_MS = 1500

interface PhaseMetrics {
  readyMs?: number
  firstRowsMs?: number
  highlightedMs: number
  /** The elements of the diff view, once the open is quiet */
  domNodes?: number
  /** The JS heap of the window in MB, once the open is quiet (Chromium only) */
  heapMb?: number
  quietMs: number
  longTasks: number
  longTaskMs: number
  maxLongTaskMs: number
  bytesToWorker?: number
  bytesFromWorker?: number
}

/** In the page: long tasks, the bytes of the worker messages when asked, and a scan of the visible diff rows */
function installProbes(measureBytes: boolean) {
  const probe = { longTasks: [] as [number, number][], bytesToWorker: 0, bytesFromWorker: 0, lastMessageAt: 0, scanRows }
  ;(window as unknown as { __perf: typeof probe }).__perf = probe
  new PerformanceObserver(list => {
    for (const entry of list.getEntries()) {
      probe.longTasks.push([entry.startTime, entry.duration])
    }
  }).observe({ type: "longtask", buffered: true })

  /** The visible rows of the diffs with their text, and how many of them are highlighted: a token has a role */
  function scanRows(): { rows: number; highlighted: number } {
    let rows = 0
    let highlighted = 0
    for (const row of document.querySelectorAll(".dv-row:not([data-kind=fold])")) {
      const box = row.getBoundingClientRect()
      if (box.bottom <= 0 || box.top >= innerHeight || box.height === 0 || (row.textContent ?? "").trim().length < 10) {
        continue
      }
      rows++
      if (row.querySelector("span[class^='t'], span[class*=' t']") !== null) {
        highlighted++
      }
    }
    return { rows, highlighted }
  }

  /** The length of the strings and the typed arrays of a message */
  function sizeOf(value: unknown): number {
    if (typeof value === "string") {
      return value.length
    }
    if (ArrayBuffer.isView(value)) {
      return value.byteLength
    }
    if (value === null || typeof value !== "object") {
      return 8
    }
    let size = 0
    for (const item of Array.isArray(value) ? value : Object.values(value)) {
      size += sizeOf(item)
    }
    return size
  }

  if (measureBytes) {
    const postMessage = Reflect.get(Worker.prototype, "postMessage") as (this: Worker, message: unknown, transfer?: unknown) => void
    Worker.prototype.postMessage = function (this: Worker, message: unknown, transfer?: unknown) {
      probe.bytesToWorker += sizeOf(message)
      probe.lastMessageAt = performance.now()
      postMessage.call(this, message, transfer)
    }
    const addEventListener = Reflect.get(Worker.prototype, "addEventListener") as (
      this: Worker,
      type: string,
      listener: EventListenerOrEventListenerObject,
      options?: unknown,
    ) => void
    Worker.prototype.addEventListener = function (this: Worker, type: string, listener: EventListenerOrEventListenerObject, options?: unknown) {
      if (type === "message" && typeof listener === "function") {
        const counted = (event: Event) => {
          probe.bytesFromWorker += sizeOf((event as MessageEvent).data)
          probe.lastMessageAt = performance.now()
          listener.call(this, event)
        }
        addEventListener.call(this, type, counted, options)
        return
      }
      addEventListener.call(this, type, listener, options)
    }
  }
}

/**
 * The time when `condition` first held, checked every frame, in ms after the opener asked for the window
 * (`__SUB_WINDOW_OPEN__.openedAt`): a review window can be reused or made before its open, so the page's own clock
 * does not start with the open
 */
async function timeWhen(window: Page, condition: "ready" | "rows" | "highlighted", since?: number): Promise<number> {
  const handle = await window.waitForFunction(
    ([kind, start]) => {
      const sinceOpen = () =>
        Date.now() - (start ?? (window as unknown as { __SUB_WINDOW_OPEN__?: { openedAt: number | null } }).__SUB_WINDOW_OPEN__?.openedAt ?? performance.timeOrigin)
      if (kind === "ready") {
        return document.querySelector("[data-review-root]") === null ? false : sinceOpen()
      }
      const { rows, highlighted } = (window as unknown as { __perf: { scanRows: () => { rows: number; highlighted: number } } }).__perf.scanRows()
      const holds = kind === "rows" ? rows > 0 : rows >= 10 && highlighted === rows
      return holds ? sinceOpen() : false
    },
    [condition, since] as const,
    { polling: "raf", timeout: 120_000 },
  )
  return (await handle.jsonValue()) as number
}

/** Waits until no long task started for `QUIET_MS`, and sums the long tasks since `since` */
async function quietAndLongTasks(window: Page, since: number): Promise<Pick<PhaseMetrics, "quietMs" | "longTasks" | "longTaskMs" | "maxLongTaskMs">> {
  const quietAt = await window.waitForFunction(
    quiet => {
      const tasks = (window as unknown as { __perf: { longTasks: [number, number][] } }).__perf.longTasks
      const lastEnd = tasks.reduce((end, [start, duration]) => Math.max(end, start + duration), 0)
      // an array: a quiet window without any long task ends at 0, which is falsy
      return performance.now() - lastEnd >= quiet ? [lastEnd] : false
    },
    QUIET_MS,
    { polling: 100, timeout: 180_000 },
  )
  const [lastEnd] = (await quietAt.jsonValue()) as [number]
  const tasks = await window.evaluate(start => (window as unknown as { __perf: { longTasks: [number, number][] } }).__perf.longTasks.filter(([begin]) => begin >= start), since)
  return {
    quietMs: Math.max(lastEnd, since) - since,
    longTasks: tasks.length,
    longTaskMs: tasks.reduce((sum, [, duration]) => sum + duration, 0),
    maxLongTaskMs: tasks.reduce((max, [, duration]) => Math.max(max, duration), 0),
  }
}

/** The bytes of the worker messages once no message went for 2 s, then counted from zero again */
async function workerBytes(window: Page): Promise<{ bytesToWorker: number; bytesFromWorker: number }> {
  await window.waitForFunction(() => performance.now() - (window as unknown as { __perf: { lastMessageAt: number } }).__perf.lastMessageAt >= 2000, undefined, {
    polling: 250,
    timeout: 600_000,
  })
  return window.evaluate(() => {
    const probe = (window as unknown as { __perf: { bytesToWorker: number; bytesFromWorker: number } }).__perf
    const bytes = { bytesToWorker: probe.bytesToWorker, bytesFromWorker: probe.bytesFromWorker }
    probe.bytesToWorker = 0
    probe.bytesFromWorker = 0
    return bytes
  })
}

/** The answers of the review's commands for `files`, in the window's page */
async function routeCommit(window: Page, commitId: string, files: SyntheticFile[]): Promise<void> {
  const blobs = new Map<string, BlobText>(files.flatMap(it => it.blobs.map(blob => [blob.id, blob] as const)))
  const commitFiles = {
    commitId,
    files: files.map(it => it.file),
    additions: files.reduce((sum, it) => sum + it.file.additions, 0),
    deletions: files.reduce((sum, it) => sum + it.file.deletions, 0),
  }
  await window.route("**/invoke/get_commit_files", route => route.fulfill({ json: commitFiles }))
  await window.route("**/invoke/get_commits_files", route => route.fulfill({ json: [commitFiles] }))
  // the events of the channel as the test server sends them: the files, then the content of each
  const events = [
    { kind: "files", files: files.map(({ diff: { content: _, ...header } }) => header) },
    ...files.map((it, index) => ({ kind: "content", index, content: it.diff.content })),
  ]
  await window.route("**/invoke/get_file_diffs", route =>
    route.fulfill({ contentType: "text/event-stream", body: events.map(event => `event: fileDiff\ndata: ${JSON.stringify(event)}\n\n`).join("") }),
  )
  await window.route("**/invoke/get_blob_texts", async route => {
    const { blobs: requests } = route.request().postDataJSON() as { blobs: { id: string }[] }
    await route.fulfill({ json: requests.map(({ id }) => blobs.get(id) ?? { kind: "missing", id }) })
  })
}

/** D on the preview of the commit: a commit of a branch is reviewed in the window of its branch, at the commit */
async function openReviewData(page: Page): Promise<OpenedSubWindow<BranchReviewWindowData>> {
  await expandBranch(page, findBranchRow(page, "session"))
  const subject = "a prompt that waits for its turn keeps the session row running"
  const preview = await openCommitPreview(page, getBranchCommitRow(page, "session", subject))
  await expect(preview.getByRole("heading", { level: 2 })).toHaveText(subject)
  await page.keyboard.press("d")
  await expect.poll(async () => (await getOpenedSubWindows(page)).length).toBe(1)
  const [opened] = await getOpenedSubWindows<BranchReviewWindowData>(page)
  expect(opened?.windowId).toBe("branch-review")
  return opened!
}

/** Opens the review of `files`, then expands the unchanged lines above the second hunk of `expandPath` */
async function runScenario(page: Page, opened: OpenedSubWindow<BranchReviewWindowData>, files: SyntheticFile[], expandPath: string, measureBytes: boolean) {
  // the copy of the commit in the virtual branch, which the review shows
  const commitId = opened.data.focus.commitId!
  const window = await openLinkedSubWindow(page, opened, {
    beforeLoad: async target => {
      await target.addInitScript(installProbes, measureBytes)
      await routeCommit(target, commitId, files)
    },
  })
  const open: PhaseMetrics = {
    readyMs: await timeWhen(window, "ready"),
    firstRowsMs: await timeWhen(window, "rows"),
    highlightedMs: await timeWhen(window, "highlighted"),
    ...EMPTY_TASKS,
  }
  // counting the bytes walks every message on the main thread: its times and long tasks mean nothing; the long tasks
  // count from the open, on the clock of the page
  const openedAt = await window.evaluate(
    () => ((window as unknown as { __SUB_WINDOW_OPEN__?: { openedAt: number | null } }).__SUB_WINDOW_OPEN__?.openedAt ?? performance.timeOrigin) - performance.timeOrigin,
  )
  Object.assign(open, measureBytes ? await workerBytes(window) : await quietAndLongTasks(window, Math.max(openedAt, 0)))
  open.domNodes = await window.evaluate(() => document.querySelectorAll("[data-testid=diff-view] *").length)
  open.heapMb = await window.evaluate(() => {
    const memory = (performance as unknown as { memory?: { usedJSHeapSize: number } }).memory
    return memory === undefined ? Number.NaN : memory.usedJSHeapSize / 1024 / 1024
  })
  // a fold names the scope of its lines
  await expect(window.locator(".dv-fold-label").first()).toBeVisible()

  // the tree scrolls the diffs to the file; its first fold is the first one on screen
  await window.getByRole("treeitem", { name: path.basename(expandPath) }).click()
  const button = window.locator(`[data-testid=diff-view-file][data-path="${expandPath}"] .dv-fold-button`).filter({ visible: true }).first()
  await expect(button).toBeVisible()
  const [started, startedAt] = await window.evaluate(() => [performance.now(), Date.now()] as const)
  await button.click()
  const expand: PhaseMetrics = { highlightedMs: await timeWhen(window, "highlighted", startedAt), ...EMPTY_TASKS }
  Object.assign(expand, measureBytes ? await workerBytes(window) : await quietAndLongTasks(window, started))
  // the frames of the same loop without a scroll: what the machine gives a frame at its load
  const still = measureBytes ? {} : await scrollFrames(window, 0)
  const scroll3 = measureBytes ? {} : await scrollFrames(window, 3)
  const scroll20 = measureBytes ? {} : await scrollFrames(window, 20)
  await window.close()
  return { open, expand, still, scroll3, scroll20 }
}

/**
 * Scrolls the diffs from the top by `viewportsPerSecond` each frame for 2 s: the frame times, and the frames whose
 * viewport has a point that no row and no header covers
 */
async function scrollFrames(window: Page, viewportsPerSecond: number): Promise<Record<string, number>> {
  return window.evaluate(async speed => {
    const scroller = document.querySelector<HTMLElement>("[data-testid=diff-view]")!
    scroller.scrollTop = 0
    await new Promise(resolve => requestAnimationFrame(resolve))
    const box = scroller.getBoundingClientRect()
    const times: number[] = []
    ;(window as unknown as { __diffPaintTimes?: number[] }).__diffPaintTimes = times
    const frames: number[] = []
    let empty = 0
    let last = performance.now()
    const end = last + 2000
    while (performance.now() < end && scroller.scrollTop + scroller.clientHeight < scroller.scrollHeight - 1) {
      const now = await new Promise<number>(resolve => requestAnimationFrame(resolve))
      frames.push(now - last)
      // the frame shows what the last scroll painted: no point below the sticky header is the empty host of the rows
      // (the gap between two cards is no row either, by design)
      for (let index = 1; index < 10; index++) {
        const hit = document.elementFromPoint(box.left + 120, box.top + 40 + ((box.height - 40) * index) / 10)
        if (hit === null || hit.classList.contains("dv-rows")) {
          empty++
          break
        }
      }
      scroller.scrollTop += (speed * scroller.clientHeight * (now - last)) / 1000
      last = now
    }
    const sorted = frames.toSorted((a, b) => a - b)
    // the cost of each scroll event of the painter, the layout of the rows it added included
    delete (window as unknown as { __diffPaintTimes?: number[] }).__diffPaintTimes
    const paints = times.toSorted((a, b) => a - b)
    return {
      frames: frames.length,
      p95FrameMs: Math.round(sorted[Math.floor(sorted.length * 0.95)] ?? 0),
      paints: paints.length,
      medianPaintMs: Math.round((paints[Math.floor(paints.length / 2)] ?? 0) * 10) / 10,
      p95PaintMs: Math.round((paints[Math.floor(paints.length * 0.95)] ?? 0) * 10) / 10,
      maxPaintMs: Math.round((paints.at(-1) ?? 0) * 10) / 10,
      slowFramesPct: Math.round((frames.filter(frame => frame > 20).length * 1000) / Math.max(frames.length, 1)) / 10,
      emptyFrames: empty,
    }
  }, viewportsPerSecond)
}

function median(values: number[]): number {
  const sorted = values.toSorted((a, b) => a - b)
  return sorted[Math.floor(sorted.length / 2)] ?? Number.NaN
}

type ScenarioResult = Record<"open" | "expand" | "still" | "scroll3" | "scroll20", Record<string, number>>

async function measure(page: Page, files: SyntheticFile[], expandPath: string): Promise<ScenarioResult> {
  const opened = await openReviewData(page)
  const runs: Awaited<ReturnType<typeof runScenario>>[] = []
  for (let run = 0; run < RUNS; run++) {
    runs.push(await runScenario(page, opened, files, expandPath, false))
  }
  const bytes = await runScenario(page, opened, files, expandPath, true)
  const result = {} as ScenarioResult
  for (const phase of ["open", "expand"] as const) {
    const keys = Object.keys(runs[0]![phase]) as (keyof PhaseMetrics)[]
    result[phase] = Object.fromEntries(keys.map(key => [key, Math.round(median(runs.map(run => run[phase][key] ?? Number.NaN)))]))
    result[phase].bytesToWorker = bytes[phase].bytesToWorker ?? 0
    result[phase].bytesFromWorker = bytes[phase].bytesFromWorker ?? 0
  }
  for (const phase of ["still", "scroll3", "scroll20"] as const) {
    const keys = Object.keys(runs[0]![phase])
    result[phase] = Object.fromEntries(keys.map(key => [key, median(runs.map(run => (run[phase] as Record<string, number>)[key] ?? Number.NaN))]))
  }
  return result
}

const results: Record<string, ScenarioResult> = {}

test.describe.configure({ mode: "serial" })

test.describe("Diff review performance", () => {
  test.beforeEach(async ({ setupRepo, syncAndWaitForBranches }) => {
    await setupRepo("commit_tree")
    await syncAndWaitForBranches()
  })

  test.afterAll(() => {
    console.table(Object.fromEntries(Object.entries(results).flatMap(([scenario, phases]) => Object.entries(phases).map(([phase, metrics]) => [`${scenario} ${phase}`, metrics]))))
    const directory = path.join("tests", "results", "perf")
    mkdirSync(directory, { recursive: true })
    writeFileSync(path.join(directory, `diff-review-${test.info().project.name}.json`), `${JSON.stringify(results, null, 2)}\n`)
  })

  test("a commit of 200 files, 4 of them 3,000-5,000 lines", async ({ page }) => {
    test.setTimeout(20 * 60_000)
    const files = largeCommit()
    results["200 files"] = await measure(page, files, files.find(it => it.file.path.includes("/Large"))!.file.path)
  })

  test("a commit of one file of 10,000 lines", async ({ page }) => {
    test.setTimeout(20 * 60_000)
    const files = hugeFileCommit()
    results["10k lines"] = await measure(page, files, files[0]!.file.path)
  })
})
