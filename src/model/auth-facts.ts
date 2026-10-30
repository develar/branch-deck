import { notifyError } from "@/lib/errors"
import { refreshSessionFacts, sessionRepositoryPath } from "@/model/store"
import { authStore, refreshAuthStatus } from "@/stores/auth"

/*
 * The status of the sign-ins in a window of a repository (spec/repository/github-sign-in.spec.md, youtrack.spec.md):
 * the fact `auth` of its session, which `src/model/store.ts` puts into the store of `src/stores/auth.ts` with every
 * view (`applyAuthFacts`). Its session reads it again when the window asks (a popover whose YouTrack refused the
 * sign-in, Retry), and the main window tells a failed read. The side of the session: the root of a window never
 * imports it, so a window without a session loads no store of the main window.
 */

/**
 * Reads the status again: the session of the window reads its fact (`refresh_facts`, which every window of the session
 * then has), a window without one reads it itself
 */
export async function refreshWindowAuth(): Promise<void> {
  const repositoryPath = sessionRepositoryPath()
  if (repositoryPath === null) {
    await refreshAuthStatus()
  } else {
    await refreshSessionFacts(repositoryPath, "auth", "manual")
  }
}

/** Retry of a failed read of the window: the page says it reads again until the answer */
export function retryWindowAuth(): void {
  authStore.setState({ readError: null })
  void refreshWindowAuth()
}

/**
 * Tells each failure of a read of the status once, as the error "Cannot read the GitHub and YouTrack sign-ins" with
 * Retry, until a read answered (the main window, from its start: a session that answers at once may answer a failure);
 * returns the stop. Without a status the issue links show no popover, the menus no Mark Fixed and the hints of GitHub
 * no sign-in
 */
export function tellReadFailures(): () => void {
  return authStore.subscribe((state, before) => {
    if (state.readError !== null && before.readError === null) {
      notifyError("Cannot read the GitHub and YouTrack sign-ins", state.readError, { children: "Retry", onClick: retryWindowAuth })
    }
  })
}
