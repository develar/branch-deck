import type { Locator, Page } from "@playwright/test"
import { test as base, expect as playwrightExpect } from "@playwright/test"
import { PAGE_COMMANDS, tauriMockScript } from "../setup/tauri-mocks"

/**
 * How often an `expect.poll` asks: Playwright's default (100, 250 and 500 ms, then every second) is made for a remote
 * service, and a poll here reads memory, the log of a fake on the test server or the page. A change shows within 50 ms
 * instead of up to a second after it, which a test of a chain of starts and pushes paid at each step under load
 */
const POLL_INTERVALS = [50]

/** Whether `value` is a locator, whose assertions retry against the page */
function isLocator(value: unknown): value is Locator {
  return typeof value === "object" && value !== null && typeof (value as Locator).evaluateAll === "function" && typeof (value as Locator).count === "function"
}

/**
 * How many elements `locator` finds, and whether one of them lies in a popup of a portal whose exit runs: Base UI marks
 * a closing popup `data-closed` (a toast `data-ending-style`) and unmounts it once its exit animation finished, which
 * happens only as the window paints. Null when the page cannot answer (closed, navigating)
 */
async function exitState(locator: Locator): Promise<{ count: number; exiting: boolean } | null> {
  return locator
    .evaluateAll(nodes => ({
      count: nodes.length,
      exiting: nodes.some(node => {
        const closing = node.closest("[data-base-ui-portal] :is([data-closed], [data-ending-style])")
        return (
          closing !== null &&
          closing.getAnimations({ subtree: true }).some(animation => animation.playState === "running" && animation.effect?.getComputedTiming().iterations !== Infinity)
        )
      }),
    }))
    .catch(() => null)
}

const EXIT_WAIT =
  "waits for the exit animation of a closing popup to unmount it: the exit ends only as the window paints, and a loaded machine paints no frame of a window for seconds. Assert the close with expectClosed (tests/e2e/helpers/wait-helpers.ts)"

/**
 * The matchers of `expect(locator)` whose removal assertions (`toHaveCount(0)`, `toBeHidden()`, `not.toBeVisible()`,
 * `not.toBeAttached()`) refuse an element in a popup whose exit runs as they start, and say so when they time out on one
 */
function refusingExitWaits<T extends object>(matchers: T, locator: Locator, negated: boolean): T {
  return new Proxy(matchers, {
    get(target, property, receiver) {
      const value = Reflect.get(target, property, receiver) as unknown
      if (property === "not" && !negated && typeof value === "object" && value !== null) {
        return refusingExitWaits(value, locator, true)
      }
      const removal = negated ? property === "toBeVisible" || property === "toBeAttached" : property === "toHaveCount" || property === "toBeHidden"
      if (!removal || typeof value !== "function") {
        return value
      }
      return async (...args: unknown[]) => {
        if (property === "toHaveCount" && args[0] !== 0) {
          return value.apply(target, args)
        }
        const state = await exitState(locator)
        // nothing matches: every removal assertion holds, and the look that checks the exit answered it in one round
        // trip, as the matcher alone would (a check before each matcher cost 4% of the test time of a run under load)
        if (state?.count === 0) {
          return undefined
        }
        if (state?.exiting === true) {
          throw new Error(`${String(locator)} ${EXIT_WAIT}`)
        }
        try {
          return await value.apply(target, args)
        } catch (error) {
          if (error instanceof Error && (await exitState(locator))?.exiting === true) {
            error.message = `${error.message}\n\nThe assertion ${EXIT_WAIT}`
          }
          throw error
        }
      }
    },
  })
}

/**
 * Playwright's `expect` whose `poll` asks every `POLL_INTERVALS` unless the call names its own intervals, and whose
 * removal assertions of a locator refuse to wait for the exit of a closing popup (`refusingExitWaits`): the one `expect`
 * of the specs (`test-fixtures.ts`) and the helpers
 */
export const expect = new Proxy(playwrightExpect, {
  apply(target, thisArgument, args: Parameters<typeof playwrightExpect>) {
    const matchers = Reflect.apply(target, thisArgument, args) as object
    return isLocator(args[0]) ? refusingExitWaits(matchers, args[0], false) : matchers
  },
  get(target, property, receiver) {
    if (property !== "poll") {
      return Reflect.get(target, property, receiver)
    }
    return ((actual, options) =>
      target.poll(actual, typeof options === "string" ? { message: options, intervals: POLL_INTERVALS } : { intervals: POLL_INTERVALS, ...options })) as typeof target.poll
  },
})

type HoverOptions = Parameters<Locator["hover"]>[0]
type Hover = (this: Locator, options?: HoverOptions) => Promise<void>

/** The `hover` of Playwright's locators, kept by `refuseSubmenuHovers`: the move of `hoverSubmenuTrigger` */
let plainHover: Hover | null = null

/**
 * Makes `hover()` of every locator refuse a menu item that opens a submenu (`aria-haspopup="menu"`): the submenu opens
 * by `openSubmenu` (helpers/selection-helpers.ts), which asserts the open and says when the menu moved the item from
 * under the pointer before it opened (its items changed as it showed: the facts of the first sync came,
 * `waitForFactsAnswered`). A hover that opened nothing failed later, on an item of the submenu
 */
function refuseSubmenuHovers(page: Page): void {
  if (plainHover !== null) {
    return
  }
  const prototype = Object.getPrototypeOf(page.locator("html")) as object
  const hover = Reflect.get(prototype, "hover") as Hover
  plainHover = hover
  const refusing: Hover = async function (options) {
    const opensSubmenu = await this.evaluate(element => element.getAttribute("role") === "menuitem" && element.getAttribute("aria-haspopup") === "menu")
    if (opensSubmenu) {
      throw new Error(`${String(this)} opens a submenu: open it with openSubmenu (tests/e2e/helpers/selection-helpers.ts), or move with hoverSubmenuTrigger`)
    }
    return hover.call(this, options)
  }
  Reflect.set(prototype, "hover", refusing)
}

/**
 * The plain `hover()` of a menu item that opens a submenu (`refuseSubmenuHovers`): `openSubmenu`, and a test that moves
 * onto the item without the open (its delay held by a clock that stands still)
 */
export function hoverSubmenuTrigger(trigger: Locator, options?: HoverOptions): Promise<void> {
  if (plainHover === null) {
    throw new Error("hoverSubmenuTrigger before the page fixture")
  }
  return plainHover.call(trigger, options)
}

/**
 * Custom test fixture that automatically sets up Tauri mocks
 */
export const test = base.extend({
  page: async ({ page, context }, use, testInfo) => {
    refuseSubmenuHovers(page)
    // the console of the pages: every line live with DEBUG_E2E=1; otherwise their errors and uncaught exceptions, attached
    // to a failed test only, because an error-path test makes the app log its expected errors
    const pageErrors: string[] = []
    context.on("console", message => {
      if (process.env.DEBUG_E2E) {
        console.log("[Browser]", message.type(), message.text())
      } else if (message.type() === "error") {
        pageErrors.push(message.text())
      }
    })
    context.on("weberror", error => pageErrors.push(`Uncaught: ${error.error().stack ?? error.error().message}`))

    // Set debug flag if environment variable is set
    if (process.env.DEBUG_E2E) {
      await page.addInitScript(() => {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        ;(window as any).__DEBUG_E2E__ = true
      })
    }

    // Inject Tauri mocks before each navigation
    await page.addInitScript(tauriMockScript, PAGE_COMMANDS)

    // a popup, a form, a card or a toast is in place from its first frame, in every window: its zoom and slide in are
    // off. Playwright found the first frames of a zoom stable on a quick machine, the pointer went where the item was in
    // them, and the grown item left it (a submenu closed, a tooltip never opened); a click on a toast that slid in
    // (its close, its Undo) waited for the slide (a transition of 500 ms, not an animation). The exit still runs, as the
    // user sees it: a popup reopened while it closes stays tested. `E2E_EXIT_MS` lengthens every exit to that many ms, as
    // a loaded window that paints no frame for seconds does: a test that waits for an exit fails at once
    await context.addInitScript(exitMs => {
      const sheet = new CSSStyleSheet()
      const longExits = exitMs === null ? "" : ` [data-closed], [data-ending-style] { animation-duration: ${exitMs}ms !important; transition-duration: ${exitMs}ms !important; }`
      sheet.replaceSync(
        `[class*="animate-in"]:not([data-closed]) { animation: none !important; } [data-slot="toast"]:not([data-ending-style]) { transition: none !important; }${longExits}`,
      )
      document.adoptedStyleSheets = [...document.adoptedStyleSheets, sheet]
    }, process.env.E2E_EXIT_MS ?? null)

    // Set a fixed time for all tests to ensure consistent date/time display
    // Using Jan 2, 2024 10:00 AM UTC to match our test commits from Jan 1, 2024
    await context.clock.install({
      time: new Date("2024-01-02T10:00:00Z"),
    })

    // Use the page
    await use(page)

    if (testInfo.status !== testInfo.expectedStatus && pageErrors.length > 0) {
      await testInfo.attach("page errors", { body: pageErrors.join("\n"), contentType: "text/plain" })
    }
  },
})
