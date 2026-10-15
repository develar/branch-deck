import { memo, useMemo } from "react"
import { File, FileDiff, WorkerPoolContext } from "@pierre/diffs/react"
import type { ReactNode } from "react"
import { ConflictMarkerExtension } from "@/components/conflict/ConflictMarkerExtension"
import type { ConflictCommit, FileDiff as FileDiffData } from "@/lib/bindings"
import { getConflictAnnotations } from "@/lib/diff/conflict-annotations"
import type { ConflictLineData } from "@/lib/diff/conflict-annotations"
import { BASE_DIFF_OPTIONS, getDiffWorkerPool } from "@/lib/diff/diff-engine"
import { joinFileDiffHunks, toFileDiffMetadata } from "@/lib/diff/file-diff"
import type { ConflictDiffViewMode } from "@/stores/settings/conflictViewer"

export interface FileDiffViewProps {
  /** Must keep its identity while shown: a new object parses and highlights the diff again */
  diff: FileDiffData
  diffViewMode: ConflictDiffViewMode
  /** The contents agree with the hunks, so the unchanged lines expand without a request */
  withContents?: boolean
  /** An annotation under every added line with a conflict marker, naming its commit */
  conflictMarkers?: boolean
  conflictMarkerCommits?: Record<string, ConflictCommit>
}

/** The diff of one file without a file header (the list item above it names the file), highlighted in the worker pool */
export const FileDiffView = memo(function FileDiffView({ diff, diffViewMode, withContents = false, conflictMarkers = false, conflictMarkerCommits }: FileDiffViewProps) {
  const fileDiff = useMemo(() => toFileDiffMetadata(diff, { withContents }), [diff, withContents])
  const annotations = useMemo(
    () => (conflictMarkers ? getConflictAnnotations(joinFileDiffHunks(diff), conflictMarkerCommits) : undefined),
    [diff, conflictMarkers, conflictMarkerCommits],
  )
  const options = useMemo(() => ({
    ...BASE_DIFF_OPTIONS,
    diffStyle: diffViewMode,
    disableFileHeader: true,
    // without the file contents the unchanged lines cannot be shown, so the separators only count them
    hunkSeparators: withContents ? "line-info" : "line-info-basic",
  } as const), [diffViewMode, withContents])

  // a file that does not change between the two versions (the target side of the 3-way view)
  if (fileDiff === undefined || fileDiff.hunks.length === 0) {
    return <p className="p-4 text-sm text-muted-foreground">No changed lines</p>
  }
  return (
    <DiffWorkerPool>
      <FileDiff<ConflictLineData>
        fileDiff={fileDiff}
        options={options}
        lineAnnotations={annotations}
        renderAnnotation={renderConflictMarker}
      />
    </DiffWorkerPool>
  )
})

function renderConflictMarker(annotation: { metadata: ConflictLineData }) {
  return <ConflictMarkerExtension data={annotation.metadata} />
}

export interface FileContentViewProps {
  name: string
  contents: string
}

const FILE_OPTIONS = { ...BASE_DIFF_OPTIONS, disableFileHeader: true } as const

/** A whole file without changes (the base of the 3-way view), highlighted as a diff is */
export const FileContentView = memo(function FileContentView({ name, contents }: FileContentViewProps) {
  const file = useMemo(() => ({ name, contents }), [name, contents])
  return (
    <DiffWorkerPool>
      <File file={file} options={FILE_OPTIONS} />
    </DiffWorkerPool>
  )
})

function DiffWorkerPool({ children }: { children: ReactNode }) {
  return <WorkerPoolContext.Provider value={getDiffWorkerPool()}>{children}</WorkerPoolContext.Provider>
}
