import type { ApiError } from "@/lib/bindings"

/** What a sync git refused says: its title, and when the branches sync */
export interface SyncDeferral {
  readonly title: string
  readonly description: string
}

/**
 * The words of a sync git refused before it built anything (`operation_in_progress`, `detached_head`,
 * `managed_branch`): the toast of the user's own sync and the alert of a page without a table say the same
 */
export function describeSyncDeferral(error: ApiError): SyncDeferral {
  return {
    title: error.message,
    description: deferralHint(error.code),
  }
}

function deferralHint(code: ApiError["code"]): string {
  switch (code) {
    case "detached_head":
      return "Check out a branch to sync its commits"
    case "managed_branch":
      return "Check out your main branch to sync its commits"
    default:
      return "The branches sync once git finished it"
  }
}
