import { memo, useMemo, useState } from "react"
import type { ReactNode } from "react"
import type { FileDiffContentsLoader } from "@pierre/diffs"
import { FileDiff, UnresolvedFile, WorkerPoolContext } from "@pierre/diffs/react"
import { Button } from "@/components/ui/button"
import type { CommitFile, CommitPatch, FileDiff as FileDiffData } from "@/lib/bindings"
import { getCommitFileDiff, loadCommitFileContents } from "@/lib/diff/commit-patch"
import { BASE_DIFF_OPTIONS, getDiffWorkerPool } from "@/lib/diff/diff-engine"
import { toFileDiffMetadata } from "@/lib/diff/file-diff"
import type { ConflictDiffViewMode } from "@/stores/settings/conflictViewer"

export interface FileDiffViewProps {
  /** Must keep its identity while shown: a new object parses and highlights the diff again */
  diff: FileDiffData
  diffViewMode: ConflictDiffViewMode
  /** The contents agree with the hunks, so the unchanged lines expand without a request */
  withContents?: boolean
}

/** The diff of one file without a file header (the list item above it names the file), highlighted in the worker pool */
export const FileDiffView = memo(function FileDiffView({ diff, diffViewMode, withContents = false }: FileDiffViewProps) {
  const fileDiff = useMemo(() => toFileDiffMetadata(diff, { withContents }), [diff, withContents])
  const options = useMemo(() => getDiffOptions(diffViewMode, withContents), [diffViewMode, withContents])

  // a file that does not change between the two versions
  if (fileDiff === undefined || fileDiff.hunks.length === 0) {
    return <DiffNotice>No changed lines</DiffNotice>
  }
  return (
    <DiffWorkerPool>
      <FileDiff fileDiff={fileDiff} options={options} />
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
        <Button variant="outline" size="xs" onClick={() => setShowLarge(true)}>
          Show Diff
        </Button>
      </div>
    )
  }
  return (
    <DiffWorkerPool>
      <FileDiff fileDiff={fileDiff} options={options} />
    </DiffWorkerPool>
  )
})

export interface ConflictFileViewProps {
  name: string
  /** The file as the merge left it, with its conflict markers (`<<<<<<<`, `|||||||`, `=======`, `>>>>>>>`) */
  contents: string
}

const UNRESOLVED_OPTIONS = {
  ...BASE_DIFF_OPTIONS,
  disableFileHeader: true,
  hunkSeparators: "line-info",
  mergeConflictActionsType: "none",
  maxContextLines: 3,
} as const

/**
 * The conflicts of a file as the diff engine shows an unresolved file: each conflict as the change of the branch the
 * commit is copied onto against the change of the commit, a few lines of context around it, the rest folded. Nothing
 * is resolved here: a regroup or a coding agent resolves a sync conflict
 */
export const ConflictFileView = memo(function ConflictFileView({ name, contents }: ConflictFileViewProps) {
  const file = useMemo(() => ({ name, contents }), [name, contents])
  return (
    <DiffWorkerPool>
      <UnresolvedFile file={file} options={UNRESOLVED_OPTIONS} />
    </DiffWorkerPool>
  )
})

function DiffWorkerPool({ children }: { children: ReactNode }) {
  return <WorkerPoolContext.Provider value={getDiffWorkerPool()}>{children}</WorkerPoolContext.Provider>
}
