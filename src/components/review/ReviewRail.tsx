import { useRef, useState } from "react"
import { Spinner } from "@/components/ui/spinner"
import { LineCounts } from "@/components/commit/CommitMetadata"
import { CommitRow } from "@/components/commit/CommitRow"
import { FileContextMenu } from "@/components/files/FileContextMenu"
import { FilePathLabel } from "@/components/files/FilePathLabel"
import { ShapeRows } from "@/components/review/ReviewMap"
import { LoadingLine } from "@/components/shared/LoadingLine"
import { SectionLabel } from "@/components/shared/SectionLabel"
import type { CommitFile, CommitFiles, ReviewCommit, ReviewRequest, TransferProgress } from "@/lib/bindings"
import { copyText, isCopyKey } from "@/lib/clipboard"
import { toCardCommit } from "@/lib/commit"
import type { MapView } from "@/presenters/review-map"
import { commitTitleOf } from "@/presenters/review-steps"

export interface ReviewRailProps {
  request: ReviewRequest
  /** The files of the range with their sums; undefined while they are read */
  files: CommitFiles | undefined
  /**
   * The map of the plan of all changes, once read and of a shape: its rows in place of the files, each showing its first
   * file through the navigator of the overview (`SelectReviewFileContext`); null while the plan is off or being read,
   * or of a change too small for a shape
   */
  shape: MapView | null
  /** The commits of the range; null while the pull request is fetched */
  commits: readonly ReviewCommit[] | null
  /** How far the fetch is; null before its first report */
  progress: TransferProgress | null
  onOpenFile: (path: string) => void
  /** Reviews the changes at the commit */
  onOpenCommit: (id: string) => void
  /** The title Rust gives each commit of the plan (`withCommitTitles`), as the changes name it; its subject without one */
  titles: ReadonlyMap<string, string>
}

/**
 * The side of the overview: the shape of the pull request, the rows of its map (a click reviews the changes at the first
 * file of the row), else its files (a click reviews the changes at the file), then its commits, newest first as in the
 * commit column of the changes (a click reviews the changes at the commit); while it is fetched, how far the fetch is
 */
export function ReviewRail({ request, files, shape, commits, progress, onOpenFile, onOpenCommit, titles }: ReviewRailProps) {
  if (commits === null) {
    return (
      <aside className="flex flex-col gap-2" aria-label="Commits" data-testid="review-commits">
        <FetchProgress request={request} progress={progress} />
      </aside>
    )
  }
  return (
    <aside className="flex min-w-0 flex-col gap-5">
      {shape !== null ? (
        <section className="flex min-w-0 flex-col gap-1.5" aria-label="Shape" data-testid="review-shape">
          <SectionLabel title="Shape" className="px-2">
            {shape.total.files === 1 ? "1 file " : `${shape.total.files} files `}
            <LineCounts additions={shape.total.additions} deletions={shape.total.deletions} colored={false} />
          </SectionLabel>
          <div className="px-2">
            <ShapeRows rows={shape.rows} more={shape.more} />
          </div>
        </section>
      ) : (
        <section className="flex flex-col gap-1" aria-label="Files" data-testid="review-files">
          <SectionLabel title="Files" count={files?.files.length ?? request.changedFiles} className="px-2">
            {files !== undefined && <LineCounts additions={files.additions} deletions={files.deletions} colored={false} />}
          </SectionLabel>
          {files === undefined ? <LoadingLine className="px-2">Loading files…</LoadingLine> : <FileRows files={files.files} onOpen={onOpenFile} />}
        </section>
      )}
      <section className="flex flex-col gap-1" aria-label="Commits" data-testid="review-commits">
        <SectionLabel title="Commits" count={commits.length} className="px-2" />
        <CommitRows commits={commits} onOpen={onOpenCommit} titles={titles} />
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

const RAIL_ROW_CLASS = "flex w-full cursor-pointer items-center gap-2 rounded-lg px-2 py-1 text-left text-sm outline-none hover:bg-muted bd-focus-inset focus-visible:bd-focus"

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
                  if (isCopyKey(event)) {
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

/**
 * The commits of the pull request, newest first (the range lists them oldest first), each the one commit row of a
 * sidebar (`CommitRow`): its subject, the mark of its card, which its hover, the hover of the subject and `I` on the row
 * open; a click and Enter review the commit
 */
function CommitRows({ commits, onOpen, titles }: { commits: readonly ReviewCommit[]; onOpen: (id: string) => void; titles: ReadonlyMap<string, string> }) {
  // the one open card, by the commit: by the hover of its mark or subject, or by `I`
  const [card, setCard] = useState<{ id: string; byKey: boolean } | null>(null)
  const rows = useRef(new Map<string, HTMLElement>())
  if (commits.length === 0) {
    return <p className="px-2 text-sm text-muted-foreground">No commits</p>
  }
  return (
    <ol className="flex flex-col">
      {commits.toReversed().map(commit => (
        <li key={commit.id}>
          <div
            ref={element => {
              if (element === null) {
                rows.current.delete(commit.id)
              } else {
                rows.current.set(commit.id, element)
              }
            }}
            role="button"
            tabIndex={0}
            data-testid="review-commit"
            data-commit-row=""
            className={RAIL_ROW_CLASS}
            onClick={() => onOpen(commit.id)}
            onKeyDown={event => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault()
                onOpen(commit.id)
              } else if (event.code === "KeyI" && commit.hasBody && !event.metaKey && !event.ctrlKey && !event.altKey) {
                event.preventDefault()
                setCard({ id: commit.id, byKey: true })
              }
            }}
          >
            <span className="flex min-w-0 flex-1 flex-col">
              <CommitRow
                subject={commit.subject}
                title={commitTitleOf(titles, commit.id)}
                hash={commit.id}
                subjectTestId="review-commit-subject"
                card={
                  commit.hasBody
                    ? {
                        commit: toCardCommit(commit),
                        state: {
                          open: card?.id === commit.id,
                          openedByKey: card?.id === commit.id && card.byKey,
                          onOpenChange: open =>
                            setCard(current => (open ? (current?.id === commit.id ? current : { id: commit.id, byKey: false }) : current?.id === commit.id ? null : current)),
                          focusRow: () => rows.current.get(commit.id)?.focus(),
                        },
                      }
                    : null
                }
              />
            </span>
          </div>
        </li>
      ))}
    </ol>
  )
}
