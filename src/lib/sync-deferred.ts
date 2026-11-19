import type { ApiError, ErrorDetails } from "@/lib/bindings"

/** What a sync git refused says: its title, when the branches sync, and how loud */
export interface SyncDeferral {
  readonly title: string
  readonly description: string
  /**
   * `info` for a state of git the user ends as they work (a rebase finishes, a branch is checked out), shown where the
   * page has no table; `warning` for a history that no work of the user ends on its own (`unrelated_history`), shown
   * above the table too, as the table is the one of the sync before it
   */
  readonly tone: "info" | "warning"
}

/**
 * The words of a sync git refused before it built anything (`operation_in_progress`, `detached_head`,
 * `managed_branch`, `unrelated_history`): the toast of the user's own sync and the alert of the page say the same
 */
export function describeSyncDeferral(error: ApiError): SyncDeferral {
  return {
    title: error.message,
    description: deferralHint(error),
    tone: error.code === "unrelated_history" ? "warning" : "info",
  }
}

function deferralHint(error: ApiError): string {
  switch (error.code) {
    case "detached_head":
      return "Check out a branch to sync its commits"
    case "managed_branch":
      return "Check out your main branch to sync its commits"
    case "unrelated_history":
      return unrelatedHistoryHint(error.details)
    default:
      return "The branches sync once git finished it"
  }
}

/** Which side lacks what: the baseline a root of the branch, or the repository the parents a shallow fetch cut */
function unrelatedHistoryHint(details: ErrorDetails | null): string {
  if (details?.type !== "unrelatedHistory") {
    return "The branches sync once the commits ahead of the baseline build on it"
  }
  const root = details.root.slice(0, 8)
  if (details.shallow) {
    return `The parents of ${root} were not fetched: fetch the whole history of ${details.head} to sync its commits`
  }
  return `${details.baseline} lacks ${root}, a root commit of ${details.head}: the branches sync once ${details.baseline} holds every root of ${details.head}`
}
