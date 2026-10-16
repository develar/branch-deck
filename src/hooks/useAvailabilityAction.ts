import { useEffect } from "react"
import type { PullRequestAvailability } from "@/lib/bindings"
import { getAvailabilityAction } from "@/lib/pull-request"
import { loadGitHubAuthStatus, useGitHubSignInAvailable } from "@/stores/github-auth"

/**
 * The button after the hint of an availability (`getAvailabilityAction`). A missing or rejected token needs to know
 * whether this build signs in ("Sign In…") or not ("Settings…"): the status is asked once, when such a hint shows.
 */
export function useAvailabilityAction(availability: PullRequestAvailability | null): { label: string, onClick: () => void } | null {
  const signInAvailable = useGitHubSignInAvailable()
  const needsToken = availability?.status === "signedOut" || availability?.status === "notAuthenticated"
  useEffect(() => {
    if (needsToken && signInAvailable === null) {
      void loadGitHubAuthStatus()
    }
  }, [needsToken, signInAvailable])
  return getAvailabilityAction(availability, signInAvailable)
}
