import type { ApiError } from "@/lib/bindings"

/** What a sync git refused says: its title, and when the branches sync */
export interface SyncDeferral {
  readonly title: string
  readonly description: string
}

/**
 * The words of a sync git refused before it built anything (`operation_in_progress`, `detached_head`): the toast of the
 * user's own sync and the alert of a page without a table say the same
 */
export function describeSyncDeferral(error: ApiError): SyncDeferral {
  return {
    title: error.message,
    description: error.code === "detached_head" ? "Check out a branch to sync its commits" : "The branches sync once git finished it",
  }
}
