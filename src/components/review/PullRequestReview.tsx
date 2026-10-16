import { useEffect, useMemo, useRef, useState } from "react"
import type { KeyboardEvent, RefObject } from "react"
import { ExternalLinkIcon, InfoIcon, RotateCwIcon } from "lucide-react"
import { cn } from "cn"
import { FieldGroup } from "@/components/layout/FieldGroup"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { CommitMessageBody } from "@/components/commit/CommitMessage"
import { CommitPreviewDialog } from "@/components/commit/CommitPreviewDialog"
import { CommitReview } from "@/components/commit/CommitReview"
import type { ReviewTab } from "@/components/review/ReviewDecisions"
import { ReviewDecisions, ReviewTabs } from "@/components/review/ReviewDecisions"
import type { ReviewChrome } from "@/components/review/ReviewHeader"
import { ReviewHeader, ReviewToolbar } from "@/components/review/ReviewHeader"
import { ReviewKeyHints } from "@/components/review/ReviewKeyHints"
import { ReviewRail } from "@/components/review/ReviewRail"
import { Callout } from "@/components/shared/Callout"
import { PreviewDialogBody, PreviewDialogFooter, PreviewNavigationHints } from "@/components/shared/PreviewDialogFrame"
import type { MissingCommit, ReviewCommit, ReviewRange, ReviewRequest } from "@/lib/bindings"
import type { DiffSubject } from "@/lib/commit-files"
import { diffSubjectId, parseDiffSubjectId } from "@/lib/commit-files"
import { formatShortHash } from "@/lib/hash"
import { handlePreviewNavigationKey, hasKeyModifier, isTypingTarget } from "@/lib/preview-keys"
import type { ReviewPosition } from "@/lib/review-position"
import { formatReviewPosition } from "@/lib/review-position"
import type { ReviewedCommit, ReviewIntent, ReviewRequestActionContext } from "@/lib/review-request-actions"
import { getReviewRequestActions } from "@/lib/review-request-actions"
import { loadFilesOfSubject, useFilesOfCommit } from "@/stores/commit-files"
import type { ReviewFetch } from "@/stores/review-fetch"
import { reviewFetchKey, startReviewFetch, useReviewFetch } from "@/stores/review-fetch"
import type { ReviewAction, SubmittedReview } from "@/stores/review-requests"

/**
 * Where the review of a pull request is shown, and what it does there: the dialog over the main window runs the writes
 * itself, the review window asks the main window (`PullRequestReviewPage`)
 */
export interface PullRequestReviewHost {
  readonly chrome: ReviewChrome
  /** The dialog is open (the window always is): a close forgets the state of the review */
  readonly open: boolean
  readonly repositoryPath: string
  /** Waiting the longest first, as in Your Turn */
  readonly requests: readonly ReviewRequest[]
  readonly request: ReviewRequest
  /** Its place in `requests`; -1 once it left the list (reviewed, or withdrawn) */
  readonly index: number
  /** Its place when it opened: ↓ shows the request now there once it left */
  readonly openedIndex: number
  /** No longer waiting for the user's review, and not reviewed by the user here */
  readonly withdrawn: boolean
  readonly submitted: SubmittedReview | undefined
  readonly busy: ReviewAction | undefined
  readonly safePush: ReviewRequestActionContext["safePush"]
  /** The write the row's menu asked for, offered once per open (`openToken`) */
  readonly intent: ReviewIntent | null
  readonly openToken: unknown
  readonly showRequest: (index: number) => void
  readonly close: () => void
  /** null in the window itself */
  readonly openInWindow: (() => void) | null
  readonly openOnGitHub: () => void
  readonly approve: (commitId: string) => Promise<boolean>
  readonly requestChanges: (commitId: string, body: string) => Promise<boolean>
  readonly startSafePush: (commitId: string) => Promise<boolean>
}

export interface PullRequestReviewProps {
  host: PullRequestReviewHost
  /** The body, focused when the review opens: PageUp/PageDown scroll it */
  bodyRef: RefObject<HTMLDivElement | null>
  /** Set to what Escape does in the review first (closes a form, goes back to the overview); false: nothing, close it */
  escapeRef?: RefObject<(() => boolean) | null>
  /** The review of the changes shows or not: the dialog takes the window for it */
  onReviewingChange?: (reviewing: boolean) => void
}

/**
 * The review of a pull request that waits for the user (spec/repository/review-requests.spec.md), in the dialog or in
 * its own window. A toolbar (‹ › of Your Turn, Open in Window, Open on GitHub, "⋯", the close), who asks and how long,
 * the title and the chips, then Overview / Changes (D) and the decisions: Request Changes… (R), Approve and Safe Push…
 * (M) where Patronus merges, Approve (A), the primary. The overview has the description beside the files and commits of
 * the fetched range; Changes is the review of `base..head` (the tree beside all diffs, J/K, N/P). A write is on the
 * fetched head, the commit the user sees; the keys work in both views (`getReviewRequestActions`, as the row's menu).
 */
export function PullRequestReview({ host, bodyRef, escapeRef, onReviewingChange }: PullRequestReviewProps) {
  const { chrome, request, repositoryPath, requests, index } = host
  const fetch = useReviewFetch(reviewFetchKey(repositoryPath, request))
  const range = fetch?.status === "fetched" ? fetch.range : null

  const [tab, setTab] = useState<ReviewTab>("overview")
  const [sinceReview, setSinceReview] = useState(false)
  const [initialPath, setInitialPath] = useState<string | null>(null)
  const [filePosition, setFilePosition] = useState<ReviewPosition | null>(null)
  const [commitIndex, setCommitIndex] = useState(-1)
  const [isRequestingChanges, setRequestingChanges] = useState(false)
  const [isConfirmingSafePush, setConfirmingSafePush] = useState(false)
  // the head of the first range of this request: a poll that brings new commits fetches them, and the review says so
  const [firstHead, setFirstHead] = useState<{ number: number, head: string } | null>(null)
  if (range !== null && firstHead?.number !== request.number) {
    setFirstHead({ number: request.number, head: range.head })
  }
  // a close forgets the review: the dialog stays mounted for the next open
  const [wasOpen, setWasOpen] = useState(host.open)
  if (wasOpen !== host.open) {
    setWasOpen(host.open)
    if (!host.open) {
      setTab("overview")
      setSinceReview(false)
      setCommitIndex(-1)
      setRequestingChanges(false)
      setConfirmingSafePush(false)
    }
  }
  const requestChangesRef = useRef<HTMLButtonElement>(null)
  const approveRef = useRef<HTMLButtonElement>(null)
  const safePushRef = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    if (host.open) {
      startReviewFetch(repositoryPath, request)
    }
  }, [host.open, repositoryPath, request])

  // opened from the row's menu for a write: the comment form at once, the Approve button once there is a commit
  const { intent, openToken } = host
  const [handledIntent, setHandledIntent] = useState<unknown>(null)
  if (host.open && (intent === "requestChanges" || intent === "safePush") && handledIntent !== openToken) {
    setHandledIntent(openToken)
    if (intent === "requestChanges") {
      setRequestingChanges(true)
    }
    else {
      setConfirmingSafePush(true)
    }
  }
  useEffect(() => {
    if (intent === "approve" && range !== null) {
      approveRef.current?.focus()
    }
  }, [intent, range])

  const canReviewSince = range !== null && canShowSinceReview(request, range)
  const subject: DiffSubject | null = range === null ? null : { kind: "range", base: canReviewSince && sinceReview ? request.reviewedCommit! : range.base, head: range.head }
  const reviewing = host.open && tab === "changes" && subject !== null
  useEffect(() => {
    onReviewingChange?.(reviewing)
  }, [onReviewingChange, reviewing])
  // back in the overview, the keys go to its body again; a closing dialog gives the focus back to the row
  const { open } = host
  useEffect(() => {
    if (open && !reviewing) {
      bodyRef.current?.focus()
    }
  }, [bodyRef, open, reviewing])

  // the files of the whole range for the rail, read once the range is fetched
  const rangeSubject: DiffSubject | null = range === null ? null : { kind: "range", base: range.base, head: range.head }
  const rangeId = rangeSubject === null ? "" : diffSubjectId(rangeSubject)
  const files = useFilesOfCommit(rangeId)
  // by the id: the subject object is new on every render
  useEffect(() => {
    if (rangeId !== "") {
      void loadFilesOfSubject(parseDiffSubjectId(rangeId))
    }
  }, [rangeId])

  const commits = useMemo(() => range?.commits.map(asPreviewCommit) ?? [], [range])
  const reviewedCommit = getReviewedCommit(request, fetch)

  const showTab = (next: ReviewTab, path: string | null = null) => {
    if (next === "changes" && range === null) {
      return
    }
    setInitialPath(path)
    setTab(next)
  }

  const showRequest = (next: number) => {
    if (requests[next] !== undefined) {
      setRequestingChanges(false)
      setConfirmingSafePush(false)
      host.showRequest(next)
    }
  }

  const actions = getReviewRequestActions(request, {
    reviewedCommit,
    submitted: host.submitted,
    busy: host.busy,
    safePush: host.safePush,
    onReview: () => showTab("changes"),
    onApprove: () => {
      if (reviewedCommit.id !== null) {
        void host.approve(reviewedCommit.id)
      }
    },
    onSafePush: () => setConfirmingSafePush(true),
    onRequestChanges: () => setRequestingChanges(true),
    onOpenPullRequest: host.openOnGitHub,
    onOpenInWindow: () => host.openInWindow?.(),
  }, chrome)

  /** What Escape does inside the review; false when it is the close of the review */
  const handleEscape = (): boolean => {
    if (isRequestingChanges || isConfirmingSafePush) {
      // the form closes and keeps its draft; the focus goes back to its button
      const button = isRequestingChanges ? requestChangesRef : safePushRef
      setRequestingChanges(false)
      setConfirmingSafePush(false)
      button.current?.focus()
      return true
    }
    // the commit preview above closes itself
    if (commitIndex !== -1) {
      return true
    }
    if (reviewing) {
      showTab("overview")
      return true
    }
    return false
  }
  useEffect(() => {
    if (escapeRef !== undefined) {
      escapeRef.current = handleEscape
    }
  })

  const handleKeyDown = (event: KeyboardEvent) => {
    // the window closes on an Escape nobody took (`installEscapeToClose`); the dialog asks `escapeRef` itself
    if (chrome === "window" && event.key === "Escape" && !event.defaultPrevented && !hasKeyModifier(event)) {
      if (handleEscape()) {
        event.preventDefault()
      }
      return
    }
    // a key of the commit preview above (it is in this React tree), of a text field, or with a modifier; the form of an
    // action is open: its keys are its own
    if (event.defaultPrevented || commitIndex !== -1 || hasKeyModifier(event) || isTypingTarget(event) || isRequestingChanges || isConfirmingSafePush) {
      return
    }
    // the physical keys, in any keyboard layout; the actions of the pull request work in both views
    const keyed = {
      KeyA: actions.approved ? undefined : actions.approve,
      KeyR: actions.requestChanges,
      KeyM: actions.safePush ?? undefined,
      KeyO: actions.openPullRequest,
      KeyW: actions.openInWindow ?? undefined,
    } as const
    const action = keyed[event.code as keyof typeof keyed]
    if (action !== undefined) {
      event.preventDefault()
      if (action.disabled !== true) {
        action.onSelect()
      }
      return
    }
    if (event.code === "KeyD") {
      event.preventDefault()
      showTab(reviewing ? "overview" : "changes")
      return
    }
    // the review's own keys are J/K/N/P and the arrows of its tree
    if (reviewing) {
      return
    }
    // a request that left the list (the user reviewed it) keeps its place: ↓ shows the one now there, ↑ the one before
    if (index === -1 && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
      event.preventDefault()
      showRequest(event.key === "ArrowDown" ? host.openedIndex : host.openedIndex - 1)
      return
    }
    // Space closes the dialog (Quick Look); in the window it scrolls
    if (chrome === "window" && event.key === " ") {
      return
    }
    handlePreviewNavigationKey(event, index, requests.length, showRequest, host.close)
  }

  const top = (
    <div className="flex min-w-0 flex-col">
      <ReviewToolbar
        chrome={chrome}
        request={request}
        index={index}
        count={requests.length}
        onPrevious={previousOf(host) === null ? null : () => showRequest(previousOf(host)!)}
        onNext={nextOf(host) === null ? null : () => showRequest(nextOf(host)!)}
        actions={actions}
      />
      <div className={cn("flex min-w-0 flex-col px-6", reviewing ? "gap-2 pt-3 pb-2" : "gap-4 pt-4 pb-3")}>
        <ReviewHeader chrome={chrome} request={request} submitted={host.submitted} compact={reviewing} onOpenOnGitHub={host.openOnGitHub}>
          {range !== null && firstHead?.number === request.number && firstHead.head !== range.head && (
            <p className="text-xs text-warning" data-testid="review-new-commits">
              {`#${request.number} got new commits while it was open: the review shows ${formatShortHash(range.head)} now`}
            </p>
          )}
        </ReviewHeader>
        <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
          <ReviewTabs tab={reviewing ? "changes" : "overview"} onTabChange={next => showTab(next)} fileCount={range === null ? null : (files?.files.length ?? request.changedFiles)} fetching={fetch?.status === "fetching"} />
          {reviewing && canReviewSince && <RangeToggle sinceReview={sinceReview} onChange={setSinceReview} />}
          <ReviewDecisions
            request={request}
            actions={actions}
            busy={host.busy}
            commitId={reviewedCommit.id}
            requestChangesRef={requestChangesRef}
            approveRef={approveRef}
            safePushRef={safePushRef}
            isRequestingChanges={isRequestingChanges}
            onRequestingChangesChange={setRequestingChanges}
            isConfirmingSafePush={isConfirmingSafePush}
            onConfirmingSafePushChange={setConfirmingSafePush}
            onRequestChanges={body => reviewedCommit.id === null ? Promise.resolve(false) : host.requestChanges(reviewedCommit.id, body)}
            onSafePush={() => reviewedCommit.id === null ? Promise.resolve(false) : host.startSafePush(reviewedCommit.id)}
          />
        </div>
      </div>
    </div>
  )

  const overview = (
    <div className="grid gap-x-10 gap-y-6 md:grid-cols-[minmax(0,1fr)_18rem]">
      <div className="flex min-w-0 flex-col gap-5">
        {host.withdrawn && (
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
              <Button size="xs" variant="ghost" onClick={host.openOnGitHub}>
                <ExternalLinkIcon data-icon="inline-start" />
                Open Pull Request
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
      </div>
      {fetch?.status !== "failed" && (
        <ReviewRail
          request={request}
          files={files}
          commits={range?.commits ?? null}
          progress={fetch?.status === "fetching" ? fetch.progress : null}
          onOpenFile={path => showTab("changes", path)}
          onOpenCommit={setCommitIndex}
        />
      )}
    </div>
  )

  const body = (
    <PreviewDialogBody ref={bodyRef} itemKey={String(request.number)} fill={reviewing} data-testid="review-dialog-body">
      {reviewing && subject !== null
        ? <CommitReview key={initialPath ?? ""} subject={subject} initialPath={initialPath} onPositionChange={setFilePosition} />
        : overview}
    </PreviewDialogBody>
  )

  const footer = (
    <PreviewDialogFooter
      className={chrome === "window" ? "border-t-0 px-1 py-0" : undefined}
      start={reviewing && <span className="tabular-nums" data-testid="review-file-position">{formatReviewPosition({ file: filePosition })}</span>}
    >
      {reviewing
        ? <ReviewKeyHints commits={false} escape="Overview" />
        : <PreviewNavigationHints count={requests.length} closeKey={chrome === "dialog" ? "Space" : "Esc"} />}
    </PreviewDialogFooter>
  )

  // a commit of the pull request: the commit preview and its review, ⌥↑/⌥↓ between the commits
  const commitPreview = (
    <CommitPreviewDialog
      commits={commits}
      index={commitIndex}
      onIndexChange={setCommitIndex}
      onClose={() => setCommitIndex(-1)}
      showAuthor
      showFileCount={false}
      showNewHash={false}
    />
  )

  if (chrome === "dialog") {
    // the rows of the grid of `PreviewDialogFrame`: the header, the body that alone scrolls, the footer
    return (
      <div className="contents" onKeyDown={handleKeyDown}>
        <div className="border-b border-border">{top}</div>
        {body}
        {footer}
        {commitPreview}
      </div>
    )
  }
  // the window: islands on the canvas, the key hints below them; a click anywhere keeps the focus in the page, so the
  // keys work
  return (
    <div tabIndex={-1} className="flex h-dvh flex-col gap-2 bg-canvas p-2 text-sm outline-none" data-testid="review-window" onKeyDown={handleKeyDown}>
      <Card className="shrink-0 gap-0 py-0">{top}</Card>
      <Card className="grid min-h-0 flex-1 grid-rows-[minmax(0,1fr)] gap-0 py-0">{body}</Card>
      {footer}
      {commitPreview}
    </div>
  )
}

/** The request ↑ shows; null at the top */
function previousOf(host: PullRequestReviewHost): number | null {
  const previous = (host.index === -1 ? host.openedIndex : host.index) - 1
  return previous >= 0 && previous < host.requests.length ? previous : null
}

/** The request ↓ shows; null at the bottom */
function nextOf(host: PullRequestReviewHost): number | null {
  const next = host.index === -1 ? host.openedIndex : host.index + 1
  return next < host.requests.length ? next : null
}

/**
 * The commit a review of the user is on: the head of the fetched range, which the user sees; none while it is fetched,
 * when the fetch failed, or when the commit the pull request shows could not be fetched (the review shows the source
 * branch then)
 */
function getReviewedCommit(request: ReviewRequest, fetch: ReviewFetch | undefined): ReviewedCommit {
  if (fetch?.status === "fetched") {
    return fetch.range.headState === "missing"
      ? { id: null, reason: `The commit #${request.number} shows could not be fetched: the review would be on another one` }
      : { id: fetch.range.head }
  }
  if (fetch?.status === "failed") {
    return { id: null, reason: `#${request.number} could not be fetched: a review is on the commit you saw` }
  }
  return { id: null, reason: `Wait until #${request.number} is fetched: a review is on the commit you saw` }
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

/** "All Changes · Since Your Review": a re-requested review shows what changed since the user's last one */
function RangeToggle({ sinceReview, onChange }: { sinceReview: boolean, onChange: (sinceReview: boolean) => void }) {
  return (
    <FieldGroup className="self-center" data-testid="review-range-toggle">
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

/** A commit of the pull request as the commit preview shows one, with its whole message */
function asPreviewCommit(commit: ReviewCommit): MissingCommit {
  return {
    hash: commit.id,
    subject: commit.subject,
    message: commit.message,
    authorTime: commit.authorTime,
    committerTime: commit.authorTime,
    author: commit.author,
    filesTouched: [],
    fileDiffs: [],
  }
}
