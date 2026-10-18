import { cn } from "cn"
import { Spinner } from "@/components/ui/spinner"
import { LineCounts } from "@/components/commit/CommitMetadata"
import { CommitSubjectLine } from "@/components/commit/CommitSubject"
import { FileContextMenu } from "@/components/files/FileContextMenu"
import { FilePathLabel } from "@/components/files/FilePathLabel"
import { LoadingLine } from "@/components/shared/LoadingLine"
import { SectionLabel } from "@/components/shared/SectionLabel"
import type { CommitFile, CommitFiles, ReviewCommit, ReviewRequest, TransferProgress } from "@/lib/bindings"
import { copyText } from "@/lib/clipboard"
import { isCopyPathKey } from "@/lib/file-actions"
import { formatShortHash } from "@/lib/hash"
import { formatCompactTimestamp } from "@/lib/time"

export interface ReviewRailProps {
  request: ReviewRequest
  /** The files of the range with their sums; undefined while they are read */
  files: CommitFiles | undefined
  /** The commits of the range; null while the pull request is fetched */
  commits: readonly ReviewCommit[] | null
  /** How far the fetch is; null before its first report */
  progress: TransferProgress | null
  onOpenFile: (path: string) => void
  /** Reviews the changes at the commit */
  onOpenCommit: (id: string) => void
}

/**
 * The side of the overview: the files of the pull request (a click reviews the changes at the file) and its commits,
 * newest first as in the commit column of the changes (a click reviews the changes at the commit); while it is fetched,
 * how far the fetch is
 */
export function ReviewRail({ request, files, commits, progress, onOpenFile, onOpenCommit }: ReviewRailProps) {
  if (commits === null) {
    return (
      <aside className="flex flex-col gap-2" aria-label="Commits" data-testid="review-commits">
        <FetchProgress request={request} progress={progress} />
      </aside>
    )
  }
  return (
    <aside className="flex min-w-0 flex-col gap-5">
      <section className="flex flex-col gap-1" aria-label="Files" data-testid="review-files">
        <SectionLabel title="Files" count={files?.files.length ?? request.changedFiles} className="px-2">
          {files !== undefined && <LineCounts additions={files.additions} deletions={files.deletions} colored={false} />}
        </SectionLabel>
        {files === undefined ? <LoadingLine className="px-2">Loading files…</LoadingLine> : <FileRows files={files.files} onOpen={onOpenFile} />}
      </section>
      <section className="flex flex-col gap-1" aria-label="Commits" data-testid="review-commits">
        <SectionLabel title="Commits" count={commits.length} className="px-2" />
        <CommitRows commits={commits} onOpen={onOpenCommit} />
      </section>
    </aside>
  )
}

/** "Fetching #123 from origin… 42%" */
function FetchProgress({ request, progress }: { request: ReviewRequest; progress: TransferProgress | null }) {
  const source = request.isCrossRepository ? "GitHub" : "origin"
  return (
    <p className="flex items-center gap-2 px-2 text-sm text-muted-foreground" data-testid="review-fetch-progress">
      <Spinner className="size-4" />
      <span className="tabular-nums">{`Fetching #${request.number} from ${source}…${progress === null ? "" : ` ${progress.percent}%`}`}</span>
    </p>
  )
}

const RAIL_ROW_CLASS =
  "flex w-full cursor-pointer items-center gap-2 rounded-lg px-2 py-1 text-left text-sm outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30"

/**
 * The files by path, the name first and its directory muted, giving way first; a click reviews the changes at it, the
 * right click and ⌘C copy the path (`fileMenuItems`)
 */
function FileRows({ files, onOpen }: { files: readonly CommitFile[]; onOpen: (path: string) => void }) {
  if (files.length === 0) {
    return <p className="px-2 text-sm text-muted-foreground">No changed files</p>
  }
  return (
    <ul className="flex flex-col">
      {files.map(file => (
        <li key={file.path}>
          <FileContextMenu
            target={() => ({ kind: "file", path: file.path })}
            copyKey
            render={
              <button
                type="button"
                title={file.path}
                data-testid="review-file"
                className={RAIL_ROW_CLASS}
                onClick={() => onOpen(file.path)}
                onKeyDown={event => {
                  if (isCopyPathKey(event)) {
                    event.preventDefault()
                    void copyText(file.path)
                  }
                }}
              />
            }
          >
            <FilePathLabel path={file.path} oldPath={file.oldPath} status={file.status} className="flex-1" />
            {!file.binary && (
              <span className="shrink-0 text-xs text-muted-foreground">
                <LineCounts additions={file.additions} deletions={file.deletions} colored />
              </span>
            )}
          </FileContextMenu>
        </li>
      ))}
    </ul>
  )
}

/** The commits of the pull request, newest first (the range lists them oldest first); a click reviews the commit */
function CommitRows({ commits, onOpen }: { commits: readonly ReviewCommit[]; onOpen: (id: string) => void }) {
  if (commits.length === 0) {
    return <p className="px-2 text-sm text-muted-foreground">No commits</p>
  }
  return (
    <ol className="flex flex-col">
      {commits.toReversed().map(commit => (
        <li key={commit.id}>
          <button type="button" data-testid="review-commit" className={cn(RAIL_ROW_CLASS, "flex-col items-stretch gap-0.5 py-1.5")} onClick={() => onOpen(commit.id)}>
            <span className="flex min-w-0" data-testid="review-commit-subject">
              <CommitSubjectLine subject={commit.subject} />
            </span>
            <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
              <span className="min-w-0 truncate">{commit.author}</span>
              <span aria-hidden>·</span>
              <span className="tabular-nums">{formatCompactTimestamp(commit.authorTime)}</span>
              <span className="ml-auto shrink-0 font-mono">{formatShortHash(commit.id)}</span>
            </span>
          </button>
        </li>
      ))}
    </ol>
  )
}
