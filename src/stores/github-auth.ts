import { openUrl } from "@tauri-apps/plugin-opener"
import { useStore } from "zustand"
import { createStore } from "zustand/vanilla"
import type { GitHubAuthStatus } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { copyText } from "@/lib/clipboard"
import { notifyError } from "@/lib/errors"
import { showToast } from "@/lib/toast-message"

/*
 * The app's own GitHub sign-in (spec/repository/github-sign-in.spec.md): the device flow of the Branch Deck OAuth app,
 * shown in Settings. The token of `gh`, `GH_TOKEN` or git needs no sign-in; the status says which one the app takes.
 */

/** Where the sign-in is */
export type GitHubSignIn =
  | { phase: "idle" }
  | { phase: "starting" }
  /** The user enters `userCode` at `verificationUri` until `expiresAt` (`Date.now()` ms) */
  | { phase: "waiting"; userCode: string; verificationUri: string; expiresAt: number }

export interface GitHubAuthState {
  /** null until asked */
  readonly status: GitHubAuthStatus | null
  /** Why the last read of the status failed; null once one answers. The last status stays */
  readonly readError: string | null
  readonly signIn: GitHubSignIn
}

export const gitHubAuthStore = createStore<GitHubAuthState>(() => ({ status: null, readError: null, signIn: { phase: "idle" } }))

export function useGitHubAuthStatus(): GitHubAuthStatus | null {
  return useStore(gitHubAuthStore, state => state.status)
}

export function useGitHubAuthReadError(): string | null {
  return useStore(gitHubAuthStore, state => state.readError)
}

export function useGitHubSignIn(): GitHubSignIn {
  return useStore(gitHubAuthStore, state => state.signIn)
}

/** Asks the backend where the token comes from; a failure leaves the last status and says why (`readError`) */
export async function refreshGitHubAuthStatus(): Promise<void> {
  const result = await commands.getGithubAuthStatus()
  gitHubAuthStore.setState(result.status === "ok" ? { status: result.data, readError: null } : { readError: result.error.message })
}

/** Retry of a failed read: the page says it reads again until the answer */
export function retryGitHubAuthStatus(): void {
  gitHubAuthStore.setState({ readError: null })
  void refreshGitHubAuthStatus()
}

let loading: Promise<void> | null = null

/** Asks for the status unless it is known or asked already: for the button of a hint, which needs only what the build is */
export function loadGitHubAuthStatus(): Promise<void> {
  if (gitHubAuthStore.getState().status !== null) {
    return Promise.resolve()
  }
  loading ??= refreshGitHubAuthStatus().finally(() => {
    loading = null
  })
  return loading
}

/**
 * Starts the device flow and waits for the user to enter the code on GitHub; then "Signed in to GitHub as @octocat".
 * The backend tells every window (`github-auth-changed`), which asks GitHub again.
 */
export async function startGitHubSignIn(): Promise<void> {
  if (gitHubAuthStore.getState().signIn.phase !== "idle") {
    return
  }
  gitHubAuthStore.setState({ signIn: { phase: "starting" } })
  const started = await commands.startGithubSignIn()
  if (started.status === "error") {
    gitHubAuthStore.setState({ signIn: { phase: "idle" } })
    notifyError("Cannot sign in to GitHub", started.error)
    return
  }
  const { userCode, verificationUri, expiresIn } = started.data
  gitHubAuthStore.setState({ signIn: { phase: "waiting", userCode, verificationUri, expiresAt: Date.now() + expiresIn * 1000 } })
  const waited = await commands.waitGithubSignIn()
  // a newer sign-in replaced this one
  const current = gitHubAuthStore.getState().signIn
  if (current.phase === "waiting" && current.userCode !== userCode) {
    return
  }
  gitHubAuthStore.setState({ signIn: { phase: "idle" } })
  if (waited.status === "error") {
    if (waited.error.code !== "canceled") {
      notifyError("Cannot sign in to GitHub", waited.error)
    }
    return
  }
  showToast({ title: `Signed in to GitHub as @${waited.data}` })
  await refreshGitHubAuthStatus()
}

/** Copies the code and opens the page of GitHub where it is entered */
export async function copyCodeAndOpenGitHub(): Promise<void> {
  const signIn = gitHubAuthStore.getState().signIn
  if (signIn.phase !== "waiting") {
    return
  }
  await copyText(signIn.userCode)
  await openUrl(signIn.verificationUri).catch((error: unknown) => notifyError("Cannot open GitHub", error))
}

/** Ends the wait; the backend answers the waiting call with the code `canceled`, which toasts nothing */
export async function cancelGitHubSignIn(): Promise<void> {
  gitHubAuthStore.setState({ signIn: { phase: "idle" } })
  await commands.cancelGithubSignIn()
}

/** Removes the token of the app's own sign-in; GitHub keeps the authorization until it is revoked there */
export async function signOutOfGitHub(): Promise<void> {
  const result = await commands.signOutOfGithub()
  if (result.status === "error") {
    notifyError("Cannot sign out of GitHub", result.error)
    return
  }
  showToast({ title: "Signed out of GitHub", description: "GitHub keeps the authorization of Branch Deck until you revoke it there" })
  await refreshGitHubAuthStatus()
}
