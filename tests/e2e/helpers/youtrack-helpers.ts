import type { Locator, Page } from "@playwright/test"
import type { YouTrackAuthStatus } from "../../../src/lib/bindings"
import { openSettingsWindow } from "./settings-helpers"
import { rewriteNextSyncEvents } from "./sync-helpers"
import { routeCommand } from "./wait-helpers"

/** The YouTrack of the tests */
export const YOUTRACK_URL = "https://youtrack.example.com"

export const SIGNED_OUT: YouTrackAuthStatus = { url: null, signedInAs: null, source: null, problem: null, signedInAt: null, manageUrl: null }
export const SIGNED_IN: YouTrackAuthStatus = {
  url: YOUTRACK_URL,
  signedInAs: { login: "jane", fullName: "Jane Doe" },
  source: "signIn",
  problem: null,
  signedInAt: 1_704_117_600,
  // a Hub of its own, as hub.jetbrains.com of youtrack.jetbrains.com
  manageUrl: "https://hub.example.com/users/me",
}

/**
 * The next sync links `[A-Z]+-\d+` to the issue pages of the YouTrack of the tests, as a repository of a YouTrack user
 * does (`.idea/vcs.xml`); the fixture of the templates links them to Jira, which other tests check
 */
export async function linkIssuesToYouTrack(page: Page): Promise<void> {
  await rewriteNextSyncEvents(page, event =>
    event.type === "issueNavigationConfig" ? { ...event, data: { config: { links: [{ issueRegexp: "\\b[A-Z]+-\\d+\\b", linkRegexp: `${YOUTRACK_URL}/issue/$0` }] } } } : event,
  )
}

/** Answers `get_youtrack_auth_status` in the page with `status()`, asked on each call */
export async function routeYouTrackStatus(page: Page, status: () => YouTrackAuthStatus): Promise<void> {
  await routeCommand(page, "get_youtrack_auth_status", () => ({ status: "ok", data: status() }))
}

/**
 * Answers the browser sign-in in the page: the Hub page of `start_youtrack_sign_in`, and a wait that ends with Jane Doe
 * when the returned `finish` is called (with "canceled" after a cancel)
 */
export async function routeYouTrackSignIn(page: Page): Promise<{ finish: () => void; starts: Record<string, unknown>[]; cancels: Record<string, unknown>[] }> {
  const starts = await routeCommand(page, "start_youtrack_sign_in", args => ({
    status: "ok",
    data: { authorizeUrl: `${String(args.url)}/hub/api/rest/oauth2/auth?client_id=localhost-auth` },
  }))
  let finish: (answer: unknown) => void = () => {}
  await page.route("**/invoke/wait_youtrack_sign_in", async route => {
    const answer = await new Promise(resolve => {
      finish = resolve
    })
    await route.fulfill({ json: answer })
  })
  const cancels = await routeCommand(page, "cancel_youtrack_sign_in", () => {
    finish({ status: "error", error: { message: "canceled" } })
    return { status: "ok", data: null }
  })
  return { finish: () => finish({ status: "ok", data: SIGNED_IN.signedInAs }), starts, cancels }
}

/** Settings at YouTrack, its status `status` until `setStatus`, the browser sign-in answered in the page */
export async function openYouTrackSettings(
  page: Page,
  status: YouTrackAuthStatus,
): Promise<{ settings: Page; youtrack: Locator; auth: Locator; signIn: Awaited<ReturnType<typeof routeYouTrackSignIn>>; setStatus: (status: YouTrackAuthStatus) => void }> {
  let current = status
  let signIn: Awaited<ReturnType<typeof routeYouTrackSignIn>> | null = null
  const settings = await openSettingsWindow(
    page,
    { page: "youtrack" },
    {
      beforeLoad: async window => {
        await routeYouTrackStatus(window, () => current)
        signIn = await routeYouTrackSignIn(window)
      },
    },
  )
  const youtrack = settings.getByTestId("settings-page-youtrack")
  return {
    settings,
    youtrack,
    auth: youtrack.getByTestId("youtrack-auth"),
    signIn: signIn!,
    setStatus: next => {
      current = next
    },
  }
}
