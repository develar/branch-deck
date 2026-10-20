import { openUrl } from "@tauri-apps/plugin-opener"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { YouTrackAuthStatus } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { showToast } from "@/lib/toast-message"
import { formatYouTrackHost } from "@/lib/youtrack"

/*
 * The YouTrack sign-in (spec/repository/youtrack.spec.md): in the browser through the Hub of the YouTrack, or a pasted
 * token. Every window knows the status: the issue popovers ask only the YouTrack the token is for.
 */

/** Where the browser sign-in is */
export type YouTrackSignInPhase =
  | { phase: "idle" }
  | { phase: "starting" }
  /** The Hub page is open in the browser; the backend waits for it to come back */
  | { phase: "waiting"; authorizeUrl: string }

export interface YouTrackAuthState {
  /** null until asked */
  readonly status: YouTrackAuthStatus | null
  readonly signIn: YouTrackSignInPhase
}

export const youTrackAuthStore = createStore<YouTrackAuthState>(() => ({ status: null, signIn: { phase: "idle" } }))

export function useYouTrackAuthStatus(): YouTrackAuthStatus | null {
  return useStore(youTrackAuthStore, state => state.status)
}

export function useYouTrackSignIn(): YouTrackSignInPhase {
  return useStore(youTrackAuthStore, state => state.signIn)
}

/** Asks the backend for the sign-in; a failure leaves the last status */
export async function refreshYouTrackAuthStatus(): Promise<void> {
  const result = await commands.getYoutrackAuthStatus()
  if (result.status === "ok") {
    youTrackAuthStore.setState({ status: result.data })
  }
}

/**
 * Opens the Hub page of the YouTrack at `url` in the browser and waits for it to come back; then "Signed in to
 * youtrack.example.com as Jane Doe". The backend tells every window (`youtrack-auth-changed`).
 */
export async function startYouTrackSignIn(url: string): Promise<void> {
  if (youTrackAuthStore.getState().signIn.phase !== "idle") {
    return
  }
  youTrackAuthStore.setState({ signIn: { phase: "starting" } })
  const started = await commands.startYoutrackSignIn(url)
  if (started.status === "error") {
    youTrackAuthStore.setState({ signIn: { phase: "idle" } })
    notifyError("Cannot sign in to YouTrack", started.error)
    return
  }
  const { authorizeUrl } = started.data
  youTrackAuthStore.setState({ signIn: { phase: "waiting", authorizeUrl } })
  await openYouTrackSignInPage()
  const waited = await commands.waitYoutrackSignIn()
  // a newer sign-in replaced this one
  const current = youTrackAuthStore.getState().signIn
  if (current.phase === "waiting" && current.authorizeUrl !== authorizeUrl) {
    return
  }
  youTrackAuthStore.setState({ signIn: { phase: "idle" } })
  if (waited.status === "error") {
    if (waited.error.message !== "canceled") {
      notifyError("Cannot sign in to YouTrack", waited.error)
    }
    return
  }
  await refreshYouTrackAuthStatus()
  showToast({ title: signedInTitle(waited.data.fullName) })
}

/** Opens the Hub page of the sign-in in progress (again, after the tab was closed) */
export async function openYouTrackSignInPage(): Promise<void> {
  const signIn = youTrackAuthStore.getState().signIn
  if (signIn.phase === "waiting") {
    await openUrl(signIn.authorizeUrl).catch((error: unknown) => notifyError("Cannot open the YouTrack sign-in", error))
  }
}

/** Ends the wait; the backend answers the waiting call with "canceled" */
export async function cancelYouTrackSignIn(): Promise<void> {
  youTrackAuthStore.setState({ signIn: { phase: "idle" } })
  await commands.cancelYoutrackSignIn()
}

/** Keeps a token the user created in YouTrack, once YouTrack accepted it; false when it did not */
export async function saveYouTrackToken(url: string, token: string): Promise<boolean> {
  const result = await commands.saveYoutrackToken(url, token)
  if (result.status === "error") {
    notifyError("Cannot sign in to YouTrack", result.error)
    return false
  }
  await refreshYouTrackAuthStatus()
  showToast({ title: signedInTitle(result.data.fullName) })
  return true
}

/** Removes the token of the app's own sign-in; the Hub keeps the authorization until it is revoked there */
export async function signOutOfYouTrack(): Promise<void> {
  const result = await commands.signOutOfYoutrack()
  if (result.status === "error") {
    notifyError("Cannot sign out of YouTrack", result.error)
    return
  }
  showToast({ title: "Signed out of YouTrack", description: "YouTrack keeps the token until you revoke it in Account Security" })
  await refreshYouTrackAuthStatus()
}

function signedInTitle(fullName: string): string {
  const url = youTrackAuthStore.getState().status?.url
  return url == null ? `Signed in to YouTrack as ${fullName}` : `Signed in to ${formatYouTrackHost(url)} as ${fullName}`
}
