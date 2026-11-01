import { useEffect } from "react"
import type { RepositoryView, TableChanges } from "@/lib/bindings"
import { onSessionFacts } from "@/model/store"
import { applySessionFacts } from "@/stores/patronus"
import { applyPullRequestsFacts } from "@/stores/pull-requests"
import { applyReadinessFacts } from "@/stores/review-readiness"
import { applyReviewRequestsFacts } from "@/stores/review-requests"
import { applyReviewsView } from "@/stores/reviews"
import { applyRobotDetailsFacts } from "@/stores/robot-details"

/**
 * Fills the stores of the facts of the window's session from its view, as the session patches it: the robots of
 * Patronus and what the session derives of them (`applySessionFacts`), the pull requests, the review readiness, the
 * details of a robot, the review requests and the Reviews view. Pure appliers: no toast, no OS notification, no record
 * in Activity, so every window that holds a session (`ensureSession`: the main window and the review window) mounts
 * it, and the builders of the actions read the same stores in both. The news of the session are the main window's
 * (`useReviewStatusRefresh`), which mounts this hook first, so the news read the stores already applied.
 */
export function useSessionFactStores(): void {
  useEffect(() => onSessionFacts(applySessionFactStores), [])
}

/** The stores of the facts take the view of the session of `repositoryPath`, or its changes; never its news */
export function applySessionFactStores(repositoryPath: string, view: RepositoryView, changes: TableChanges | null): void {
  applySessionFacts(repositoryPath, view, changes)
  applyPullRequestsFacts(repositoryPath, view.facts, changes?.facts ?? null)
  applyReadinessFacts(repositoryPath, view.facts, changes?.facts ?? null)
  applyRobotDetailsFacts(repositoryPath, view.facts, changes?.facts ?? null)
  applyReviewRequestsFacts(repositoryPath, view.facts, changes?.facts ?? null)
  applyReviewsView(repositoryPath, view, changes)
}
