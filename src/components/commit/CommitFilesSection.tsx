import { useEffect, useId, useMemo, useState } from "react"
import { Badge } from "@/components/ui/badge"
import { Spinner } from "@/components/ui/spinner"
import { LineCounts } from "@/components/commit/CommitMetadata"
import { OnDemandFileDiffList } from "@/components/conflict/OnDemandFileDiffList"
import type { OnDemandFile } from "@/components/conflict/OnDemandFileDiffList"
import { Callout } from "@/components/shared/Callout"
import { DiffViewToggle } from "@/components/shared/DiffViewToggle"
import { commands } from "@/lib/bindings"
import type { CommitFile } from "@/lib/bindings"
import { formatCommitFilePath, formatFileStatus, getFileStatusColor } from "@/lib/commit-files"
import { subtleBadgeVariant } from "@/lib/ui-types"
import { getFilesOfCommit, loadFilesOfCommit, useFilesOfCommit } from "@/stores/commit-files"
import { getSelectedProject } from "@/stores/repository"
import { useConflictViewerSettings } from "@/stores/settings/conflictViewer"

/** The files of a commit in its preview, after the message: each file's diff is loaded when it is expanded. */
export function CommitFilesSection({ commitId }: { commitId: string }) {
  const titleId = useId()
  const commitFiles = useFilesOfCommit(commitId)
  const diffViewMode = useConflictViewerSettings(state => state.conflictDiffViewMode)
  // the commit whose files could not be read; another commit tries again
  const [failedId, setFailedId] = useState<string | null>(null)

  useEffect(() => {
    void loadFilesOfCommit(commitId).then(() => {
      if (getFilesOfCommit(commitId) === undefined) {
        setFailedId(commitId)
      }
    })
  }, [commitId])

  const files = useMemo(() => commitFiles?.files.map(toDiffListFile), [commitFiles])
  const loadDiff = (file: OnDemandFile) => {
    const repositoryPath = getSelectedProject()?.path ?? ""
    const oldPath = commitFiles?.files.find(it => it.path === file.path)?.oldPath ?? null
    return commands.getCommitFileDiff({ repositoryPath, commitId, path: file.path, oldPath })
  }

  return (
    // a rule apart from the message above it
    <section aria-labelledby={titleId} data-testid="commit-files" className="flex flex-col gap-3 border-t border-border pt-5">
      <div className="flex items-center justify-between gap-2">
        {/* not a heading: the subject is the one heading of the preview */}
        <div id={titleId} className="text-sm font-medium">Files</div>
        {files !== undefined && files.length > 0 && <DiffViewToggle />}
      </div>
      {files === undefined
        ? failedId === commitId
          ? <Callout tone="destructive" title="Cannot read the files of the commit" />
          : (
              <p className="flex items-center gap-2 text-sm text-muted-foreground">
                <Spinner className="size-4" />
                Loading files…
              </p>
            )
        : files.length === 0
          ? <p className="text-sm text-muted-foreground">No changed files</p>
          // keyed by the commit: the expanded files and loaded diffs belong to one commit
          : <OnDemandFileDiffList key={commitId} files={files} keyPrefix={commitId} diffViewMode={diffViewMode} loadDiff={loadDiff} />}
    </section>
  )
}

function toDiffListFile(file: CommitFile): OnDemandFile {
  const hasLines = file.additions + file.deletions > 0
  return {
    path: file.path,
    label: formatCommitFilePath(file),
    actions: (
      <span className="flex items-center gap-2">
        {hasLines && <span className="text-xs"><LineCounts data-testid="file-line-counts" additions={file.additions} deletions={file.deletions} colored /></span>}
        <Badge variant={subtleBadgeVariant(getFileStatusColor(file.status))}>{formatFileStatus(file.status)}</Badge>
      </span>
    ),
    notice: file.binary ? "Binary file" : hasLines ? undefined : "No changed lines",
  }
}
