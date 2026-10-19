import { lazy, Suspense } from "react"
import type { ReactNode } from "react"
import type { DiffViewProps } from "@/components/diff/DiffView"
import { LoadingLine } from "@/components/shared/LoadingLine"

// our renderer, its highlight worker and its grammars load with the first diff of a view outside the review windows
const DiffViewChunk = lazy(() => import("@/components/diff/DiffView").then(it => ({ default: it.DiffView })))

/** A spinner while the diff chunk loads */
function DiffSuspense({ children }: { children: ReactNode }) {
  const fallback = <LoadingLine className="p-4">Loading diff…</LoadingLine>
  return <Suspense fallback={fallback}>{children}</Suspense>
}

/** `DiffView` from its own chunk */
export function LazyDiffView(props: DiffViewProps) {
  return (
    <DiffSuspense>
      <DiffViewChunk {...props} />
    </DiffSuspense>
  )
}
