import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import type { KeyboardEvent, ReactNode, RefObject } from "react"
import { cn } from "cn"
import { ExternalLinkIcon, InfoIcon, RotateCwIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { CommitBodyView, NoDescription } from "@/components/commit/CommitBodyView"
import { RangeReview } from "@/components/review/RangeReview"
import type { ReviewTab } from "@/components/review/ReviewDecisions"
import { ReviewDecisions, ReviewTabs } from "@/components/review/ReviewDecisions"
import { ReviewHeader, ReviewNavigation, ReviewToolbarActions } from "@/components/review/ReviewHeader"
import { ReviewKeyHints } from "@/components/review/ReviewKeyHints"
import { ReviewOwnerChips } from "@/components/review/ReviewOwnerChips"
import { ReviewPrologue } from "@/components/review/ReviewPrologue"
import { ReviewRail } from "@/components/review/ReviewRail"
import { ReviewSidebarToggle } from "@/components/review/ReviewSidebarToggle"
import { Callout } from "@/components/shared/Callout"
import { DiffViewToggle } from "@/components/shared/DiffViewToggle"
import { PreviewDialogBody, PreviewDialogFooter, PreviewNavigationHints } from "@/components/shared/PreviewDialogFrame"
import { WindowPage } from "@/components/shared/WindowPage"
import { useFileOwners } from "@/hooks/useFileOwners"
import { useOperationWords } from "@/hooks/useOperationWords"
import { useReviewOwnerFilter } from "@/hooks/useReviewOwnerFilter"
import { useSubjectPlan } from "@/hooks/useReviewPlan"
import { useReviewPosition } from "@/hooks/useReviewPosition"
import { SelectReviewFileContext, linkNavigator } from "@/hooks/useSelectReviewFile"
import type { ReviewLink } from "@/hooks/useSelectReviewFile"
import type { ReviewerGroupView, ReviewInvolvement, ReviewRange, ReviewRequest } from "@/lib/bindings"
import type { DiffSubject } from "@/lib/commit-files"
import { diffSubjectId, parseDiffSubjectId } from "@/lib/commit-files"
import { formatShortHash } from "@/lib/hash"
import { FROM_MENU } from "@/lib/menu"
import { handlePreviewNavigationKey, hasKeyModifier, isTypingTarget } from "@/lib/preview-keys"
import type { ReviewSource } from "@/lib/pull-request-review"
import { ALL_CHANGES, SINCE_REVIEW, isSingleCommit, pullRequestItems, shownKey } from "@/lib/range-review"
import { ownerPaths, reviewFilterChips, reviewOwnerMarks } from "@/lib/review-owners"
import type { ReviewOwners } from "@/lib/review-owners"
import { planFacts } from "@/lib/review-plan-answer"
import type { ReviewedCommit, ReviewIntent, ReviewRequestActionContext } from "@/lib/review-request-actions"
import { getReviewRequestActions } from "@/lib/review-request-actions"
import { withCommitTitles } from "@/presenters/review-steps"
import { presentApprovalScope } from "@/presenters/reviews"
import { loadFilesOfSubject, useFilesOfCommit } from "@/stores/commit-files"
import type { ReviewFetch } from "@/stores/review-fetch"
import { reviewFetchKey, startReviewFetch, useReviewFetch } from "@/stores/review-fetch"
import type { ReviewAction, SubmittedReview } from "@/stores/review-requests"

/** What the review of a pull request shows and does, from the session of the review window (`PullRequestReviewWindow`) */
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
  /** The review the user submitted here, until the session says it */
  readonly submitted: SubmittedReview | undefined
  /** The actions Rust decides of the pull request (`pull:<number>`) */
  readonly actions: ReviewRequestActionContext["actions"]
  /**
   * The reviewer groups of the pull request Rust derives from Patronus's readiness of its fetched head (`pull:<number>`);
   * none while it is not known, Patronus doesn't answer, or the pull request goes into another branch
   */
  readonly reviewGroups: readonly ReviewerGroupView[]
  /** The baseline whose `OWNERSHIP` files name the owners of the files; null before a sync of the window's session */
  readonly baselineBranch: string | null
  readonly busy: ReviewAction | undefined
  /** The write the row's menu asked for, offered once per open (`openToken`); "review" shows the changes */
  readonly intent: ReviewIntent | null
  readonly openToken: unknown
  /** The last open of the review (an open of a request of the list too): each one opens on the remembered owner chip again */
  readonly openedAt: number
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
  /** Back to the review the window showed before, at the start of the title bar; null while it has none */
  back: ReactNode
}

/**
 * The review of a pull request that waits for the user (spec/repository/review-requests.spec.md), in the review window.
 * The title bar: ↑ ↓ of Your Turn, then Open on GitHub, "⋯" and the decisions, Request Changes… (R), Approve and Safe
 * Push… (M) where Patronus merges, Approve (A), the primary, last. The header island: who asks and how long, the title
 * and the chips, then Overview / Changes (D), and while the changes show the sidebar and the diff view at the end of
 * the tabs. The overview has the description beside the files and commits of the fetched range; Changes is the review
 * of `base..head` (the tree beside all diffs, J/K, N/P). A write is on the fetched head, the commit the user sees; the
 * keys work in both views (`getReviewRequestActions`, as the row's menu).
 */
export function PullRequestReview({ host, bodyRef, back }: PullRequestReviewProps) {
  const { request, repositoryPath, requests, index } = host
  const fetch = useReviewFetch(reviewFetchKey(repositoryPath, request))
  const range = fetch?.status === "fetched" ? fetch.range : null

  const [tab, setTab] = useState<ReviewTab>("overview")
  // the item of the commit column: all changes, since the user's review, a commit
  const [selected, setSelected] = useState(ALL_CHANGES)
  // a file chosen in the rail, a link of the overview: the review starts over at it
  const [jump, setJump] = useState(0)
  // the link of the overview the changes follow once read: the file at its line, the use outside, the step
  const [link, setLink] = useState<ReviewLink | null>(null)
  // the files the window showed, for what Approve counts ("You opened 3 of its 5 files"), by request and head for the
  // life of the window: ↓ to another request and back keeps them, a new head of a request counts from zero; a report is
  // of the head of the render that made its callback
  const shownHead = fetch?.status === "fetched" ? fetch.range.head : null
  const shownNumber = request.number
  const [opened, setOpened] = useState<ReadonlyMap<number, OpenedFiles>>(NO_OPENED_FILES)
  // each file the review shows counts for Approve
  const countOpened = useCallback(
    (path: string) => {
      if (shownHead !== null) {
        setOpened(previous => withOpenedFile(previous, shownNumber, shownHead, path))
      }
    },
    [shownNumber, shownHead],
  )
  // the plan of what is shown, once read or known to be none: until then the first file is no file the reader opened,
  // the overview of the plan will head the changes
  const [planSettled, setPlanSettled] = useState(true)
  const { footer: positionFooter, lastPath, setLastPath, showPosition } = useReviewPosition(null, countOpened, planSettled)
  const openedOfRequest = opened.get(shownNumber)
  const openedPaths = shownHead !== null && openedOfRequest?.head === shownHead ? openedOfRequest.paths : NOTHING_OPENED
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
    setLink(null)
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
  // the plan of what the review shows (spec/branch/review-units.spec.md): its overview and the story of its diffs, unless the
  // repository turns the plan off; the overview tab tells the plan of all changes as the same document
  const plan = useSubjectPlan(repositoryPath, reviewing ? subject : range === null ? null : { kind: "range", base: range.base, head: range.head })
  const settled = plan.disabled || plan.error !== null || (plan.plan !== null ? plan.current : !plan.loading)
  if (settled !== planSettled) {
    setPlanSettled(settled)
  }
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

  // the owner chips (spec/repository/review-requests.spec.md, "Owner chips"): the groups of the request, the owners by
  // the OWNERSHIP files of the baseline (of the base of the pull request until a sync of the window names it), of the files
  // of the shown scope; the window opens on the remembered chip, else the user's group that still needs an approval
  const subjectFiles = useFilesOfCommit(subject === null ? "" : diffSubjectId(subject))?.files
  const rangeFiles = files?.files
  const ownedPaths = useMemo(() => [...new Set([...(rangeFiles ?? []), ...(subjectFiles ?? [])].map(file => file.path))], [rangeFiles, subjectFiles])
  const fileOwners = useFileOwners(repositoryPath, host.baselineBranch ?? range?.base ?? null, ownedPaths)
  const [ownerFilter, setOwnerFilter] = useReviewOwnerFilter(repositoryPath, `${request.number}@${host.openedAt}`, {
    chips: rangeFiles === undefined ? [] : reviewFilterChips(rangeFiles, host.reviewGroups, fileOwners),
    preferYours: true,
    named: null,
  })
  const owners: ReviewOwners = { files: subjectFiles, groups: host.reviewGroups, owners: fileOwners }
  // what Approve counts for: the user's groups still to approve and their files among all changes of the head, whatever
  // chip is chosen; nothing while the head is fetched or Patronus hasn't told the groups of it
  const approvalScope =
    rangeFiles === undefined
      ? null
      : presentApprovalScope(
          host.reviewGroups.map(group => ({
            name: group.name,
            yours: group.yours,
            approved: group.approved,
            files: ownerPaths({ kind: "group", name: group.name }, rangeFiles, host.reviewGroups, fileOwners),
          })),
          openedPaths,
        )

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
      setLink(null)
      setJump(jump + 1)
    }
    setTab(next)
  }

  /** The changes of all of the pull request at the place of a link of the overview, as the link in them shows it */
  const showLink = (next: ReviewLink) => {
    if (range === null) {
      return
    }
    setSelected(ALL_CHANGES)
    // the file the diffs read first: of a changed file its own; the others start at the overview
    setLastPath(next.kind === "show" ? next.place.path : null)
    setLink(next)
    setJump(jump + 1)
    setTab("changes")
  }

  const showRequest = (next: number) => {
    if (requests[next] !== undefined) {
      setRequestingChanges(false)
      setConfirmingSafePush(false)
      host.showRequest(next)
    }
  }

  // the operations of the session only while an action of the pull request runs, waits or is busy behind one
  const words = useOperationWords(host.actions)
  const actions = getReviewRequestActions(
    request,
    {
      reviewedCommit,
      actions: host.actions,
      words,
      busy: host.busy,
      approvalScope,
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

  // what the author wrote: the lead of the document of the overview and of the changes
  const description = request.bodyText.trim() === "" ? <NoDescription /> : <CommitBodyView body={`${request.title}\n\n${request.bodyText}`} />
  // the commits of the rail named as the changes name them, by the plan of all changes the overview tells
  const railTitles = withCommitTitles(NO_TITLES, plan.plan?.steps.steps ?? [])
  // a link of the overview shows its place in the changes of all of the pull request, as the same link there does
  const overviewNavigator = linkNavigator(showLink)

  const overview = (
    // the document as wide as what it holds, the measure of reading at most (`ReadingColumn`, a body at `max-w-prose`),
    // the rail right after it, the two in the middle of the island: no gap of the window between them
    <div className="mx-auto grid w-fit max-w-full gap-x-10 gap-y-6 md:grid-cols-[auto_18rem]">
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
        {plan.plan === null && !plan.loading ? (
          description
        ) : (
          // the same document as the changes tell above their diffs: the body of the pull request as its lead, then the steps
          <SelectReviewFileContext value={overviewNavigator}>
            <ReviewPrologue state={plan} headline={false} lead={description} />
          </SelectReviewFileContext>
        )}
      </div>
      {fetch?.status !== "failed" && (
        <ReviewRail
          request={request}
          files={files}
          commits={range?.commits ?? null}
          titles={railTitles}
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
          initialLink={link}
          onPositionChange={showPosition}
          ownerMarks={reviewOwnerMarks(ownerFilter, owners)}
          onClearOwnerFilter={() => setOwnerFilter({ kind: "all" })}
          fileOwners={fileOwners}
          rangeNoun="pull request"
          filesHeader={count => <ReviewOwnerChips of={owners} filter={ownerFilter} onFilterChange={setOwnerFilter} count={count} />}
          // the overview says the plan is being read, and why it failed: only a repository that turns the plan off, or a
          // read that found none, tells none (spec/branch/review-plan.spec.md, "Reading the review plan…")
          plan={plan.disabled || (plan.plan === null && !plan.loading && plan.error === null) ? null : plan}
          reviewFacts={planFacts(plan)}
          planHeadline={false}
          planLead={description}
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
            {positionFooter(items, selectedKey)}
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
      title={
        <>
          {back}
          <ReviewNavigation {...toolbarProps} />
        </>
      }
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

const NOTHING_OPENED: ReadonlySet<string> = new Set()
const NO_TITLES: ReadonlyMap<string, string> = new Map()
const NO_OPENED_FILES: ReadonlyMap<number, OpenedFiles> = new Map()

/** The files the window showed of a request, of the head it showed them of */
interface OpenedFiles {
  readonly head: string
  readonly paths: ReadonlySet<string>
}

/** `opened` with `path` shown of `head` of the request `number`: of another head, the request's files count from zero */
function withOpenedFile(opened: ReadonlyMap<number, OpenedFiles>, number: number, head: string, path: string): ReadonlyMap<number, OpenedFiles> {
  const of = opened.get(number)
  if (of?.head === head && of.paths.has(path)) {
    return opened
  }
  const paths = of?.head === head ? new Set([...of.paths, path]) : new Set([path])
  return new Map(opened).set(number, { head, paths })
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
