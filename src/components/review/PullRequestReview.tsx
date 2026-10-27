import { useCallback, useEffect, useRef, useState } from "react"
import type { KeyboardEvent, RefObject } from "react"
import { cn } from "cn"
import { ExternalLinkIcon, InfoIcon, RotateCwIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { CommitMessageBody } from "@/components/commit/CommitMessage"
import { RangeReview } from "@/components/review/RangeReview"
import type { ReviewTab } from "@/components/review/ReviewDecisions"
import { ReviewDecisions, ReviewTabs } from "@/components/review/ReviewDecisions"
import { ReviewHeader, ReviewNavigation, ReviewToolbarActions } from "@/components/review/ReviewHeader"
import { ReviewKeyHints } from "@/components/review/ReviewKeyHints"
import { ReviewRail } from "@/components/review/ReviewRail"
import { ReviewSidebarToggle } from "@/components/review/ReviewSidebarToggle"
import { Callout } from "@/components/shared/Callout"
import { DiffViewToggle } from "@/components/shared/DiffViewToggle"
import { PreviewDialogBody, PreviewDialogFooter, PreviewNavigationHints } from "@/components/shared/PreviewDialogFrame"
import { WindowPage } from "@/components/shared/WindowPage"
import type { ReviewInvolvement, ReviewRange, ReviewRequest } from "@/lib/bindings"
import type { DiffSubject } from "@/lib/commit-files"
import { diffSubjectId, parseDiffSubjectId } from "@/lib/commit-files"
import { formatShortHash } from "@/lib/hash"
import { FROM_MENU } from "@/lib/menu"
import { handlePreviewNavigationKey, hasKeyModifier, isTypingTarget } from "@/lib/preview-keys"
import type { ReviewSource } from "@/lib/pull-request-review"
import { ALL_CHANGES, SINCE_REVIEW, isSingleCommit, pullRequestItems, rangePosition, shownKey } from "@/lib/range-review"
import type { ReviewPosition } from "@/lib/review-position"
import { formatReviewPosition } from "@/lib/review-position"
import type { ReviewedCommit, ReviewIntent, ReviewRequestActionContext } from "@/lib/review-request-actions"
import { getReviewRequestActions } from "@/lib/review-request-actions"
import { loadFilesOfSubject, useFilesOfCommit } from "@/stores/commit-files"
import type { ReviewFetch } from "@/stores/review-fetch"
import { reviewFetchKey, startReviewFetch, useReviewFetch } from "@/stores/review-fetch"
import type { ReviewAction, SubmittedReview } from "@/stores/review-requests"

/** What the review window of a pull request shows and does: its writes ask the main window (`PullRequestReviewPage`) */
export interface PullRequestReviewHost {
  readonly repositoryPath: string
  /** The list the window walks */
  readonly source: ReviewSource
  /** The rows of that list: waiting the longest first, as in Your Turn; in the order of Reviews */
  readonly requests: readonly ReviewRequest[]
  /** How the user takes part in the pull request of Reviews; null for a request by name */
  readonly involvement: ReviewInvolvement | null
  readonly request: ReviewRequest
  /** Its place in `requests`; -1 once it left the list (reviewed, or withdrawn) */
  readonly index: number
  /** Its place when it opened: ↓ shows the request now there once it left */
  readonly openedIndex: number
  /** No longer waiting for the user's review, and not reviewed by the user here */
  readonly withdrawn: boolean
  /** The review the user submitted here, until the main window's state says it */
  readonly submitted: SubmittedReview | undefined
  /** The actions Rust decides of the pull request (`pull:<number>`) */
  readonly actions: ReviewRequestActionContext["actions"]
  readonly busy: ReviewAction | undefined
  /** The write the row's menu asked for, offered once per open (`openToken`); "review" shows the changes */
  readonly intent: ReviewIntent | null
  readonly openToken: unknown
  readonly showRequest: (index: number) => void
  readonly close: () => void
  readonly openOnGitHub: () => void
  readonly approve: (commitId: string) => Promise<boolean>
  readonly requestChanges: (commitId: string, body: string) => Promise<boolean>
  readonly startSafePush: (commitId: string) => Promise<boolean>
}

export interface PullRequestReviewProps {
  host: PullRequestReviewHost
  /** The body, focused when the review opens: PageUp/PageDown scroll it */
  bodyRef: RefObject<HTMLDivElement | null>
}

/**
 * The review of a pull request that waits for the user (spec/repository/review-requests.spec.md), in its own window.
 * The title bar: ↑ ↓ of Your Turn, then Open on GitHub, "⋯" and the decisions, Request Changes… (R), Approve and Safe
 * Push… (M) where Patronus merges, Approve (A), the primary, last. The header island: who asks and how long, the title
 * and the chips, then Overview / Changes (D), and while the changes show the sidebar and the diff view at the end of
 * the tabs. The overview has the description beside the files and commits of the fetched range; Changes is the review
 * of `base..head` (the tree beside all diffs, J/K, N/P). A write is on the fetched head, the commit the user sees; the
 * keys work in both views (`getReviewRequestActions`, as the row's menu).
 */
export function PullRequestReview({ host, bodyRef }: PullRequestReviewProps) {
  const { request, repositoryPath, requests, index } = host
  const fetch = useReviewFetch(reviewFetchKey(repositoryPath, request))
  const range = fetch?.status === "fetched" ? fetch.range : null

  const [tab, setTab] = useState<ReviewTab>("overview")
  // the item of the commit column: all changes, since the user's review, a commit
  const [selected, setSelected] = useState(ALL_CHANGES)
  // the file on screen, where the review starts again (another item, back from the overview)
  const [lastPath, setLastPath] = useState<string | null>(null)
  // a file chosen in the rail: the review starts over at it
  const [jump, setJump] = useState(0)
  const [filePosition, setFilePosition] = useState<ReviewPosition | null>(null)
  const showPosition = useCallback((next: ReviewPosition) => {
    setFilePosition(next)
    if (next.path !== null) {
      setLastPath(next.path)
    }
  }, [])
  const [isRequestingChanges, setRequestingChanges] = useState(false)
  const [isConfirmingSafePush, setConfirmingSafePush] = useState(false)
  // the head of the first range of this request: a poll that brings new commits fetches them, and the review says so
  const [firstHead, setFirstHead] = useState<{ number: number; head: string } | null>(null)
  if (range !== null && firstHead?.number !== request.number) {
    setFirstHead({ number: request.number, head: range.head })
  }
  const requestChangesRef = useRef<HTMLButtonElement>(null)
  const approveRef = useRef<HTMLButtonElement>(null)
  const safePushRef = useRef<HTMLButtonElement>(null)

  useEffect(() => {
    startReviewFetch(repositoryPath, request)
  }, [repositoryPath, request])

  // opened from the row's menu for a write: the comment form at once, the Approve button once there is a commit
  const { intent, openToken } = host
  const [handledIntent, setHandledIntent] = useState<unknown>(null)
  if ((intent === "requestChanges" || intent === "safePush") && handledIntent !== openToken) {
    setHandledIntent(openToken)
    if (intent === "requestChanges") {
      setRequestingChanges(true)
    } else {
      setConfirmingSafePush(true)
    }
  }
  useEffect(() => {
    if (intent === "approve" && range !== null) {
      approveRef.current?.focus()
    }
  }, [intent, range])
  // D on the row: the changes, at once the pull request is fetched
  const [appliedView, setAppliedView] = useState<unknown>(null)
  if (intent === "review" && range !== null && appliedView !== openToken) {
    setAppliedView(openToken)
    setSelected(ALL_CHANGES)
    setLastPath(null)
    setJump(jump + 1)
    setTab("changes")
  }

  // the commit column: the scopes, then the commits newest first; a scope that left (the user reviewed the head) or a
  // commit a force push dropped shows all changes
  const items = range === null ? [] : pullRequestItems(range.commits, canShowSinceReview(request, host.involvement, range) ? request.reviewedCommit : null)
  const selectedKey = shownKey(items, selected)
  const subject: DiffSubject | null =
    range === null
      ? null
      : selectedKey === ALL_CHANGES
        ? { kind: "range", base: range.base, head: range.head }
        : selectedKey === SINCE_REVIEW
          ? { kind: "range", base: request.reviewedCommit!, head: range.head }
          : { kind: "commit", id: selectedKey }
  const reviewing = tab === "changes" && subject !== null
  // back in the overview, the keys go to its body again
  useEffect(() => {
    if (!reviewing) {
      bodyRef.current?.focus()
    }
  }, [bodyRef, reviewing])

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

  const reviewedCommit = getReviewedCommit(request, fetch)

  /** Shows the overview or the changes; the changes at a file of the rail (of all changes) or at a commit */
  const showTab = (next: ReviewTab, at: { path?: string; item?: string } = {}) => {
    if (next === "changes" && range === null) {
      return
    }
    if (at.item !== undefined) {
      setSelected(at.item)
    }
    if (at.path !== undefined) {
      setLastPath(at.path)
      setJump(jump + 1)
    }
    setTab(next)
  }

  const showRequest = (next: number) => {
    if (requests[next] !== undefined) {
      setRequestingChanges(false)
      setConfirmingSafePush(false)
      host.showRequest(next)
    }
  }

  const actions = getReviewRequestActions(
    request,
    {
      reviewedCommit,
      actions: host.actions,
      busy: host.busy,
      onReview: () => showTab("changes"),
      onApprove: () => {
        if (reviewedCommit.id !== null) {
          void host.approve(reviewedCommit.id)
        }
      },
      onSafePush: () => setConfirmingSafePush(true),
      onRequestChanges: () => setRequestingChanges(true),
      onOpenPullRequest: host.openOnGitHub,
    },
    "window",
  )

  /** What Escape does inside the review; false when it is the close of the window */
  const handleEscape = (): boolean => {
    if (isRequestingChanges || isConfirmingSafePush) {
      // the form closes and keeps its draft; the focus goes back to its button
      const button = isRequestingChanges ? requestChangesRef : safePushRef
      setRequestingChanges(false)
      setConfirmingSafePush(false)
      button.current?.focus()
      return true
    }
    if (reviewing) {
      showTab("overview")
      return true
    }
    // the window stays while a write it asked for runs: its answer is shown here
    return host.busy !== undefined
  }

  const handleKeyDown = (event: KeyboardEvent) => {
    // the window closes on an Escape nobody took (`installEscapeToClose`)
    if (event.key === "Escape" && !event.defaultPrevented && !hasKeyModifier(event)) {
      if (handleEscape()) {
        event.preventDefault()
      }
      return
    }
    // a key of a text field, or with a modifier (⌥↑/⌥↓ of the commit column); the form of an action is open: its keys
    // are its own
    if (event.defaultPrevented || hasKeyModifier(event) || isTypingTarget(event) || isRequestingChanges || isConfirmingSafePush) {
      return
    }
    // the physical keys, in any keyboard layout; the actions of the pull request work in both views
    const keyed = {
      KeyA: actions.approved ? undefined : actions.approve,
      KeyR: actions.requestChanges,
      KeyM: actions.safePush ?? undefined,
      KeyO: actions.openPullRequest,
    } as const
    const action = keyed[event.code as keyof typeof keyed]
    if (action !== undefined) {
      event.preventDefault()
      if (action.disabled !== true) {
        action.onSelect(FROM_MENU)
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
    // Space scrolls
    if (event.key === " ") {
      return
    }
    handlePreviewNavigationKey(event, index, requests.length, showRequest, host.close)
  }

  const toolbarProps = {
    request,
    index,
    count: requests.length,
    onPrevious: previousOf(host) === null ? null : () => showRequest(previousOf(host)!),
    onNext: nextOf(host) === null ? null : () => showRequest(nextOf(host)!),
    actions,
    source: host.source,
  }
  const top = (
    <div className="flex min-w-0 flex-col">
      <div className={cn("flex min-w-0 flex-col px-6", reviewing ? "gap-2 pt-3 pb-2" : "gap-4 pt-4 pb-3")}>
        <ReviewHeader request={request} involvement={host.involvement} submitted={host.submitted} compact={reviewing} onOpenOnGitHub={host.openOnGitHub}>
          {range !== null && firstHead?.number === request.number && firstHead.head !== range.head && (
            <p className="text-xs text-warning" data-testid="review-new-commits">
              {`#${request.number} got new commits while it was open: the review shows ${formatShortHash(range.head)} now`}
            </p>
          )}
        </ReviewHeader>
        <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
          <ReviewTabs
            tab={reviewing ? "changes" : "overview"}
            onTabChange={next => showTab(next)}
            fileCount={range === null ? null : (files?.files.length ?? request.changedFiles)}
          />
          {/* the view of the diffs, at the end of the tabs, in the header of the block it switches */}
          {reviewing && (
            <div className="ml-auto flex items-center gap-2" data-testid="review-view">
              <ReviewSidebarToggle />
              <DiffViewToggle />
            </div>
          )}
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
            {host.source === "reviews" ? "No longer in Reviews" : "No longer waiting for your review"}
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
        {request.bodyText.trim() === "" ? (
          <p className="text-sm text-muted-foreground">No description</p>
        ) : (
          <CommitMessageBody message={`${request.title}\n\n${request.bodyText}`} size="reading" />
        )}
      </div>
      {fetch?.status !== "failed" && (
        <ReviewRail
          request={request}
          files={files}
          commits={range?.commits ?? null}
          progress={fetch?.status === "fetching" ? fetch.progress : null}
          onOpenFile={path => showTab("changes", { path, item: ALL_CHANGES })}
          onOpenCommit={id => showTab("changes", { item: id })}
        />
      )}
    </div>
  )

  const body = (
    <PreviewDialogBody ref={bodyRef} itemKey={String(request.number)} fill={reviewing} data-testid="review-dialog-body">
      {reviewing && subject !== null ? (
        <RangeReview
          key={jump}
          items={items}
          selected={selectedKey}
          onSelect={setSelected}
          subject={subject}
          initialPath={lastPath}
          onPositionChange={showPosition}
          rangeNoun="pull request"
        />
      ) : (
        overview
      )}
    </PreviewDialogBody>
  )

  const footer = (
    <PreviewDialogFooter
      className="border-t-0 px-1 py-0"
      start={
        reviewing && (
          <span className="tabular-nums" data-testid="review-file-position">
            {formatReviewPosition({ file: filePosition, ...rangePosition(items, selectedKey) })}
          </span>
        )
      }
    >
      {reviewing ? <ReviewKeyHints commits={!isSingleCommit(items)} escape="Overview" /> : <PreviewNavigationHints count={requests.length} closeKey="Esc" />}
    </PreviewDialogFooter>
  )

  // the window: the toolbar with the decisions is its title bar, the islands on the canvas, the key hints below them; a click anywhere keeps
  // the focus in the page, so the keys work
  return (
    <WindowPage
      tabIndex={-1}
      className="outline-none"
      data-testid="review-window"
      onKeyDown={handleKeyDown}
      body="fill"
      title={<ReviewNavigation {...toolbarProps} />}
      titleEnd={
        <div className="flex items-center gap-1" data-testid="review-toolbar">
          <ReviewToolbarActions {...toolbarProps} />
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
            onRequestChanges={body => (reviewedCommit.id === null ? Promise.resolve(false) : host.requestChanges(reviewedCommit.id, body))}
            onSafePush={() => (reviewedCommit.id === null ? Promise.resolve(false) : host.startSafePush(reviewedCommit.id))}
          />
        </div>
      }
      footer={footer}
    >
      <Card className="shrink-0 gap-0 py-0">{top}</Card>
      <Card className="grid min-h-0 flex-1 grid-rows-[minmax(0,1fr)] gap-0 py-0">{body}</Card>
    </WindowPage>
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

/**
 * The user's last review is on a commit of the pull request (so an ancestor of the head), before the head: a request
 * asked again, or a pull request of Reviews the user reviewed
 */
function canShowSinceReview(request: ReviewRequest, involvement: ReviewInvolvement | null, range: ReviewRange): boolean {
  const reviewed = request.reviewedCommit
  const again = request.reRequested || (involvement !== null && involvement.kind !== "authored")
  return again && reviewed !== null && reviewed !== range.head && range.commits.some(commit => commit.id === reviewed)
}

/** "origin", or "GitHub" for a fork (and for a mirror without the commit) */
function sourceName(range: ReviewRange): string {
  return range.source === "gitHub" ? "GitHub" : "origin"
}
