import type { PullRequestAvailability } from "@/lib/bindings"
import { getAvailabilityAction } from "@/lib/pull-request"
import { useAuthStatus } from "@/stores/auth"

/**
 * The button after the hint of an availability (`getAvailabilityAction`). A missing or rejected token needs to know
 * whether this build signs in ("Sign In…") or not ("Settings…"), the token of the app's sign-in that can't see the
 * repository the page of the app on GitHub: the status of the sign-ins, which the session of the window reads when it
 * opens (the fact `auth`); the button waits for it.
 */
export function useAvailabilityAction(availability: PullRequestAvailability | null): { label: string; onClick: () => void } | null {
  return getAvailabilityAction(availability, useAuthStatus()?.github ?? null)
}
