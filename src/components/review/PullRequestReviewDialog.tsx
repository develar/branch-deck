import { Fragment, useEffect, useMemo, useRef, useState } from "react"
import type { KeyboardEvent, ReactNode, RefObject } from "react"
import { EllipsisIcon, ExternalLinkIcon, InfoIcon, RotateCwIcon } from "lucide-react"
import { cn } from "cn"
import { FieldGroup } from "@/components/layout/FieldGroup"
import { Button } from "@/components/ui/button"
import { DialogTitle } from "@/components/ui/dialog"
import { DropdownMenu, DropdownMenuContent, DropdownMenuTrigger } from "@/components/ui/dropdown-menu"
import { Kbd } from "@/components/ui/kbd"
import { Popover, PopoverContent } from "@/components/ui/popover"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { LineCounts } from "@/components/commit/CommitMetadata"
import { CommitMessageBody } from "@/components/commit/CommitMessage"
import { CommitPreviewDialog, ReviewKeyHints } from "@/components/commit/CommitPreviewDialog"
import { CommitReview } from "@/components/commit/CommitReview"
import { CommitSubjectLine } from "@/components/commit/CommitSubject"
import { Callout } from "@/components/shared/Callout"
import { DropdownMenuItems } from "@/components/shared/DropdownMenuItems"
import { MenuItemButton, PRIMARY_KBD_CLASS_NAME } from "@/components/shared/MenuItemButton"
import { KeyHint, PreviewDialogBody, PreviewDialogFooter, PreviewDialogFrame, PreviewDialogHeader, PreviewNavigationHints } from "@/components/shared/PreviewDialogFrame"
import { RelativeTime } from "@/components/shared/RelativeTime"
import type { MissingCommit, ReviewCommit, ReviewRange, ReviewRequest, TransferProgress } from "@/lib/bindings"
import type { DiffSubject } from "@/lib/commit-files"
import { formatShortHash } from "@/lib/hash"
import { isMac } from "@/lib/platform"
import { handlePreviewNavigationKey, hasKeyModifier, isTypingTarget } from "@/lib/preview-keys"
import type { ReviewedCommit, ReviewRequestActionContext, ReviewRequestActions } from "@/lib/review-request-actions"
import { getReviewRequestActions, isApprovedCommit } from "@/lib/review-request-actions"
import { readPatronusTargetBranch, usePatronusAvailable } from "@/stores/patronus"
import type { ReviewFetch } from "@/stores/review-fetch"
import { endReviewFetch, reviewFetchKey, startReviewFetch, useReviewFetch } from "@/stores/review-fetch"
import type { SubmittedReview } from "@/stores/review-requests"
import {
  approveAndSafePush,
  closeReviewRequest,
  openRequestOnGitHub,
  openReviewRequest,
  submitReview,
  useOpenReview,
  useReviewAction,
  useReviewRequests,
  useSubmittedReview,
} from "@/stores/review-requests"

/**
 * The review of a pull request that waits for the user, opened from the Your Turn card or the toast of a new request.
 * The preview shows what GitHub said (the title, the author, `base ← head`, the size, the checks, the description) while
 * the pull request is fetched underneath; then its commits, and `D` reviews `base..head` in the full-width review of a
 * commit (the tree beside all diffs, J/K, N/P). ↑/↓ show the previous or next request, a commit opens its preview,
 * Escape goes back from the review and closes the preview, which kills the fetch. In both forms the footer has the
 * actions of the pull request (`getReviewRequestActions`, as its row's menu): Request Changes (R), Approve (A) on the
 * fetched head, Approve and Safe Push (M) where Patronus merges into its base, Open Pull Request (O) and the copy
 * actions in "⋯".
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
  const submitted = useSubmittedReview(request?.number ?? -1)
  const busy = useReviewAction(request?.number ?? -1)

  const [isReviewing, setReviewing] = useState(false)
  const [sinceReview, setSinceReview] = useState(false)
  const [reviewPosition, setReviewPosition] = useState("")
  const [commitIndex, setCommitIndex] = useState(-1)
  const [isRequestingChanges, setRequestingChanges] = useState(false)
  const [isConfirmingSafePush, setConfirmingSafePush] = useState(false)
  // the head of the first range of this request: a poll that brings new commits fetches them, and the review says so
  const [firstHead, setFirstHead] = useState<{ number: number, head: string } | null>(null)
  if (range !== null && request !== undefined && firstHead?.number !== request.number) {
    setFirstHead({ number: request.number, head: range.head })
  }
  const bodyRef = useRef<HTMLDivElement>(null)
  const requestChangesRef = useRef<HTMLButtonElement>(null)
  const approveRef = useRef<HTMLButtonElement>(null)
  const safePushRef = useRef<HTMLButtonElement>(null)
  const safePush = useSafePushState(open, request)

  useEffect(() => {
    if (open && request !== undefined) {
      startReviewFetch(repositoryPath, request)
    }
  }, [open, repositoryPath, request])

  // opened from the row's menu for a write: the comment form at once, the Approve button once there is a commit
  const intent = openReview?.intent ?? null
  const [handledIntent, setHandledIntent] = useState<object | null>(null)
  if (openReview !== null && (intent === "requestChanges" || intent === "safePush") && handledIntent !== openReview) {
    setHandledIntent(openReview)
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

  const canReviewSince = range !== null && request !== undefined && canShowSinceReview(request, range)
  const subject: DiffSubject | null = range === null ? null : { kind: "range", base: canReviewSince && sinceReview ? request!.reviewedCommit! : range.base, head: range.head }
  const reviewing = open && isReviewing && subject !== null
  const commits = useMemo(() => range?.commits.map(asPreviewCommit) ?? [], [range])
  const reviewedCommit = request === undefined ? NO_COMMIT : getReviewedCommit(request, fetch)

  const close = () => {
    setReviewing(false)
    setSinceReview(false)
    setCommitIndex(-1)
    setRequestingChanges(false)
    setConfirmingSafePush(false)
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
      setRequestingChanges(false)
      setConfirmingSafePush(false)
      openReviewRequest(repositoryPath, target.number)
    }
  }

  const actions = request === undefined
    ? null
    : getReviewRequestActions(request, {
        reviewedCommit,
        submitted,
        busy,
        safePush,
        onReview: startReview,
        onApprove: () => {
          if (reviewedCommit.id !== null) {
            void submitReview(request, "approve", reviewedCommit.id)
          }
        },
        onSafePush: () => setConfirmingSafePush(true),
        onRequestChanges: () => setRequestingChanges(true),
        onOpenPullRequest: () => openRequestOnGitHub(request),
      }, "dialog")

  const handleKeyDown = (event: KeyboardEvent) => {
    // a key of the commit preview above (it is in this React tree), of a text field, or with a modifier
    // the form of an action is open: its keys are its own
    if (event.defaultPrevented || commitIndex !== -1 || hasKeyModifier(event) || isTypingTarget(event) || actions === null || isRequestingChanges || isConfirmingSafePush) {
      return
    }
    // the review's own keys are J/K/N/P and the arrows: the actions of the pull request work in both forms
    const action = ({ KeyA: actions.approve, KeyR: actions.requestChanges, KeyM: actions.safePush ?? undefined, KeyO: actions.openPullRequest } as const)[event.code as "KeyA" | "KeyR" | "KeyM" | "KeyO"]
    if (action !== undefined) {
      event.preventDefault()
      if (action.disabled !== true) {
        action.onSelect()
      }
      return
    }
    if (reviewing) {
      return
    }
    // a request that left the list (the user reviewed it) keeps its place: ↓ shows the one now there, ↑ the one before
    if (index === -1 && shown !== null && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
      event.preventDefault()
      showRequest(event.key === "ArrowDown" ? shown.index : shown.index - 1)
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
        // Escape closes the form of an action first (the dialog may see the key before the popover), keeping a draft
        if ((isRequestingChanges || isConfirmingSafePush) && eventDetails.reason === "escape-key") {
          eventDetails.cancel()
          const button = isRequestingChanges ? requestChangesRef : safePushRef
          setRequestingChanges(false)
          setConfirmingSafePush(false)
          button.current?.focus()
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
      {request !== undefined && actions !== null && (
        <>
          <PreviewDialogHeader>
            <DialogTitle className="text-base leading-6 font-medium">{request.title}</DialogTitle>
            <RequestMeta request={request} submitted={submitted} />
            {range !== null && firstHead?.number === request.number && firstHead.head !== range.head && (
              <p className="text-xs text-warning" data-testid="review-new-commits">
                {`#${request.number} got new commits while it was open: the review shows ${formatShortHash(range.head)} now`}
              </p>
            )}
            {reviewing && canReviewSince && <RangeToggle sinceReview={sinceReview} onChange={setSinceReview} />}
          </PreviewDialogHeader>

          <PreviewDialogBody ref={bodyRef} itemKey={String(request.number)} fill={reviewing} data-testid="review-dialog-body">
            {reviewing && subject !== null
              ? <CommitReview subject={subject} initialPath={null} onPositionChange={setReviewPosition} />
              : (
                  <div className="flex flex-col gap-5">
                    {live === undefined && submitted === undefined && (
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
            <ReviewActionsFooter
              request={request}
              actions={actions}
              busy={busy}
              reviewedCommit={reviewedCommit}
              requestChangesRef={requestChangesRef}
              approveRef={approveRef}
              safePushRef={safePushRef}
              isRequestingChanges={isRequestingChanges}
              onRequestingChangesChange={setRequestingChanges}
              isConfirmingSafePush={isConfirmingSafePush}
              onConfirmingSafePushChange={setConfirmingSafePush}
              submitted={submitted}
            />
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

const NO_COMMIT: ReviewedCommit = { id: null, reason: "No pull request is shown" }

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
 * Whether Patronus merges the pull request: it does in a repository it guards, into the branch it pushes into for the
 * baseline only (a pull request into another base, e.g. of a stacked branch, would be merged there). null: no item.
 */
function useSafePushState(open: boolean, request: ReviewRequest | undefined): ReviewRequestActionContext["safePush"] {
  const patronusAvailable = usePatronusAvailable()
  // undefined while it is read
  const [target, setTarget] = useState<string | null | undefined>(undefined)
  useEffect(() => {
    if (!open || !patronusAvailable) {
      return
    }
    let current = true
    void readPatronusTargetBranch().then((branch) => {
      if (current) {
        setTarget(branch)
      }
    })
    return () => {
      current = false
    }
  }, [open, patronusAvailable])
  if (!patronusAvailable || request === undefined) {
    return null
  }
  if (target === undefined) {
    return { blocked: "Reading the branch Patronus pushes into…" }
  }
  if (target === null) {
    return { blocked: "Patronus pushes into a branch of origin: the baseline is none" }
  }
  if (request.baseRefName !== target) {
    return { blocked: `#${request.number} merges into ${request.baseRefName}: Patronus would merge it there, not into ${target}` }
  }
  return { blocked: null }
}

interface ReviewActionsFooterProps {
  request: ReviewRequest
  actions: ReviewRequestActions
  busy: ReturnType<typeof useReviewAction>
  reviewedCommit: ReviewedCommit
  requestChangesRef: RefObject<HTMLButtonElement | null>
  approveRef: RefObject<HTMLButtonElement | null>
  safePushRef: RefObject<HTMLButtonElement | null>
  isRequestingChanges: boolean
  onRequestingChangesChange: (open: boolean) => void
  isConfirmingSafePush: boolean
  onConfirmingSafePushChange: (open: boolean) => void
  submitted: SubmittedReview | undefined
}

/**
 * The actions of the pull request in the footer, the same in the preview and in the review: "⋯" (Open Pull Request,
 * the copy actions), Request Changes… with its comment form, Approve, and Approve and Safe Push…, the primary one where
 * Patronus merges (else Approve is)
 */
function ReviewActionsFooter(props: ReviewActionsFooterProps) {
  const { request, actions, busy, reviewedCommit, requestChangesRef, approveRef, safePushRef, isRequestingChanges, onRequestingChangesChange } = props
  return (
    <>
      <DropdownMenu>
        <Tooltip>
          <TooltipTrigger
            render={<DropdownMenuTrigger render={<Button variant="ghost" size="icon-xs" aria-label={`More actions for #${request.number}`} data-testid="review-dialog-more" />} />}
          >
            <EllipsisIcon />
          </TooltipTrigger>
          <TooltipContent>More actions</TooltipContent>
        </Tooltip>
        <DropdownMenuContent align="end" className="w-auto min-w-36">
          <DropdownMenuItems groups={[[actions.openPullRequest], [actions.copyLink, actions.copyBranchName]]} />
        </DropdownMenuContent>
      </DropdownMenu>
      <MenuItemButton item={actions.requestChanges} ref={requestChangesRef} pending={busy === "requestChanges"} data-testid="review-request-changes" />
      <RequestChangesPopover
        request={request}
        commitId={reviewedCommit.id}
        anchor={requestChangesRef}
        open={isRequestingChanges && reviewedCommit.id !== null}
        onOpenChange={onRequestingChangesChange}
      />
      <MenuItemButton item={actions.approve} ref={approveRef} variant={actions.safePush === null ? "default" : "ghost"} pending={busy === "approve"} data-testid="review-approve" />
      {actions.safePush !== null && (
        <>
          <MenuItemButton item={actions.safePush} ref={safePushRef} variant="default" pending={busy === "safePush"} data-testid="review-safe-push" />
          <SafePushPopover
            request={request}
            commitId={reviewedCommit.id}
            approved={isApprovedCommit(props.submitted, reviewedCommit.id)}
            pending={busy === "safePush" || busy === "approve"}
            anchor={safePushRef}
            open={props.isConfirmingSafePush && reviewedCommit.id !== null && actions.safePush.disabled !== true}
            onOpenChange={props.onConfirmingSafePushChange}
          />
        </>
      )}
    </>
  )
}

/**
 * What Approve and Safe Push does, before it does it: the approval of the commit the user saw, then the `/patronus
 * merge` comment, which merges and deletes the branch; Enter confirms. While Patronus is asked, the form says so.
 */
function SafePushPopover({ request, commitId, approved, pending, anchor, open, onOpenChange }: {
  request: ReviewRequest
  commitId: string | null
  approved: boolean
  pending: boolean
  anchor: RefObject<HTMLButtonElement | null>
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const start = async () => {
    if (commitId !== null && !pending && await approveAndSafePush(request, commitId)) {
      onOpenChange(false)
    }
  }
  const hash = commitId === null ? "" : formatShortHash(commitId)
  return (
    <Popover open={open} onOpenChange={next => !pending && onOpenChange(next)}>
      {/* opened by a key or the row's menu as well, without a trigger: the focus goes back to the button, in the dialog */}
      <PopoverContent anchor={anchor} finalFocus={anchor} side="top" align="end" className="w-96" data-testid="review-safe-push-form">
        <p className="text-sm" data-testid="review-safe-push-description">
          {`${approved ? "Comments" : `Approves ${hash}, then comments`} /patronus merge on #${request.number}: Patronus rebases ${request.headRefName} onto ${request.baseRefName}, runs the checks, merges it and deletes the branch. It reports in the pull request.`}
        </p>
        <div className="flex items-center justify-end gap-3 text-xs text-muted-foreground">
          {pending
            ? (
                <span className="mr-auto flex items-center gap-2" data-testid="review-safe-push-progress">
                  <Spinner className="size-3" />
                  {`Asking Patronus on pull request #${request.number}…`}
                </span>
              )
            : <KeyHint keys={<Kbd>Esc</Kbd>}>Cancel</KeyHint>}
          <Button size="xs" autoFocus disabled={pending || commitId === null} data-testid="review-safe-push-start" onClick={() => void start()}>
            {approved ? "Safe Push" : "Approve and Safe Push"}
            <Kbd className={PRIMARY_KBD_CLASS_NAME}>↵</Kbd>
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  )
}

/** The comment of a request for changes, required: ⌘↵ sends it, Escape closes the form and keeps the draft */
function RequestChangesPopover({ request, commitId, anchor, open, onOpenChange }: {
  request: ReviewRequest
  commitId: string | null
  anchor: RefObject<HTMLButtonElement | null>
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const [draft, setDraft] = useState("")
  const [draftOf, setDraftOf] = useState(request.number)
  if (draftOf !== request.number) {
    setDraftOf(request.number)
    setDraft("")
  }
  const [isSending, setSending] = useState(false)
  const canSend = draft.trim() !== "" && commitId !== null && !isSending

  const send = async () => {
    if (!canSend) {
      return
    }
    setSending(true)
    const sent = await submitReview(request, "requestChanges", commitId, draft)
    setSending(false)
    if (sent) {
      setDraft("")
      onOpenChange(false)
    }
  }

  return (
    <Popover open={open} onOpenChange={next => onOpenChange(next)}>
      {/* opened by a key or the row's menu as well, without a trigger: the focus goes back to the button, in the dialog */}
      <PopoverContent anchor={anchor} finalFocus={anchor} side="top" align="end" className="w-96" data-testid="review-request-changes-form">
        <Textarea
          aria-label={`What #${request.number} should change`}
          data-testid="review-request-changes-comment"
          className="min-h-28"
          placeholder="What should change"
          value={draft}
          disabled={isSending}
          autoFocus
          onChange={event => setDraft(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
              event.preventDefault()
              void send()
            }
          }}
        />
        <div className="flex items-center justify-end gap-3 text-xs text-muted-foreground">
          <KeyHint keys={<Kbd>Esc</Kbd>}>Cancel</KeyHint>
          <Button size="xs" disabled={!canSend} data-testid="review-request-changes-send" onClick={() => void send()}>
            {isSending && <Spinner data-icon="inline-start" />}
            Request Changes
            <Kbd className={PRIMARY_KBD_CLASS_NAME}>{isMac ? "⌘↵" : "Ctrl+Enter"}</Kbd>
          </Button>
        </div>
      </PopoverContent>
    </Popover>
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

/** "#123 · alice · master ← alice/feature · 8 files · +120 −30 · checks failing · approved by you" */
function RequestMeta({ request, submitted }: { request: ReviewRequest, submitted: SubmittedReview | undefined }) {
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
  if (submitted !== undefined) {
    items.push(
      <span className={cn(submitted.event === "approve" && "text-success")} data-testid="review-dialog-submitted">
        {submitted.event === "approve" ? "approved by you" : "changes requested by you"}
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
