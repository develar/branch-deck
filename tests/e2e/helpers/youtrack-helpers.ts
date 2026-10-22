import type { Locator, Page } from "@playwright/test"
import { expect } from "@playwright/test"
import type { YouTrackUser } from "../../../src/lib/bindings"
import { getOpenedUrls } from "./pull-request-helpers"
import { openSettingsWindow } from "./settings-helpers"
import { rewriteNextSyncEvents } from "./sync-helpers"
import type { TestRepositoryBuilder } from "./test-repository"

/**
 * The sign-in the token file of the app holds: `signIn` through the browser, `pastedToken` a token of Account Security,
 * `expired` a browser sign-in whose access token expired and whose refresh token the Hub no longer knows, `signedOut`
 */
export type YouTrackSignedIn = "signIn" | "pastedToken" | "expired" | "signedOut"

/**
 * An issue of the fake YouTrack (`FakeIssue` of the youtrack crate): its state field `State` has the values Open, In
 * Progress, Fixed, Duplicate and Verified (the last three resolve it); `resolved` in Unix milliseconds
 */
export interface FakeYouTrackIssue {
  key: string
  summary: string
  state: string
  resolved?: number | null
  fields?: { name: string; value: string }[]
}

/**
 * What the fake YouTrack of a test repository holds (`YouTrackFixture` of the test server); a field left out changes
 * nothing. `me`: the user of every token (Jane Doe by default); `tokens`: tokens created in Account Security, which it
 * accepts besides those its Hub issued
 */
export interface YouTrackFixture {
  issues?: FakeYouTrackIssue[]
  me?: YouTrackUser
  tokens?: string[]
  signedIn?: YouTrackSignedIn
}

/**
 * The URL of the fake YouTrack (`http://127.0.0.1:<port>`, its Hub at `/hub`), whether a sign-in is in progress, and
 * what reached it in order: the commands applied, the keys of the issue lookups, the token of each `GET /api/users/me`
 */
export interface YouTrackLog {
  url: string
  pending: boolean
  commands: { query: string; issues: string[] }[]
  lookups: string[]
  meTokens: (string | null)[]
}

/** 2024-01-02 10:00Z, where the clock of every page starts (`fixtures/base-test.ts`), in Unix milliseconds */
const PAGE_CLOCK_START = Date.parse("2024-01-02T10:00:00Z")

/** An open issue of the fake YouTrack, with a priority and an assignee */
export function openIssue(key: string): FakeYouTrackIssue {
  return {
    key,
    summary: "The settings page loses the timeout",
    state: "Open",
    fields: [
      { name: "Priority", value: "Major" },
      { name: "Assignee", value: "Jane Doe" },
    ],
  }
}

/** `openIssue` resolved as `state` an hour before the page's clock starts */
export function resolvedIssue(key: string, state: "Fixed" | "Duplicate"): FakeYouTrackIssue {
  return { ...openIssue(key), state, resolved: PAGE_CLOCK_START - 3_600_000 }
}

/** The URL of the fake YouTrack of the repository */
export async function youTrackUrlOf(repo: TestRepositoryBuilder): Promise<string> {
  return (await repo.getYouTrack()).url
}

/** The YouTrack in a sentence, as the app says it ("Signed in to 127.0.0.1:<port> as Jane Doe") */
export function youTrackHost(url: string): string {
  return url.replace(/^https?:\/\//, "")
}

/**
 * The next sync links `[A-Z]+-\d+` to the issue pages of the YouTrack at `url`, as a repository of a YouTrack user does
 * (`.idea/vcs.xml`); the fixture of the templates links them to Jira, which other tests check
 */
export async function linkIssuesToYouTrack(page: Page, url: string): Promise<void> {
  await rewriteNextSyncEvents(page, event =>
    event.type === "issueNavigationConfig" ? { ...event, data: { config: { links: [{ issueRegexp: "\\b[A-Z]+-\\d+\\b", linkRegexp: `${url}/issue/$0` }] } } } : event,
  )
}

/**
 * The browser of the sign-in the app opened in `page`: it loads the Hub page, which redirects back to the app's
 * listener with a code at once, as for a user who approved before; the waiting sign-in trades the code for the tokens
 */
export async function completeYouTrackSignIn(page: Page): Promise<void> {
  const hubPage = (await getOpenedUrls(page)).findLast(url => url.includes("/hub/api/rest/oauth2/auth?"))
  expect(hubPage, "the Hub page the app opened").toBeDefined()
  const answer = await fetch(hubPage!)
  expect(answer.status).toBe(200)
  expect(await answer.text()).toContain("Signed in to YouTrack. You can close this tab and return to Branch Deck.")
}

/** The link of `key` in the row of the branch named so */
export function issueKeyLink(page: Page, key: string): Locator {
  return page.locator(`[data-testid="branch-row"][data-branch-name="${key}"]`).getByRole("link", { name: key }).first()
}

/** Settings at YouTrack: its page and the sign-in on it */
export async function openYouTrackSettings(page: Page): Promise<{ settings: Page; youtrack: Locator; auth: Locator }> {
  const settings = await openSettingsWindow(page, { page: "youtrack" })
  const youtrack = settings.getByTestId("settings-page-youtrack")
  return { settings, youtrack, auth: youtrack.getByTestId("youtrack-auth") }
}
