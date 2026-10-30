import { test as base, expect as playwrightExpect } from "@playwright/test"
import { PAGE_COMMANDS, tauriMockScript } from "../setup/tauri-mocks"

/**
 * How often an `expect.poll` asks: Playwright's default (100, 250 and 500 ms, then every second) is made for a remote
 * service, and a poll here reads memory, the log of a fake on the test server or the page. A change shows within 50 ms
 * instead of up to a second after it, which a test of a chain of starts and pushes paid at each step under load
 */
const POLL_INTERVALS = [50]

/**
 * Playwright's `expect` whose `poll` asks every `POLL_INTERVALS` unless the call names its own intervals: the one
 * `expect` of the specs (`test-fixtures.ts`) and the helpers
 */
export const expect = new Proxy(playwrightExpect, {
  get(target, property, receiver) {
    if (property !== "poll") {
      return Reflect.get(target, property, receiver)
    }
    return ((actual, options) =>
      target.poll(actual, typeof options === "string" ? { message: options, intervals: POLL_INTERVALS } : { intervals: POLL_INTERVALS, ...options })) as typeof target.poll
  },
})

/**
 * Custom test fixture that automatically sets up Tauri mocks
 */
export const test = base.extend({
  page: async ({ page, context }, use, testInfo) => {
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
    // user sees it: a popup reopened while it closes stays tested
    await context.addInitScript(() => {
      const sheet = new CSSStyleSheet()
      sheet.replaceSync('[class*="animate-in"]:not([data-closed]) { animation: none !important; } [data-slot="toast"]:not([data-ending-style]) { transition: none !important; }')
      document.adoptedStyleSheets = [...document.adoptedStyleSheets, sheet]
    })

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
