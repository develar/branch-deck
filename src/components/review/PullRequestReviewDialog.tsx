import { Fragment, useEffect, useMemo, useRef, useState } from "react"
import type { KeyboardEvent, ReactNode } from "react"
import { ExternalLinkIcon, InfoIcon, RotateCwIcon } from "lucide-react"
import { cn } from "cn"
import { FieldGroup } from "@/components/layout/FieldGroup"
import { Button } from "@/components/ui/button"
import { DialogTitle } from "@/components/ui/dialog"
import { Kbd } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { LineCounts } from "@/components/commit/CommitMetadata"
import { CommitMessageBody } from "@/components/commit/CommitMessage"
import { CommitPreviewDialog, ReviewKeyHints } from "@/components/commit/CommitPreviewDialog"
import { CommitReview } from "@/components/commit/CommitReview"
import { CommitSubjectLine } from "@/components/commit/CommitSubject"
import { Callout } from "@/components/shared/Callout"
import { PreviewDialogBody, PreviewDialogFooter, PreviewDialogFrame, PreviewDialogHeader, PreviewNavigationHints } from "@/components/shared/PreviewDialogFrame"
import { RelativeTime } from "@/components/shared/RelativeTime"
import type { MissingCommit, ReviewCommit, ReviewRange, ReviewRequest, TransferProgress } from "@/lib/bindings"
import type { DiffSubject } from "@/lib/commit-files"
import { formatShortHash } from "@/lib/hash"
import { handlePreviewNavigationKey, hasKeyModifier } from "@/lib/preview-keys"
import { endReviewFetch, reviewFetchKey, startReviewFetch, useReviewFetch } from "@/stores/review-fetch"
import { closeReviewRequest, openRequestOnGitHub, openReviewRequest, useOpenReview, useReviewRequests } from "@/stores/review-requests"

/**
 * The review of a pull request that waits for the user, opened from the Your Turn card or the toast of a new request.
 * The preview shows what GitHub said (the title, the author, `base ← head`, the size, the checks, the description) while
 * the pull request is fetched underneath; then its commits, and `D` reviews `base..head` in the full-width review of a
 * commit (the tree beside all diffs, J/K, N/P). ↑/↓ show the previous or next request, a commit opens its preview,
 * Escape goes back from the review and closes the preview, which kills the fetch. Approving is GitHub's: "Review on
 * GitHub" opens the files of the pull request there.
 *
 * Stays mounted after the first open (see `PreviewDialogFrame`).
 */
export function PullRequestReviewDialog() {
  const openReview = useOpenReview()
  // the last shown request stays while the closing animation runs
  const [shown, setShown] = useState(openReview)
  if (openReview !== null && openReview !== shown) {
    setShown(openReview)
  }
  const open = openReview !== null
  const requests = useReviewRequests()
  const live = shown === null ? undefined : requests.find(it => it.number === shown.number)
  // a request withdrawn while the dialog is open stays, with a notice
  const request = live ?? shown?.request
  const index = live === undefined ? -1 : requests.indexOf(live)
  const repositoryPath = shown?.repositoryPath ?? ""
  const fetch = useReviewFetch(request === undefined ? null : reviewFetchKey(repositoryPath, request))
  const range = fetch?.status === "fetched" ? fetch.range : null

  const [isReviewing, setReviewing] = useState(false)
  const [sinceReview, setSinceReview] = useState(false)
  const [reviewPosition, setReviewPosition] = useState("")
  const [commitIndex, setCommitIndex] = useState(-1)
  const bodyRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (open && request !== undefined) {
      startReviewFetch(repositoryPath, request)
    }
  }, [open, repositoryPath, request])

  const canReviewSince = range !== null && request !== undefined && canShowSinceReview(request, range)
  const subject: DiffSubject | null = range === null ? null : { kind: "range", base: canReviewSince && sinceReview ? request!.reviewedCommit! : range.base, head: range.head }
  const reviewing = open && isReviewing && subject !== null
  const commits = useMemo(() => range?.commits.map(asPreviewCommit) ?? [], [range])

  const close = () => {
    setReviewing(false)
    setSinceReview(false)
    setCommitIndex(-1)
    endReviewFetch()
    closeReviewRequest()
  }

  const startReview = () => {
    if (range !== null) {
      setReviewing(true)
    }
  }

  const showRequest = (next: number) => {
    const target = requests[next]
    if (target !== undefined) {
      openReviewRequest(repositoryPath, target.number)
    }
  }

  const handleKeyDown = (event: KeyboardEvent) => {
    // a key of the commit preview above (it is in this React tree), of the content, or of the review
    if (event.defaultPrevented || commitIndex !== -1 || reviewing || hasKeyModifier(event)) {
      return
    }
    if (handlePreviewNavigationKey(event, index, requests.length, showRequest, close)) {
      return
    }
    // the physical key, in any keyboard layout
    if (event.code === "KeyD") {
      event.preventDefault()
      startReview()
    }
  }

  return (
    <PreviewDialogFrame
      data-testid="review-dialog"
      open={open && request !== undefined}
      onOpenChange={(nextOpen, eventDetails) => {
        if (nextOpen) {
          return
        }
        // in the review, Escape goes back to the preview of the pull request
        if (reviewing && eventDetails.reason === "escape-key") {
          eventDetails.cancel()
          setReviewing(false)
          bodyRef.current?.focus()
          return
        }
        close()
      }}
      initialFocus={bodyRef}
      width={reviewing ? "full" : "lg"}
      onKeyDown={handleKeyDown}
    >
      {request !== undefined && (
        <>
          <PreviewDialogHeader>
            <DialogTitle className="text-base leading-6 font-medium">{request.title}</DialogTitle>
            <RequestMeta request={request} />
            {reviewing && canReviewSince && <RangeToggle sinceReview={sinceReview} onChange={setSinceReview} />}
          </PreviewDialogHeader>

          <PreviewDialogBody ref={bodyRef} itemKey={String(request.number)} fill={reviewing} data-testid="review-dialog-body">
            {reviewing && subject !== null
              ? <CommitReview subject={subject} initialPath={null} onPositionChange={setReviewPosition} />
              : (
                  <div className="flex flex-col gap-5">
                    {live === undefined && (
                      <p className="flex items-center gap-2 text-sm text-muted-foreground" data-testid="review-withdrawn">
                        <InfoIcon aria-hidden className="size-4 shrink-0" />
                        No longer waiting for your review
                      </p>
                    )}
                    {fetch?.status === "failed" && (
                      <Callout tone="destructive" title={`Cannot fetch pull request #${request.number}`} data-testid="review-fetch-failed">
                        <p>{fetch.message}</p>
                        <div className="-ml-2 flex gap-1">
                          <Button size="xs" variant="ghost" data-testid="review-fetch-retry" onClick={() => startReviewFetch(repositoryPath, request)}>
                            <RotateCwIcon data-icon="inline-start" />
                            Retry
                          </Button>
                          <Button size="xs" variant="ghost" onClick={() => openRequestOnGitHub(request)}>
                            <ExternalLinkIcon data-icon="inline-start" />
                            Open on GitHub
                          </Button>
                        </div>
                      </Callout>
                    )}
                    {range?.headState === "missing" && (
                      <Callout tone="warning" data-testid="review-head-missing" title={`${sourceName(range)} has commits the pull request does not have yet; the review shows them.`} />
                    )}
                    {request.bodyText.trim() === ""
                      ? <p className="text-sm text-muted-foreground">No description</p>
                      : <CommitMessageBody message={`${request.title}\n\n${request.bodyText}`} size="reading" />}
                    {fetch?.status !== "failed" && (
                      <section className="flex flex-col gap-2" aria-label="Commits" data-testid="review-commits">
                        <h3 className="text-xs font-medium text-muted-foreground">Commits</h3>
                        {range === null ? <FetchProgress request={request} progress={fetch?.status === "fetching" ? fetch.progress : null} /> : <CommitRows commits={range.commits} onOpen={setCommitIndex} />}
                      </section>
                    )}
                  </div>
                )}
          </PreviewDialogBody>

          <PreviewDialogFooter
            start={reviewing
              ? <span data-testid="review-dialog-position">{reviewPosition}</span>
              : index !== -1 && <span data-testid="review-dialog-position">{`${index + 1} / ${requests.length}`}</span>}
          >
            {reviewing
              ? <ReviewKeyHints count={1} />
              : (
                  <>
                    <PreviewNavigationHints count={requests.length} />
                    <Button variant="ghost" size="xs" disabled={range === null} data-testid="review-dialog-review" onClick={startReview}>
                      {fetch?.status === "fetching" ? <Spinner data-icon="inline-start" /> : <Kbd>D</Kbd>}
                      Review
                    </Button>
                  </>
                )}
            <Button size="xs" data-testid="review-on-github" onClick={() => openRequestOnGitHub(request, "/files")}>
              <ExternalLinkIcon data-icon="inline-start" />
              Review on GitHub
            </Button>
          </PreviewDialogFooter>

          {/* a commit of the pull request: the commit preview and its review, ⌥↑/⌥↓ between the commits */}
          <CommitPreviewDialog
            commits={commits}
            index={commitIndex}
            onIndexChange={setCommitIndex}
            onClose={() => setCommitIndex(-1)}
            showAuthor
            showFileCount={false}
            showNewHash={false}
          />
        </>
      )}
    </PreviewDialogFrame>
  )
}

/** The user's last review is on a commit of the pull request (so an ancestor of the head), before the head */
function canShowSinceReview(request: ReviewRequest, range: ReviewRange): boolean {
  const reviewed = request.reviewedCommit
  return request.reRequested && reviewed !== null && reviewed !== range.head && range.commits.some(commit => commit.id === reviewed)
}

/** "origin", or "GitHub" for a fork (and for a mirror without the commit) */
function sourceName(range: ReviewRange): string {
  return range.source === "gitHub" ? "GitHub" : "origin"
}

/** "#123 · alice · master ← alice/feature · 8 files · +120 −30 · checks failing" */
function RequestMeta({ request }: { request: ReviewRequest }) {
  const items: ReactNode[] = [
    `#${request.number}`,
    ...(request.author === null ? [] : [request.author]),
    <span className="min-w-0 truncate font-mono">{`${request.baseRefName} ← ${request.headRefName}`}</span>,
    <span className="tabular-nums">{request.changedFiles === 1 ? "1 file" : `${request.changedFiles} files`}</span>,
    <LineCounts additions={request.additions} deletions={request.deletions} colored />,
  ]
  if (request.checksState !== null) {
    items.push(
      <span className={cn(request.checksState === "failing" && "text-destructive", request.checksState === "passing" && "text-success")}>
        {request.checksState === "failing" ? "checks failing" : request.checksState === "pending" ? "checks running" : "checks passed"}
      </span>,
    )
  }
  return (
    <div className="flex min-w-0 flex-wrap items-center gap-x-2 text-xs text-muted-foreground" data-testid="review-dialog-meta">
      {/* the items of one request never reorder, so the index is a stable key */}
      {items.map((item, index) => (
        <Fragment key={index}>
          {index > 0 && <span aria-hidden>·</span>}
          {item}
        </Fragment>
      ))}
    </div>
  )
}

/** "All Changes · Since Your Review": a re-requested review shows what changed since the user's last one */
function RangeToggle({ sinceReview, onChange }: { sinceReview: boolean, onChange: (sinceReview: boolean) => void }) {
  return (
    <FieldGroup className="mt-2 self-start" data-testid="review-range-toggle">
      {[false, true].map(since => (
        <Button
          key={String(since)}
          size="xs"
          variant="outline"
          aria-pressed={sinceReview === since}
          className={cn(sinceReview === since && "border-primary/50 text-primary hover:text-primary")}
          onClick={() => onChange(since)}
        >
          {since ? "Since Your Review" : "All Changes"}
        </Button>
      ))}
    </FieldGroup>
  )
}

/** "Fetching #123 from origin… 42%" */
function FetchProgress({ request, progress }: { request: ReviewRequest, progress: TransferProgress | null }) {
  const source = request.isCrossRepository ? "GitHub" : "origin"
  return (
    <p className="flex items-center gap-2 text-sm text-muted-foreground" data-testid="review-fetch-progress">
      <Spinner className="size-4" />
      <span className="tabular-nums">{`Fetching #${request.number} from ${source}…${progress === null ? "" : ` ${progress.percent}%`}`}</span>
    </p>
  )
}

/** The commits of the pull request, oldest first; a click opens the commit preview */
function CommitRows({ commits, onOpen }: { commits: readonly ReviewCommit[], onOpen: (index: number) => void }) {
  if (commits.length === 0) {
    return <p className="text-sm text-muted-foreground">No commits</p>
  }
  return (
    <ol className="-mx-2 flex flex-col">
      {commits.map((commit, index) => (
        <li key={commit.id}>
          <button
            type="button"
            data-testid="review-commit"
            className="flex w-full cursor-pointer items-center gap-3 rounded-lg px-2 py-1.5 text-left text-sm outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30"
            onClick={() => onOpen(index)}
          >
            <span className="flex min-w-0 flex-1">
              <CommitSubjectLine subject={commit.subject} />
            </span>
            <span className="hidden shrink-0 text-xs text-muted-foreground sm:inline">
              {`${commit.author} · `}
              <RelativeTime time={commit.authorTime * 1000} />
            </span>
            <span className="shrink-0 font-mono text-xs text-muted-foreground">{formatShortHash(commit.id)}</span>
          </button>
        </li>
      ))}
    </ol>
  )
}

/** A commit of the pull request as the commit preview shows one: its subject is all of its message the range has */
function asPreviewCommit(commit: ReviewCommit): MissingCommit {
  return {
    hash: commit.id,
    subject: commit.subject,
    message: commit.subject,
    authorTime: commit.authorTime,
    committerTime: commit.authorTime,
    author: commit.author,
    filesTouched: [],
    fileDiffs: [],
  }
}
