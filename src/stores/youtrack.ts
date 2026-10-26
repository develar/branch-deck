import { useEffect } from "react"
import { openUrl } from "@tauri-apps/plugin-opener"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import { useTauriListen } from "@/hooks/useTauriListen"
import { useWindowFocus } from "@/hooks/useWindowFocus"
import type { YouTrackAuthStatus, YouTrackIssueAnswer } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { compileIssuePatterns, parseTextSegments } from "@/lib/issue-links"
import { showToast } from "@/lib/toast-message"
import { isMainWindow } from "@/lib/window-label"
import { formatYouTrackHost, youTrackKeyOf } from "@/lib/youtrack"
import { getRecentProjects } from "@/stores/settings/repositorySettings"

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

/** An issue of a popover: asked when a popover shows it, the answer kept until the next one */
export type IssueEntry = YouTrackIssueAnswer

export interface YouTrackAuthState {
  /** null until asked */
  readonly status: YouTrackAuthStatus | null
  /** Why the last read of the status failed; null once one answers. The last status stays */
  readonly readError: string | null
  readonly signIn: YouTrackSignInPhase
  /** By issue key */
  readonly issues: Readonly<Record<string, IssueEntry>>
}

export const youTrackAuthStore = createStore<YouTrackAuthState>(() => ({ status: null, readError: null, signIn: { phase: "idle" }, issues: {} }))

/** The YouTrack the token is for while it is used; null signed out. The links of its issues open a popover. */
export function useYouTrackUrl(): string | null {
  return useStore(youTrackAuthStore, state => state.status?.url ?? null)
}

export function useIssueEntry(key: string): IssueEntry | undefined {
  return useStore(youTrackAuthStore, state => state.issues[key])
}

/**
 * The status of every window: asked once at the start, again on a sign-in or sign-out of any window, and after a failed
 * read on the next focus, so a failure never reads as signed out for the session
 */
export function useYouTrackAuthSync(): void {
  useEffect(() => {
    void refreshYouTrackAuthStatus()
  }, [])
  useTauriListen("youtrack-auth-changed", () => {
    youTrackAuthStore.setState({ issues: {} })
    void refreshYouTrackAuthStatus()
  })
  useWindowFocus(() => {
    if (youTrackAuthStore.getState().readError !== null) {
      void refreshYouTrackAuthStatus()
    }
  })
}

export function useYouTrackAuthStatus(): YouTrackAuthStatus | null {
  return useStore(youTrackAuthStore, state => state.status)
}

export function useYouTrackAuthReadError(): string | null {
  return useStore(youTrackAuthStore, state => state.readError)
}

export function useYouTrackSignIn(): YouTrackSignInPhase {
  return useStore(youTrackAuthStore, state => state.signIn)
}

/**
 * Asks the backend for the sign-in; a failure leaves the last status and says why (`readError`): in Settings, and in
 * the main window once per failure as an error with Retry (without a status the issue links show no popover and the
 * branch menus no Mark Fixed)
 */
export async function refreshYouTrackAuthStatus(): Promise<void> {
  const result = await commands.getYoutrackAuthStatus()
  const failedBefore = youTrackAuthStore.getState().readError !== null
  youTrackAuthStore.setState(result.status === "ok" ? { status: result.data, readError: null } : { readError: result.error.message })
  if (result.status === "error" && !failedBefore && isMainWindow) {
    notifyError("Cannot read the YouTrack sign-in", result.error, { children: "Retry", onClick: retryYouTrackAuthStatus })
  }
}

/** Retry of a failed read: the page says it reads again until the answer */
export function retryYouTrackAuthStatus(): void {
  youTrackAuthStore.setState({ readError: null })
  void refreshYouTrackAuthStatus()
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
    if (waited.error.code !== "canceled") {
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

/** Ends the wait; the backend answers the waiting call with the code `canceled`, which toasts nothing */
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

/**
 * Asks the backend for the issue (it keeps an answer a minute); a second ask while one runs waits for it. A sign-in
 * problem the answer reports is the status of every surface.
 */
export async function loadIssue(key: string): Promise<void> {
  if (loadingIssues.has(key)) {
    return
  }
  loadingIssues.add(key)
  const result = await commands.getYoutrackIssue(key).finally(() => loadingIssues.delete(key))
  const answer: YouTrackIssueAnswer = result.status === "ok" ? result.data : { status: "error", message: result.error.message }
  setIssue(key, answer)
  if (answer.status === "signInProblem") {
    void refreshYouTrackAuthStatus()
  }
}

/** The keys asked now */
const loadingIssues = new Set<string>()

function setIssue(key: string, entry: IssueEntry): void {
  youTrackAuthStore.setState(state => ({ issues: { ...state.issues, [key]: entry } }))
}

/** The key of the branch's issue when it is an issue of the YouTrack signed in to, by the issue links of the repository */
export function getYouTrackKeyOfIssue(issueKey: string | null): string | null {
  const url = youTrackAuthStore.getState().status?.url
  if (issueKey === null || url == null) {
    return null
  }
  const patterns = compileIssuePatterns(getRecentProjects()[0]?.issueNavigationConfig)
  const link = parseTextSegments(issueKey, patterns).find(segment => segment.type === "link")
  return link?.href === undefined ? null : youTrackKeyOf(link.href, url)
}

/** The value Mark Fixed sets: the resolved state of YouTrack's default workflow */
export const FIXED_STATE = "Fixed"
