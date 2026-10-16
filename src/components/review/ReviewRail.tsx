import type { ReactNode } from "react"
import { cn } from "cn"
import { Spinner } from "@/components/ui/spinner"
import { LineCounts } from "@/components/commit/CommitMetadata"
import { CommitSubjectLine } from "@/components/commit/CommitSubject"
import { RelativeTime } from "@/components/shared/RelativeTime"
import type { CommitFile, CommitFiles, ReviewCommit, ReviewRequest, TransferProgress } from "@/lib/bindings"
import { getFileStatusLetter, getFileStatusTextClass } from "@/lib/commit-files"
import { formatShortHash } from "@/lib/hash"

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
        <RailHeading title="Files" count={files?.files.length ?? request.changedFiles}>
          {files !== undefined && <LineCounts additions={files.additions} deletions={files.deletions} colored />}
        </RailHeading>
        {files === undefined
          ? <Spinner className="m-2 size-4 text-muted-foreground" />
          : <FileRows files={files.files} onOpen={onOpenFile} />}
      </section>
      <section className="flex flex-col gap-1" aria-label="Commits" data-testid="review-commits">
        <RailHeading title="Commits" count={commits.length} />
        <CommitRows commits={commits} onOpen={onOpenCommit} />
      </section>
    </aside>
  )
}

/** "Files 4 … +12 −3": the name, the count, and on the right what `children` sum up */
function RailHeading({ title, count, children }: { title: string, count: number, children?: ReactNode }) {
  return (
    <h3 className="flex items-center gap-1.5 px-2 text-xs font-medium text-muted-foreground">
      {title}
      <span className="tabular-nums">{count}</span>
      {children !== undefined && <span className="ml-auto font-normal">{children}</span>}
    </h3>
  )
}

/** "Fetching #123 from origin… 42%" */
function FetchProgress({ request, progress }: { request: ReviewRequest, progress: TransferProgress | null }) {
  const source = request.isCrossRepository ? "GitHub" : "origin"
  return (
    <p className="flex items-center gap-2 px-2 text-sm text-muted-foreground" data-testid="review-fetch-progress">
      <Spinner className="size-4" />
      <span className="tabular-nums">{`Fetching #${request.number} from ${source}…${progress === null ? "" : ` ${progress.percent}%`}`}</span>
    </p>
  )
}

const RAIL_ROW_CLASS = "flex w-full cursor-pointer items-center gap-2 rounded-lg px-2 py-1 text-left text-sm outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30"

/** The files by path, the name first and its directory muted; a click reviews the changes at it */
function FileRows({ files, onOpen }: { files: readonly CommitFile[], onOpen: (path: string) => void }) {
  if (files.length === 0) {
    return <p className="px-2 text-sm text-muted-foreground">No changed files</p>
  }
  return (
    <ul className="flex flex-col">
      {files.map((file) => {
        const slash = file.path.lastIndexOf("/")
        const name = file.path.slice(slash + 1)
        const directory = slash === -1 ? "" : file.path.slice(0, slash)
        return (
          <li key={file.path}>
            <button type="button" title={file.path} data-testid="review-file" className={RAIL_ROW_CLASS} onClick={() => onOpen(file.path)}>
              <span aria-hidden className={cn("w-3 shrink-0 text-center font-mono text-xs", getFileStatusTextClass(file.status), "no-underline")}>
                {getFileStatusLetter(file.status)}
              </span>
              <span className="flex min-w-0 flex-1 items-baseline gap-1.5">
                <span className={cn("shrink-0 truncate", getFileStatusTextClass(file.status))}>{name}</span>
                {directory !== "" && <span className="min-w-0 truncate text-xs text-muted-foreground" dir="rtl">{directory}</span>}
              </span>
              {!file.binary && (
                <span className="shrink-0 text-xs text-muted-foreground">
                  <LineCounts additions={file.additions} deletions={file.deletions} colored />
                </span>
              )}
            </button>
          </li>
        )
      })}
    </ul>
  )
}

/** The commits of the pull request, newest first (the range lists them oldest first); a click reviews the commit */
function CommitRows({ commits, onOpen }: { commits: readonly ReviewCommit[], onOpen: (id: string) => void }) {
  if (commits.length === 0) {
    return <p className="px-2 text-sm text-muted-foreground">No commits</p>
  }
  return (
    <ol className="flex flex-col">
      {commits.toReversed().map(commit => (
        <li key={commit.id}>
          <button type="button" data-testid="review-commit" className={cn(RAIL_ROW_CLASS, "flex-col items-stretch gap-0.5 py-1.5")} onClick={() => onOpen(commit.id)}>
            <span className="flex min-w-0">
              <CommitSubjectLine subject={commit.subject} />
            </span>
            <span className="flex items-center gap-1.5 text-xs text-muted-foreground">
              <span className="min-w-0 truncate">{commit.author}</span>
              <span aria-hidden>·</span>
              <RelativeTime time={commit.authorTime * 1000} />
              <span className="ml-auto shrink-0 font-mono">{formatShortHash(commit.id)}</span>
            </span>
          </button>
        </li>
      ))}
    </ol>
  )
}
