import { lazy, Suspense } from "react"
import type { ReactNode } from "react"
import type { CommitFileDiffViewProps, ConflictFileViewProps, FileDiffViewProps } from "@/components/diff/FileDiffView"
import { LoadingLine } from "@/components/shared/LoadingLine"

// the diff engine (Shiki, its themes and the worker pool) loads with the first diff shown
const loadDiffViews = () => import("@/components/diff/FileDiffView")
const FileDiffViewChunk = lazy(() => loadDiffViews().then(it => ({ default: it.FileDiffView })))
const ConflictFileViewChunk = lazy(() => loadDiffViews().then(it => ({ default: it.ConflictFileView })))
const CommitFileDiffViewChunk = lazy(() => loadDiffViews().then(it => ({ default: it.CommitFileDiffView })))

/** A spinner while the diff chunk loads */
function DiffSuspense({ children }: { children: ReactNode }) {
  const fallback = <LoadingLine className="p-4">Loading diff…</LoadingLine>
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

/** `ConflictFileView` from its own chunk */
export function LazyConflictFileView(props: ConflictFileViewProps) {
  return (
    <DiffSuspense>
      <ConflictFileViewChunk {...props} />
    </DiffSuspense>
  )
}
