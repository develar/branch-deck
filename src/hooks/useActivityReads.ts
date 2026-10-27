import { useEffect } from "react"
import { useTauriListen } from "@/hooks/useTauriListen"
import type { GitHubRepository } from "@/lib/pull-request"
import { linkActivityPullRequests, refreshActivity } from "@/stores/activity"

/** The event the backend tells every window when a journal Activity reads changed (`activity-changed`: an operation ended, a write) */
const ACTIVITY_CHANGED = "activity-changed"

/**
 * The reads of the log of a window that shows Activity, the main window and the Activity window alike
 * (spec/repository/activity.spec.md): the log is read when the component mounts (each open of the window mounts it
 * anew) and again whenever a journal changed; the pull requests of the entries link in `gitHubRepository`, the GitHub
 * repository of the shown repository
 */
export function useActivityReads(gitHubRepository: GitHubRepository | null): void {
  const owner = gitHubRepository?.owner
  const name = gitHubRepository?.name
  useEffect(() => linkActivityPullRequests(owner === undefined || name === undefined ? null : { owner, name }), [owner, name])
  useEffect(() => void refreshActivity(), [])
  useTauriListen<null>(ACTIVITY_CHANGED, () => void refreshActivity())
}
