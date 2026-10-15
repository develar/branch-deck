import { lazy, Suspense } from "react"
import { Spinner } from "@/components/ui/spinner"
import type { FileContentViewProps, FileDiffViewProps } from "@/components/diff/FileDiffView"

// the diff engine (Shiki, its themes and the worker pool) loads with the first diff shown
const FileDiffViewChunk = lazy(() => import("@/components/diff/FileDiffView").then(it => ({ default: it.FileDiffView })))
const FileContentViewChunk = lazy(() => import("@/components/diff/FileDiffView").then(it => ({ default: it.FileContentView })))

function DiffFallback() {
  return (
    <p className="flex items-center gap-2 p-4 text-sm text-muted-foreground">
      <Spinner className="size-4" />
      Loading diff…
    </p>
  )
}

/** `FileDiffView` from its own chunk */
export function LazyFileDiffView(props: FileDiffViewProps) {
  return (
    <Suspense fallback={<DiffFallback />}>
      <FileDiffViewChunk {...props} />
    </Suspense>
  )
}

/** `FileContentView` from its own chunk */
export function LazyFileContentView(props: FileContentViewProps) {
  return (
    <Suspense fallback={<DiffFallback />}>
      <FileContentViewChunk {...props} />
    </Suspense>
  )
}
