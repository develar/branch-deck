import { expect, test, testRemainingMs } from "../fixtures/base-test"
import { pressAndExpectOpened } from "../helpers/sub-window-helpers"

// a press waits for the open of its sub-window by what is left of its test (`testRemainingMs`), never by the whole
// timeout of the test: a press that opens nothing late in its test fails with its own words before that timeout, which
// would name no cause
test("a press that opens no sub-window late in its test fails with its own words before the timeout of the test", async ({ page }) => {
  test.setTimeout(10_000)
  // past where a wait of the timeout less its margin (5 s) would outlast the test: the state the test waits for is what
  // is left of it. The press waits all of it but a quarter, a second, a margin no load of a worker races
  await expect.poll(() => testRemainingMs() < 4_000, { timeout: 10_000 }).toBe(true)
  const left = testRemainingMs()
  const pressed = Date.now()
  const failed = await pressAndExpectOpened(page, () => Promise.resolve(), "review", 1).then(
    () => "opened",
    (error: unknown) => String(error),
  )
  expect(failed).toContain("No open_sub_window answered after the press that opens review")
  // it waited for the open all of what was left but its margin, a quarter of it: a timer never fires early
  expect(Date.now() - pressed).toBeGreaterThanOrEqual(left * 0.75 - 5)
})
