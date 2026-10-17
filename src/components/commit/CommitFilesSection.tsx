import { useCallback, useEffect, useId, useMemo, useState } from "react"
import { InlineCommitFileDiff } from "@/components/commit/InlineCommitFileDiff"
import { FileTree } from "@/components/files/FileTree"
import { Callout } from "@/components/shared/Callout"
import { DiffViewToggle } from "@/components/shared/DiffViewToggle"
import { LoadingLine } from "@/components/shared/LoadingLine"
import type { CommitFile, CommitFiles } from "@/lib/bindings"
import { buildFileTree, getInitiallyCollapsed, toggleInSet } from "@/lib/file-tree"
import { getFilesOfCommit, loadFilesOfCommit, useFilesOfCommit } from "@/stores/commit-files"
import { useConflictViewerSettings } from "@/stores/settings/conflictViewer"

export interface CommitFilesSectionProps {
  commitId: string
  /** Enter on a file: the review of the commit at that file */
  onReviewFile: (path: string) => void
}

/** The files of a commit in its preview, after the message, grouped by directory: a file expands to its diff. */
export function CommitFilesSection({ commitId, onReviewFile }: CommitFilesSectionProps) {
  const titleId = useId()
  const commitFiles = useFilesOfCommit(commitId)
  // the commit whose files could not be read; another commit tries again
  const [failedId, setFailedId] = useState<string | null>(null)

  useEffect(() => {
    void loadFilesOfCommit(commitId).then(() => {
      if (getFilesOfCommit(commitId) === undefined) {
        setFailedId(commitId)
      }
    })
  }, [commitId])

  return (
    // a rule apart from the message above it
    <section aria-labelledby={titleId} data-testid="commit-files" className="flex flex-col gap-3 border-t border-border pt-5">
      <div className="flex items-center justify-between gap-2">
        {/* not a heading: the subject is the one heading of the preview */}
        <div id={titleId} className="text-sm font-medium">
          Files
        </div>
        {commitFiles !== undefined && commitFiles.files.length > 0 && <DiffViewToggle />}
      </div>
      {commitFiles === undefined ? (
        failedId === commitId ? (
          <Callout tone="destructive" title="Cannot read the files of the commit" />
        ) : (
          <LoadingLine key={commitId}>Loading files…</LoadingLine>
        )
      ) : commitFiles.files.length === 0 ? (
        <p className="text-sm text-muted-foreground">No changed files</p>
      ) : (
        // keyed by the commit: the collapsed directories and the expanded files belong to one commit
        <CommitFileTree key={commitId} commitFiles={commitFiles} onReviewFile={onReviewFile} />
      )}
    </section>
  )
}

function CommitFileTree({ commitFiles, onReviewFile }: { commitFiles: CommitFiles; onReviewFile: (path: string) => void }) {
  const diffViewMode = useConflictViewerSettings(state => state.conflictDiffViewMode)
  const nodes = useMemo(() => buildFileTree(commitFiles.files), [commitFiles])
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(() => getInitiallyCollapsed(nodes, commitFiles.files.length))
  const [expandedFiles, setExpandedFiles] = useState<ReadonlySet<string>>(() => new Set())
  const toggleDirectory = useCallback((key: string) => setCollapsed(previous => toggleInSet(previous, key)), [])
  const toggleFile = useCallback((key: string) => setExpandedFiles(previous => toggleInSet(previous, key)), [])
  const { commitId } = commitFiles

  return (
    <FileTree
      mode="inline"
      nodes={nodes}
      aria-label="Changed files"
      collapsed={collapsed}
      onToggleDirectory={toggleDirectory}
      expandedFiles={expandedFiles}
      onToggleFile={toggleFile}
      renderFileContent={(file: CommitFile) => <InlineCommitFileDiff commitId={commitId} file={file} diffViewMode={diffViewMode} />}
      onOpenFile={(file: CommitFile) => onReviewFile(file.path)}
    />
  )
}
