import { useCallback, useEffect, useId, useMemo, useState } from "react"
import { cn } from "cn"
import { ChevronRightIcon } from "lucide-react"
import { InlineCommitFileDiff } from "@/components/commit/InlineCommitFileDiff"
import { FileTree } from "@/components/files/FileTree"
import { Callout } from "@/components/shared/Callout"
import { DiffViewToggle } from "@/components/shared/DiffViewToggle"
import { LoadingLine } from "@/components/shared/LoadingLine"
import { SectionLabel } from "@/components/shared/SectionLabel"
import type { CommitFile, CommitFiles } from "@/lib/bindings"
import { buildFileTree, getInitiallyCollapsed, toggleInSet } from "@/lib/file-tree"
import { getFilesOfCommit, loadFilesOfCommit, toggleCommitFilesSection, useFilesOfCommit, useIsCommitFilesSectionOpen } from "@/stores/commit-files"
import { useConflictViewerSettings } from "@/stores/settings/diff-view-settings"

export interface CommitFilesSectionProps {
  commitId: string
  /** "Files 4" in the header of the closed section, once the list of the commit is read */
  fileCount: number | undefined
  /** Enter on a file: the review of the commit at that file */
  onReviewFile: (path: string) => void
}

/**
 * The files of a commit in its preview, after the message: closed at first, "› Files 4" (the header, F and the footer
 * button open it), open grouped by directory, a file expands to its diff. Nothing of the files is read or rendered
 * while it is closed.
 */
export function CommitFilesSection({ commitId, fileCount, onReviewFile }: CommitFilesSectionProps) {
  const titleId = useId()
  const contentId = useId()
  const isOpen = useIsCommitFilesSectionOpen()
  const commitFiles = useFilesOfCommit(commitId)
  const hasFiles = fileCount !== 0

  return (
    // a rule apart from the message above it
    <section aria-labelledby={titleId} data-testid="commit-files" className="flex flex-col gap-3 border-t border-border pt-5">
      <div className="flex min-h-7 items-center justify-between gap-2">
        {hasFiles ? (
          <button
            type="button"
            aria-expanded={isOpen}
            aria-controls={isOpen ? contentId : undefined}
            data-testid="commit-files-header"
            className="-ml-1 flex min-w-0 cursor-pointer items-center gap-1 rounded-md py-0.5 pr-1.5 pl-1 text-left outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30"
            onClick={toggleCommitFilesSection}
          >
            <ChevronRightIcon className={cn("size-3.5 shrink-0 text-muted-foreground transition-transform duration-150", isOpen && "rotate-90")} />
            {/* not a heading: the subject is the one heading of the preview */}
            <SectionLabel id={titleId} title="Files" count={fileCount} />
          </button>
        ) : (
          <SectionLabel id={titleId} title="No changed files" />
        )}
        {isOpen && commitFiles !== undefined && commitFiles.files.length > 0 && <DiffViewToggle />}
      </div>
      {isOpen && hasFiles && (
        <div id={contentId} className="flex flex-col">
          <CommitFilesContent commitId={commitId} onReviewFile={onReviewFile} />
        </div>
      )}
    </section>
  )
}

/** The files of the open section: read when it opens (a list loaded them before), then the tree */
function CommitFilesContent({ commitId, onReviewFile }: { commitId: string; onReviewFile: (path: string) => void }) {
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

  return commitFiles === undefined ? (
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
