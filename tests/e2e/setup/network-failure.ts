/**
 * The failures of the network a page leaves behind as it leaves for the page of another window (`openSubWindowPage`),
 * which the app never does: WebKit fails the requests the page still had, which is no error of the app
 * (tests/e2e/fixtures/base-test.ts)
 */

/** What WebKit says of a request that failed as its page left, the whole of a first line */
const LEFT_BEHIND = /(?:^|: )(?:TypeError: )?Load failed$|Fetch API cannot load .+ due to access control checks\.?$/

/** A failure of the network in the words of WebKit or Chromium, wherever in the message: what `networkDiagnostics` says */
export const ANY_NETWORK_FAILURE = /Load failed|due to access control checks|Failed to fetch|net::ERR_/

/**
 * Whether `told`, an error as the page or the engine told it, is a request its page left behind: its first line alone,
 * as the log of the page puts the stack of an error on the lines after it (`Unhandled rejection of the page: Load
 * failed`, then `arrayBuffer@[native code]`, …: the mock of the IPC read an answer the page no longer waited for)
 */
export function isLeftBehind(told: string): boolean {
  const newline = told.indexOf("\n")
  return LEFT_BEHIND.test(newline === -1 ? told : told.slice(0, newline))
}
