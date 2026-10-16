import { useEffect } from "react"
import type { PullRequestAvailability } from "@/lib/bindings"
import { getAvailabilityAction } from "@/lib/pull-request"
import { loadGitHubAuthStatus, useGitHubAuthStatus } from "@/stores/github-auth"

/**
 * The button after the hint of an availability (`getAvailabilityAction`). A missing or rejected token needs to know
 * whether this build signs in ("Sign In…") or not ("Settings…"), the token of the app's sign-in that can't see the
 * repository the page of the app on GitHub: the status is asked once, when such a hint shows.
 */
export function useAvailabilityAction(availability: PullRequestAvailability | null): { label: string, onClick: () => void } | null {
  const auth = useGitHubAuthStatus()
  const needsAuth = availability?.status === "signedOut" || availability?.status === "notAuthenticated"
    || (availability?.status === "noAccess" && availability.source === "branchDeck")
  useEffect(() => {
    if (needsAuth && auth === null) {
      void loadGitHubAuthStatus()
    }
  }, [needsAuth, auth])
  return getAvailabilityAction(availability, auth)
}
