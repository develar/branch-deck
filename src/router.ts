import { createBrowserRouter } from "react-router"
import { App } from "@/App"

// A sub-window opens on the URL of its page (`openSubWindow` in `src/lib/window-management.ts`): `/conflicting-files`,
// `/missing-commits`, `/branch-review`, `/pull-request-review`, `/commit-review`, `/settings` and `/activity`.
// The main window may carry `?repoId=` (E2E); the browser router keeps search params untouched.
export const router = createBrowserRouter([
  {
    path: "/",
    Component: App,
    // rendered while a lazy sub-window page loads on initial navigation
    HydrateFallback: () => null,
    children: [
      // lazy as the sub-window pages: a sub-window shares this bundle and never needs the main window's tree. The rows
      // of the frame the start injected are in the store before the first render, and the window subscribed holding
      // them (`adoptCachedModel`)
      {
        index: true,
        lazy: {
          Component: async () => {
            const [{ IndexPage }, { adoptCachedModel }] = await Promise.all([import("@/pages/IndexPage"), import("@/model/store")])
            adoptCachedModel(window.__BD_MODEL__)
            return IndexPage
          },
        },
      },
      {
        path: "conflicting-files",
        lazy: {
          Component: async () => (await import("@/pages/ConflictingFilesPage")).ConflictingFilesPage,
        },
      },
      {
        path: "branch-review",
        lazy: {
          Component: async () => (await import("@/pages/BranchReviewPage")).BranchReviewPage,
        },
      },
      {
        path: "pull-request-review",
        lazy: {
          Component: async () => (await import("@/pages/PullRequestReviewPage")).PullRequestReviewPage,
        },
      },
      {
        path: "commit-review",
        lazy: {
          Component: async () => (await import("@/pages/CommitReviewPage")).CommitReviewPage,
        },
      },
      {
        path: "missing-commits",
        lazy: {
          Component: async () => (await import("@/pages/MissingCommitsPage")).MissingCommitsPage,
        },
      },
      {
        path: "settings",
        lazy: {
          Component: async () => (await import("@/pages/SettingsPage")).SettingsPage,
        },
      },
      {
        path: "activity",
        lazy: {
          Component: async () => (await import("@/pages/ActivityPage")).ActivityPage,
        },
      },
    ],
  },
])
