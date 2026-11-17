import { TEST_SERVER_URL } from "../helpers/test-server-url"
import { describeServerCounts, holdsAnything } from "./server-counts"
import type { ServerCounts } from "./server-counts"

/**
 * How long the teardown reads the counts again while the server holds something: the deletion of the last
 * repositories goes on in the background (their sessions close, their commands end), a moment after their tests
 */
const SETTLE_MS = 5_000
const POLL_MS = 200

async function readCounts(): Promise<ServerCounts> {
  const response = await fetch(`${TEST_SERVER_URL}/debug/counts`, { signal: AbortSignal.timeout(2_000) })
  if (!response.ok) {
    throw new Error(`${response.status} ${response.statusText}`)
  }
  return (await response.json()) as ServerCounts
}

/**
 * Global teardown for Playwright tests: prints what the test server holds once the run ended (`/debug/counts`): the
 * repositories, the deletions in progress, the apps of deleted repositories still held, the sessions and the orphans
 * among them and the watches, each left behind by the test that made it, and the waits the clocks of deleted
 * repositories served once they ran out (docs/design/e2e-testing.md, Teardown). A leak shows in every run instead of in
 * a `curl` nobody makes. It never fails the run: a server that cannot answer is said so
 */
async function globalTeardown() {
  try {
    const deadline = Date.now() + SETTLE_MS
    let counts = await readCounts()
    while (holdsAnything(counts) && Date.now() < deadline) {
      await new Promise(resolve => setTimeout(resolve, POLL_MS))
      counts = await readCounts()
    }
    console.log(
      describeServerCounts(counts)
        .map(line => `[Global Teardown] ${line}`)
        .join("\n"),
    )
  } catch (error) {
    console.log(`[Global Teardown] Cannot read what the test server holds: ${error instanceof Error ? error.message : String(error)}`)
  }
}

export default globalTeardown
