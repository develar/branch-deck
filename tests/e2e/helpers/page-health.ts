import type { BrowserContext, Page } from "@playwright/test"

/**
 * What a test's pages said of their main thread, attached to the test as `PAGE_HEALTH` for the timing reporter: the
 * long tasks of every page (from `LONG_TASK_MS`, each as its end on the clock of the runner and its length) and, for a
 * test that failed, whether every open page answered an evaluate within `PROBE_MS`: asked `NEAR_TIMEOUT_MS` before
 * its probe would end at the timeout of the test, as Playwright closes the pages of a test that times out before its
 * fixtures end, else at its end (`null`: not asked, or no page was open)
 */
export interface PageHealth {
  longTasks: [end: number, ms: number][]
  answered: boolean | null
}

export const PAGE_HEALTH = "page health"

/** A long task this long or longer is told to the runner as it ends */
export const LONG_TASK_MS = 500

/** How long a page may take to answer an evaluate at the end of a failed test before its main thread counts as stuck */
export const PROBE_MS = 2000

/** How long before the probe would end at the timeout of a test the pages are asked */
export const NEAR_TIMEOUT_MS = 1000

const LONG_TASK_MARK = "[e2e-longtask] "

/**
 * Tells the runner each long task of a page of `context` from `LONG_TASK_MS` as it ends, in a line of the console: the
 * page keeps nothing, so a long test holds no buffer, and the runner stamps the end on its own clock (the clock of the
 * page is Playwright's fake one). A task still running is in no entry: a probe of a failed test tells it
 */
export async function watchLongTasks(context: BrowserContext): Promise<void> {
  await context.addInitScript(
    ({ mark, threshold }) => {
      try {
        new PerformanceObserver(list => {
          for (const entry of list.getEntries()) {
            if (entry.duration >= threshold) {
              console.info(`${mark}${Math.round(entry.duration)}`)
            }
          }
        }).observe({ type: "longtask" })
      } catch {
        // an engine without long tasks (WebKit): nothing to tell
      }
    },
    { mark: LONG_TASK_MARK, threshold: LONG_TASK_MS },
  )
}

/** The length of the long task a line of the console tells, or `null` for another line */
export function longTaskOf(text: string): number | null {
  return text.startsWith(LONG_TASK_MARK) ? Number(text.slice(LONG_TASK_MARK.length)) : null
}

/**
 * Whether the main thread of `page` answers within `ms`: a function of the page that holds at once, waited for no
 * longer, is `false` when the page is stuck in a task (or closed meanwhile)
 */
export async function pageAnswers(page: Page, ms = PROBE_MS): Promise<boolean> {
  if (page.isClosed()) {
    return false
  }
  return page
    .waitForFunction(() => true, undefined, { timeout: ms, polling: 100 })
    .then(
      () => true,
      () => false,
    )
}
