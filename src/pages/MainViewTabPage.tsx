import { useEffect, useRef, useSyncExternalStore } from "react"
import { LandingView } from "@/components/landing/LandingView"
import { Container } from "@/components/layout/Container"
import { RepositorySelector } from "@/components/repository/RepositorySelector"
import { ReviewsView } from "@/components/reviews/ReviewsView"
import { WindowTitleBar } from "@/components/shared/WindowTitleBar"
import { useSessionFactStores } from "@/hooks/useSessionFactStores"
import { useTauriListen } from "@/hooks/useTauriListen"
import { ensureSession } from "@/model/store"
import { useBranchSync } from "@/stores/branch-sync"
import { useSelectedProjectPath } from "@/stores/repository"
import { LINK_FIELD_FOCUS_EVENT, requestLinkFieldFocus } from "@/stores/reviews"

/** The views of the main window that are a tab of their own in the IDE beside the branches, the main view */
type TabPageView = "reviews" | "landing"

function subscribeVisibility(onChange: () => void): () => void {
  document.addEventListener("visibilitychange", onChange)
  return () => document.removeEventListener("visibilitychange", onChange)
}

/** The page shows: the IDE shows its tab */
function usePageVisible(): boolean {
  return useSyncExternalStore(subscribeVisibility, () => document.visibilityState === "visible")
}

/**
 * The page of the tab of Reviews or Landing in the tool window of the IDE (openspec/specs/repository/main-views/spec.md,
 * "In the IDE"): the repository the main view shows, under its selector, and the one view. The page holds the session
 * of that repository and the stores of its facts, as the review window does, and follows a pick of another repository
 * in any tab. What only the main view owns stays there: the Dock, the subscription that starts the sync of the session,
 * the update, the news of the session, the palette.
 */
function MainViewTabPage({ view }: { view: TabPageView }) {
  const repositoryPath = useSelectedProjectPath()
  useEffect(() => {
    if (repositoryPath !== undefined && repositoryPath !== "") {
      ensureSession(repositoryPath)
    }
  }, [repositoryPath])
  useSessionFactStores()
  // "Add Pull Request by Link…" of the palette of the main view
  useTauriListen(LINK_FIELD_FOCUS_EVENT, () => requestLinkFieldFocus())
  const isSyncing = useBranchSync(state => state.isSyncing)
  const visible = usePageVisible()
  const heading = useRef<HTMLHeadingElement>(null)
  return (
    <div className="flex h-dvh flex-col" data-testid={`${view}-tab`}>
      <WindowTitleBar>
        <div className="@container flex min-w-0 flex-1 items-center gap-3">
          <RepositorySelector disabled={isSyncing} />
        </div>
      </WindowTitleBar>
      <main className="min-h-0 flex-1 overflow-y-auto">
        <Container className="pb-2">
          {repositoryPath !== undefined && (view === "reviews" ? <ReviewsView visible={visible} headingRef={heading} /> : <LandingView visible={visible} headingRef={heading} />)}
        </Container>
      </main>
    </div>
  )
}

/** The page of the tab of Reviews in the IDE (`/reviews`) */
export function ReviewsTabPage() {
  return <MainViewTabPage view="reviews" />
}

/** The page of the tab of Landing in the IDE (`/landing`) */
export function LandingTabPage() {
  return <MainViewTabPage view="landing" />
}
