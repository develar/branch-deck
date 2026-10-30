import type { ComponentType } from "react"

// The page of each window by the path of its URL. A sub-window opens on the URL of its page (`openSubWindow` in
// `src/lib/window-management.ts`); the main window opens on `/` and may carry `?repoId=` (E2E), which the pages read
// themselves. Every page is a chunk of its own: a sub-window shares this bundle and never loads the main window's tree.
const PAGES: Readonly<Record<string, () => Promise<ComponentType>>> = {
  // the rows of the frame the start injected are in the store before the first render, and the window subscribes
  // holding them (`adoptCachedModel`)
  "/": async () => {
    const [{ IndexPage }, { adoptCachedModel }] = await Promise.all([import("@/pages/IndexPage"), import("@/model/store")])
    adoptCachedModel(window.__BD_MODEL__)
    return IndexPage
  },
  "/conflicting-files": async () => (await import("@/pages/ConflictingFilesPage")).ConflictingFilesPage,
  "/missing-commits": async () => (await import("@/pages/MissingCommitsPage")).MissingCommitsPage,
  "/branch-review": async () => (await import("@/pages/BranchReviewPage")).BranchReviewPage,
  "/pull-request-review": async () => (await import("@/pages/PullRequestReviewPage")).PullRequestReviewPage,
  "/commit-review": async () => (await import("@/pages/CommitReviewPage")).CommitReviewPage,
  "/settings": async () => (await import("@/pages/SettingsPage")).SettingsPage,
  "/activity": async () => (await import("@/pages/ActivityPage")).ActivityPage,
}

/**
 * Loads the page of the window at `pathname`; the root renders once it is here, with it (`src/main.tsx`). Not a
 * `React.lazy` under `Suspense`: its fallback commits before the chunk comes, and React reveals the content of a
 * boundary that showed its fallback no sooner than 300 ms after it (every window showed 0.3 s later). Rejects for a
 * path that has no page.
 */
export function loadWindowPage(pathname: string): Promise<ComponentType> {
  const load = Object.hasOwn(PAGES, pathname) ? PAGES[pathname] : undefined
  return load === undefined ? Promise.reject(new Error(`No page at ${pathname}`)) : load()
}
