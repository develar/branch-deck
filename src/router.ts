import { createBrowserRouter } from "react-router"
import { App } from "@/App"
import { IndexPage } from "@/pages/IndexPage"

// Sub-windows are opened by the backend as `WebviewUrl::App("/conflicting-files")`, `/missing-commits` and `/branch-review`.
// The main window may carry `?repoId=` (E2E); the browser router keeps search params untouched.
export const router = createBrowserRouter([
  {
    path: "/",
    Component: App,
    // rendered while a lazy sub-window page loads on initial navigation
    HydrateFallback: () => null,
    children: [
      { index: true, Component: IndexPage },
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
        path: "missing-commits",
        lazy: {
          Component: async () => (await import("@/pages/MissingCommitsPage")).MissingCommitsPage,
        },
      },
    ],
  },
])
