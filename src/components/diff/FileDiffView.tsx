import { memo, useMemo, useState } from "react"
import { File, FileDiff, WorkerPoolContext } from "@pierre/diffs/react"
import type { FileDiffContentsLoader } from "@pierre/diffs"
import type { ReactNode } from "react"
import { Button } from "@/components/ui/button"
import { ConflictMarkerExtension } from "@/components/conflict/ConflictMarkerExtension"
import type { CommitFile, CommitPatch, ConflictCommit, FileDiff as FileDiffData } from "@/lib/bindings"
import { getCommitFileDiff, loadCommitFileContents } from "@/lib/diff/commit-patch"
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
  const options = useMemo(() => getDiffOptions(diffViewMode, withContents), [diffViewMode, withContents])

  // a file that does not change between the two versions (the target side of the 3-way view)
  if (fileDiff === undefined || fileDiff.hunks.length === 0) {
    return <DiffNotice>No changed lines</DiffNotice>
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

/**
 * The options of a diff without a file header (the row above it names the file). Without the file contents or a loader
 * the unchanged lines cannot be shown, so the separators only count them.
 */
function getDiffOptions(diffViewMode: ConflictDiffViewMode, expandable: boolean, loadDiffFiles?: FileDiffContentsLoader) {
  return {
    ...BASE_DIFF_OPTIONS,
    diffStyle: diffViewMode,
    disableFileHeader: true,
    hunkSeparators: expandable ? "line-info" : "line-info-basic",
    loadDiffFiles,
  } as const
}

function DiffNotice({ children }: { children: ReactNode }) {
  return <p className="p-4 text-sm text-muted-foreground">{children}</p>
}

/** Above this many changed lines the diff of a file waits for a click: it is not virtualized under its row */
export const INLINE_MAX_CHANGED_LINES = 2000

const lineCountFormat = new Intl.NumberFormat("en")

export interface CommitFileDiffViewProps {
  patch: CommitPatch
  file: CommitFile
  diffViewMode: ConflictDiffViewMode
}

/**
 * The diff of a file of a commit, from the patch of the whole commit; the unchanged lines between hunks expand with the
 * blob contents. A binary file, a file without changed lines and a file over the limits of the patch say so instead.
 */
export const CommitFileDiffView = memo(function CommitFileDiffView({ patch, file, diffViewMode }: CommitFileDiffViewProps) {
  const { fileDiff, omittedLines } = getCommitFileDiff(patch, file)
  const options = useMemo(() => getDiffOptions(diffViewMode, true, loadCommitFileContents), [diffViewMode])
  const changedLines = file.additions + file.deletions
  const [showLarge, setShowLarge] = useState(false)

  if (file.binary) {
    return <DiffNotice>Binary file</DiffNotice>
  }
  if (omittedLines !== null) {
    return <DiffNotice>{`Too large to show: ${lineCountFormat.format(omittedLines)} changed lines`}</DiffNotice>
  }
  if (fileDiff === undefined || fileDiff.hunks.length === 0) {
    return <DiffNotice>No changed lines</DiffNotice>
  }
  if (changedLines > INLINE_MAX_CHANGED_LINES && !showLarge) {
    return (
      <div className="flex items-center gap-3 p-4 text-sm text-muted-foreground">
        {`Large diff: ${lineCountFormat.format(changedLines)} changed lines`}
        <Button variant="outline" size="xs" onClick={() => setShowLarge(true)}>Show Diff</Button>
      </div>
    )
  }
  return (
    <DiffWorkerPool>
      <FileDiff fileDiff={fileDiff} options={options} />
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
