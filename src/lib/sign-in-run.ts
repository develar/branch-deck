// The sign-ins of the app's accounts from the page (spec/repository/github-sign-in.spec.md, youtrack.spec.md):
// `sign_in` runs one in Rust (`remote_facts::sign_in`), or joins the one of its provider that runs, and tells its events
// on the channel of the call; the page shows where it is (`SignInPhase` of `src/stores/auth.ts`), opens the page of the
// Hub in the browser when the sign-in waits for it, and words how it ended. The status that follows a sign-in or a
// sign-out comes as the fact `auth` of the session, or by `auth-changed` in a window without one.
import { Channel } from "@tauri-apps/api/core"
import { openUrl } from "@tauri-apps/plugin-opener"
import type { Account, SignIn, SignInDone, SignInEvent, SignInProvider } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { copyText } from "@/lib/clipboard"
import { getErrorCode, notifyError } from "@/lib/errors"
import { showToast } from "@/lib/toast-message"
import { formatYouTrackHost } from "@/lib/youtrack"
import type { SignInPhase } from "@/stores/auth"
import { IDLE_SIGN_IN, getSignInPhase, setSignInPhase } from "@/stores/auth"

/** The name of an account in the words of a toast */
const PROVIDER_NAMES: Record<SignInProvider, string> = { gitHub: "GitHub", youTrack: "YouTrack", youTrackCli: "youtrack" }

/** What a sign-in shows after `event`, which came at `now` (`Date.now()` ms) */
export function phaseOf(event: SignInEvent, now: number): SignInPhase {
  switch (event.kind) {
    case "waiting":
      return { phase: "waiting" }
    case "deviceCode":
      return { phase: "code", code: event.code, url: event.url, expiresAt: now + event.expiresIn * 1000 }
    case "browser":
      return { phase: "browser", url: event.url }
    case "done":
      return IDLE_SIGN_IN
  }
}

/** The toast of a sign-in: "Signed in to GitHub as @octocat", "Signed in to youtrack.example.com as Jane Doe" */
export function signedInTitle(done: SignInDone): string {
  switch (done.provider) {
    case "gitHub":
      return `Signed in to GitHub as @${done.login}`
    case "youTrack":
      return `Signed in to ${formatYouTrackHost(done.url)} as ${done.user.fullName}`
    case "youTrackCli":
      return done.title
  }
}

/**
 * Signs in by `request`, one run of each provider in this window: its phase follows the events of its channel, then
 * the toast of how it ended; a cancel says nothing (the code `canceled`, never its words), a failure "Cannot sign in to
 * GitHub" with the reason. Resolves to how it ended, null when it did not
 */
export async function runSignIn(request: SignIn): Promise<SignInDone | null> {
  const provider = request.provider
  if (getSignInPhase(provider).phase !== "idle") {
    return null
  }
  setSignInPhase(provider, { phase: "waiting" })
  const events = new Channel<SignInEvent>()
  events.onmessage = event => {
    setSignInPhase(provider, phaseOf(event, Date.now()))
    if (event.kind === "browser") {
      void openSignInPage(event.url)
    }
  }
  const result = await commands.signIn(request, events)
  setSignInPhase(provider, IDLE_SIGN_IN)
  if (result.status === "error") {
    if (getErrorCode(result.error) !== "canceled") {
      notifyError(`Cannot sign in to ${PROVIDER_NAMES[provider]}`, result.error)
    }
    return null
  }
  showToast({ title: signedInTitle(result.data) })
  return result.data
}

/** Ends the sign-in of `provider`: its call answers `canceled`, which toasts nothing */
export async function cancelSignIn(provider: SignInProvider): Promise<void> {
  setSignInPhase(provider, IDLE_SIGN_IN)
  await commands.cancelSignIn(provider)
}

/** Opens the page of the Hub in the browser, where the user signs in to YouTrack */
export async function openSignInPage(url: string): Promise<void> {
  await openUrl(url).catch((error: unknown) => notifyError("Cannot open the YouTrack sign-in", error))
}

/** Opens the page of the sign-in of `provider` that waits for the browser again (after the tab was closed) */
export async function openSignInPageAgain(provider: SignInProvider): Promise<void> {
  const signIn = getSignInPhase(provider)
  if (signIn.phase === "browser") {
    await openSignInPage(signIn.url)
  }
}

/** Copies the code of the sign-in to GitHub and opens the page of GitHub where it is entered */
export async function copyCodeAndOpenGitHub(): Promise<void> {
  const signIn = getSignInPhase("gitHub")
  if (signIn.phase !== "code") {
    return
  }
  await copyText(signIn.code)
  await openUrl(signIn.url).catch((error: unknown) => notifyError("Cannot open GitHub", error))
}

/** What a sign-out says the service keeps */
const KEPT_AFTER_SIGN_OUT: Record<Account, string> = {
  gitHub: "GitHub keeps the authorization of Branch Deck until you revoke it there",
  youTrack: "YouTrack keeps the token until you revoke it in Account Security",
}

/** Removes the token of the app's own sign-in to `account`; the service keeps the authorization until it is revoked there */
export async function signOut(account: Account): Promise<void> {
  const name = PROVIDER_NAMES[account]
  const result = await commands.signOut(account)
  if (result.status === "error") {
    notifyError(`Cannot sign out of ${name}`, result.error)
    return
  }
  showToast({ title: `Signed out of ${name}`, description: KEPT_AFTER_SIGN_OUT[account] })
}
