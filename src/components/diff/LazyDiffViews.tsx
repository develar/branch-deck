import { lazy, Suspense } from "react"
import type { ReactNode } from "react"
import { Spinner } from "@/components/ui/spinner"
import type { CommitReviewCodeProps } from "@/components/diff/CommitReviewCode"
import type { CommitFileDiffViewProps, ConflictFileViewProps, FileDiffViewProps } from "@/components/diff/FileDiffView"

// the diff engine (Shiki, its themes and the worker pool) loads with the first diff shown
const loadDiffViews = () => import("@/components/diff/FileDiffView")
const FileDiffViewChunk = lazy(() => loadDiffViews().then(it => ({ default: it.FileDiffView })))
const ConflictFileViewChunk = lazy(() => loadDiffViews().then(it => ({ default: it.ConflictFileView })))
const CommitFileDiffViewChunk = lazy(() => loadDiffViews().then(it => ({ default: it.CommitFileDiffView })))
const CommitReviewCodeChunk = lazy(() => import("@/components/diff/CommitReviewCode").then(it => ({ default: it.CommitReviewCode })))

/** A spinner while the diff chunk loads */
function DiffSuspense({ children }: { children: ReactNode }) {
  const fallback = (
    <p className="flex items-center gap-2 p-4 text-sm text-muted-foreground">
      <Spinner className="size-4" />
      Loading diff…
    </p>
  )
  return <Suspense fallback={fallback}>{children}</Suspense>
}

/** `FileDiffView` from its own chunk */
export function LazyFileDiffView(props: FileDiffViewProps) {
  return (
    <DiffSuspense>
      <FileDiffViewChunk {...props} />
    </DiffSuspense>
  )
}

/** `CommitFileDiffView` from its own chunk */
export function LazyCommitFileDiffView(props: CommitFileDiffViewProps) {
  return (
    <DiffSuspense>
      <CommitFileDiffViewChunk {...props} />
    </DiffSuspense>
  )
}

/** `CommitReviewCode` from its own chunk; the ref reaches the view once the chunk is loaded */
export function LazyCommitReviewCode(props: CommitReviewCodeProps) {
  return (
    <DiffSuspense>
      <CommitReviewCodeChunk {...props} />
    </DiffSuspense>
  )
}

/** `ConflictFileView` from its own chunk */
export function LazyConflictFileView(props: ConflictFileViewProps) {
  return (
    <DiffSuspense>
      <ConflictFileViewChunk {...props} />
    </DiffSuspense>
  )
}
