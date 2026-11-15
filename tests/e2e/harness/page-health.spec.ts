// the plain test of Playwright: the probe asks a bare page, which the mocks of the app would refuse without a repository
import { test } from "@playwright/test"
import { expect } from "../fixtures/base-test"
import { pageAnswers, PROBE_MS } from "../helpers/page-health"

// what the page fixture asks the pages of a failed test, for the stalled actions of the timing reporter
// (`tests/e2e/setup/timing-reporter.ts`): a page stuck in a task does not answer, a page on a slow machine does
test("the probe of a failed test tells a page stuck in a task from one that answers", async ({ page, context }) => {
  await page.goto("about:blank")
  expect(await pageAnswers(page)).toBe(true)

  const stuck = await context.newPage()
  await stuck.goto("about:blank")
  // the main thread loops from the next task on, as a page that never ends a task does
  await stuck.evaluate(() => {
    const channel = new MessageChannel()
    // kept, so the channel and its message outlive the evaluate
    ;(window as unknown as { __stuck: MessageChannel }).__stuck = channel
    channel.port1.onmessage = () => {
      for (;;) {
        // stuck
      }
    }
    channel.port2.postMessage(null)
  })
  const asked = Date.now()
  expect(await pageAnswers(stuck)).toBe(false)
  // it waited for the page as long as the probe allows, no longer than a few seconds
  expect(Date.now() - asked).toBeGreaterThanOrEqual(PROBE_MS - 50)
  // the other page of the test still answers
  expect(await pageAnswers(page)).toBe(true)
  await stuck.close()
})
