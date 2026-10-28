import { useEffect } from "react"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import { useTauriListen } from "@/hooks/useTauriListen"
import type { AuthStatus, FactsChanges, FactsView, SignInProvider } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { BRANCH_REVIEW_WINDOW, isMainWindow, windowLabel } from "@/lib/window-label"
import { onSessionFacts, refreshSessionFacts, sessionRepositoryPath } from "@/model/store"

/*
 * The sign-ins of the app's accounts (spec/repository/github-sign-in.spec.md, youtrack.spec.md): the one status of both
 * accounts and where each sign-in of this window is. A window of a repository (the main window, the review window of a
 * branch) has the status as the fact `auth` of its session (`applyAuthFacts`: the session reads it when a window opens
 * it, after a sign-in or a sign-out of any window and on a focus by its rule); any other window reads it itself
 * (`get_auth_status`: at its start, on each later open of Settings, and on `auth-changed`, which the backend tells every
 * window after a sign-in or a sign-out). `useAuthSync` of `App` keeps it so in every window.
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

/**
 * Reads the status again: the session of the window reads its fact (`refresh_facts`, which every window of the session
 * then has), a window without one reads it itself
 */
export async function refreshWindowAuth(): Promise<void> {
  const repositoryPath = takesSessionFact ? sessionRepositoryPath() : null
  if (repositoryPath === null) {
    await refreshAuthStatus()
  } else {
    await refreshSessionFacts(repositoryPath, "auth", "manual")
  }
}

/** Retry of a failed read: the page says it reads again until the answer */
export function retryAuthStatus(): void {
  authStore.setState({ readError: null })
  void refreshWindowAuth()
}

/**
 * The status in every window (`App`): from the session of a window of a repository; read by any other at its start and
 * on a sign-in or a sign-out of any window (`auth-changed`). The main window tells a failed read once per failure, as
 * the error "Cannot read the GitHub and YouTrack sign-ins" with Retry: without a status the issue links show no
 * popover, the menus no Mark Fixed and the hints of GitHub no sign-in
 */
export function useAuthSync(): void {
  // before the first status comes: a session that answers at once may answer a failure
  useEffect(() => (isMainWindow ? tellReadFailures() : undefined), [])
  useEffect(() => {
    if (!takesSessionFact) {
      void refreshAuthStatus()
      return
    }
    return onSessionFacts((_repositoryPath, view, changes) => applyAuthFacts(view.facts, changes?.facts ?? null))
  }, [])
  useTauriListen("auth-changed", () => {
    if (!takesSessionFact) {
      void refreshAuthStatus()
    }
  })
}

/**
 * Tells each failure of a read of the status once, as the error "Cannot read the GitHub and YouTrack sign-ins" with
 * Retry, until a read answered; returns the stop
 */
export function tellReadFailures(): () => void {
  return authStore.subscribe((state, before) => {
    if (state.readError !== null && before.readError === null) {
      notifyError("Cannot read the GitHub and YouTrack sign-ins", state.readError, { children: "Retry", onClick: retryAuthStatus })
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
