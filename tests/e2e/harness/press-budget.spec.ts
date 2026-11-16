import { expect, test, testRemainingMs } from "../fixtures/base-test"
import { pressAndExpectOpened } from "../helpers/sub-window-helpers"

// a press waits for the open of its sub-window by what is left of its test (`testRemainingMs`), never by the whole
// timeout of the test: a press that opens nothing late in its test fails with its own words before that timeout, which
// would name no cause
test("a press that opens no sub-window late in its test fails with its own words before the timeout of the test", async ({ page }) => {
  test.setTimeout(3_000)
  // past where a wait of the timeout less its margin, or of a floor of a second, would outlast the test: the state the
  // test waits for is what is left of it
  await expect.poll(() => testRemainingMs() < 800, { timeout: 3_000 }).toBe(true)
  const failed = await pressAndExpectOpened(page, () => Promise.resolve(), "review", 1).then(
    () => "opened",
    (error: unknown) => String(error),
  )
  expect(failed).toContain("No open_sub_window answered after the press that opens review")
})
