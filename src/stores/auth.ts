import { useEffect } from "react"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import { useTauriListen } from "@/hooks/useTauriListen"
import type { AuthStatus, FactsChanges, FactsView, SignInProvider } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { BRANCH_REVIEW_WINDOW, isMainWindow, windowLabel } from "@/lib/window-label"

/*
 * The sign-ins of the app's accounts (spec/repository/github-sign-in.spec.md, youtrack.spec.md): the one status of both
 * accounts and where each sign-in of this window is. A window of a repository (the main window, the review window of a
 * branch) has the status as the fact `auth` of its session: the store of the session applies it with every view
 * (`applyAuthFacts`, called by `src/model/store.ts`; the session reads it when a window opens it, after a sign-in or a
 * sign-out of any window and on a focus by its rule), and the window asks its session to read it again
 * (`src/model/auth-facts.ts`); any other window reads it itself (`get_auth_status`: at its start, on each later open
 * of Settings, and on `auth-changed`, which the backend tells every window after a sign-in or a sign-out), which
 * `useAuthSync` of `App` keeps so. The root of every window loads this module: it imports nothing of the session.
 */

/** Where a sign-in of this window is (`src/lib/sign-in-run.ts` moves it by the events of its channel) */
export type SignInPhase =
  | { phase: "idle" }
  /** It asks the service: the user has nothing to do yet */
  | { phase: "waiting" }
  /** GitHub waits until the user entered `code` at `url`, until `expiresAt` (`Date.now()` ms) */
  | { phase: "code"; code: string; url: string; expiresAt: number }
  /** The Hub waits until the user signed in on the page at `url` in the browser */
  | { phase: "browser"; url: string }

export interface AuthState {
  /** null until the first answer */
  readonly status: AuthStatus | null
  /** Why the last read failed; null once one answers. The last status stays */
  readonly readError: string | null
  /** The sign-ins of this window by provider; one it never ran is idle */
  readonly signIns: Readonly<Partial<Record<SignInProvider, SignInPhase>>>
}

export const authStore = createStore<AuthState>(() => ({ status: null, readError: null, signIns: {} }))

export const IDLE_SIGN_IN: SignInPhase = { phase: "idle" }

export function useAuthStatus(): AuthStatus | null {
  return useStore(authStore, state => state.status)
}

/** The YouTrack the token is for while it is used; null signed out or before the first answer. Its issue links show the issue */
export function useYouTrackUrl(): string | null {
  return useStore(authStore, state => state.status?.youtrack.url ?? null)
}

export function useAuthReadError(): string | null {
  return useStore(authStore, state => state.readError)
}

export function useSignInPhase(provider: SignInProvider): SignInPhase {
  return useStore(authStore, state => state.signIns[provider] ?? IDLE_SIGN_IN)
}

export function getSignInPhase(provider: SignInProvider): SignInPhase {
  return authStore.getState().signIns[provider] ?? IDLE_SIGN_IN
}

export function setSignInPhase(provider: SignInProvider, phase: SignInPhase): void {
  authStore.setState(state => ({ signIns: { ...state.signIns, [provider]: phase } }))
}

/**
 * Takes the status from the facts of the session of the window (`change` null for a snapshot): the answer of its last
 * read, and why that read failed; a session that has not read it yet leaves the status the window had (it is the app's)
 */
export function applyAuthFacts(facts: FactsView, change: FactsChanges | null): void {
  if (change !== null && change.auth == null) {
    return
  }
  const { value, state } = facts.auth
  const readError = state.kind === "unavailable" ? state.reason.message : state.kind === "stale" ? (state.reason?.message ?? null) : null
  const before = authStore.getState()
  const status = value ?? before.status
  if (status !== before.status || readError !== before.readError) {
    authStore.setState({ status, readError })
  }
}

/** Reads the status, in a window without a session; a failure leaves the last status and says why (`readError`) */
export async function refreshAuthStatus(): Promise<void> {
  const result = await commands.getAuthStatus()
  authStore.setState(result.status === "ok" ? { status: result.data, readError: null } : { readError: result.error.message })
}

/** The windows that hold the session of a repository (`ensureSession`) and take the status from its fact */
const takesSessionFact = isMainWindow || windowLabel === BRANCH_REVIEW_WINDOW

/** Retry of a failed read of a window without a session (Settings): the page says it reads again until the answer */
export function retryAuthStatus(): void {
  authStore.setState({ readError: null })
  void refreshAuthStatus()
}

/**
 * The status in a window without a session (`App` of every window): read at its start and on a sign-in or a sign-out
 * of any window (`auth-changed`). A window of a repository takes it from the fact of its session (`applyAuthFacts`),
 * which the session reads again after a sign-in or a sign-out itself
 */
export function useAuthSync(): void {
  useEffect(() => {
    if (!takesSessionFact) {
      void refreshAuthStatus()
    }
  }, [])
  useTauriListen("auth-changed", () => {
    if (!takesSessionFact) {
      void refreshAuthStatus()
    }
  })
}

/** Settings reads the status again on each later open of its window (`openCount` above 0): the start read it once */
export function useAuthStatusOnOpen(openCount: number): void {
  useEffect(() => {
    if (openCount > 0) {
      void refreshAuthStatus()
    }
  }, [openCount])
}
