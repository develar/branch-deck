import { Fragment, memo } from "react"
import type { MouseEvent, ReactNode } from "react"
import { cn } from "cn"
import { ChevronDownIcon, ChevronRightIcon, ExternalLinkIcon, FileDiffIcon, GitPullRequestIcon, LinkIcon } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { ContextMenu, ContextMenuContent, ContextMenuTrigger } from "@/components/ui/context-menu"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { LandingNodeHover } from "@/components/branch/LandingNodeHover"
import { LaneNodeDot } from "@/components/branch/StackLanes"
import { StatusChipDot } from "@/components/branch/StatusChip"
import { LineCounts } from "@/components/commit/CommitMetadata"
import { ContextMenuItems } from "@/components/shared/ContextMenuItems"
import { useLandingNode } from "@/hooks/useLandingNode"
import { RELATIVE_TIME_TICK_MS, useNow } from "@/hooks/useNow"
import { useOperationWords } from "@/hooks/useOperationWords"
import type { ActionState, ClaimConflict, IssueLead, ReviewRequest } from "@/lib/bindings"
import { reviewBlockerOf } from "@/lib/branch-review"
import { copyText } from "@/lib/clipboard"
import { formatFileCount } from "@/lib/commit-files"
import type { MenuItemGroups } from "@/lib/menu"
import type { ReturnedPullRequest } from "@/lib/outgoing-attention"
import { formatReturnedMeta } from "@/lib/outgoing-attention"
import { isMac } from "@/lib/platform"
import { openPullRequest } from "@/lib/pull-request"
import type { ReviewSource } from "@/lib/pull-request-review"
import { getReviewRequestActions, reviewRequestMenu } from "@/lib/review-request-actions"
import { SHOWN_LABELS, formatRequestMeta, formatStackMeta, formatStackTitle, formatWaiting, hasWaitedWorkingDay } from "@/lib/review-requests"
import { openBranchReviewWindow } from "@/lib/review-window"
import { cutList } from "@/lib/text"
import { useRowPart } from "@/model/store"
import type { OperationWords } from "@/presenters/operations"
import { claimedLabel, describeClaimConflict, describeUnproven, formatClaimConflict, formatRequestReading, formatStackRowReading, orderLabels } from "@/presenters/range-reading"
import { useBranchSync } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import type { ReviewAction } from "@/stores/review-requests"
import { openRequestOnGitHub, openReviewRequest, useRequestReading, useReviewAction, useStackReading } from "@/stores/review-requests"

/**
 * The muted meta after the title of a row, which gives way before the title: by a weight so far above the title's that
 * the title loses no fraction of a pixel while the meta has any (a fraction would cut the title with an ellipsis)
 */
const META_CLASS_NAME = "hidden min-w-0 shrink-[100000] truncate text-xs text-muted-foreground sm:inline"

export interface ReviewRequestRowProps {
  request: ReviewRequest
  repositoryPath: string
  unread: boolean
  /** The row the list's focus is on: the one tab stop of the list */
  tabStop: boolean
  /** Where the row is: the review window walks the rows of the same list */
  source?: ReviewSource
  /** The meta after the title, by how the user takes part (Reviews); the request's ("#101 · carol · requested by dave") otherwise */
  meta?: string
  /** What the meta leaves out, in its tooltip (Reviews: "you approved 6330a0a2", the reviewers by name) */
  metaHint?: string
  /**
   * The mark at the start (Reviews: the node of an own branch, the user's review, the dot of the user's move); the
   * unread dot otherwise
   */
  lead?: ReactNode
  /** Nothing of it is the user's move (Reviews): its size goes into the tooltip of the meta */
  quiet?: boolean
  /** What the age at the end says (Reviews); how long the request waits otherwise */
  age?: ReactNode
  /** An own pull request of the table: its branch row shows */
  onShowBranch?: () => void
  /** A pull request the user added by its link leaves Reviews */
  onRemoveFromReviews?: () => void
  /** A layer of a stack under its header: indented, its base the layer below it, which the header says */
  inStack?: boolean
}

/** The actions of the pull request `number` as Rust decides them */
function usePullRequestActions(number: number): readonly ActionState[] | undefined {
  return useRowPart(`pull:${number}`, "actions")
}

/**
 * The menu of a row, the right-click and the context menu key: the actions of the pull request, as the header of its
 * review has them (Review, Approve…, Approve and Safe Push… where Patronus merges, Request Changes…, Open Pull Request,
 * the copy actions; Show Branch of an own one, Remove from Reviews of a link); a write opens the review window for it
 */
function requestMenu(
  request: ReviewRequest,
  repositoryPath: string,
  source: ReviewSource,
  actions: readonly ActionState[] | undefined,
  words: OperationWords,
  busy: ReviewAction | undefined,
  extra: Pick<ReviewRequestRowProps, "onShowBranch" | "onRemoveFromReviews">,
): MenuItemGroups {
  const items = getReviewRequestActions(
    request,
    {
      reviewedCommit: { id: null, reason: "" },
      actions,
      words,
      busy,
      onReview: () => openReviewRequest(repositoryPath, request.number, undefined, source),
      onApprove: () => openReviewRequest(repositoryPath, request.number, "approve", source),
      onSafePush: () => openReviewRequest(repositoryPath, request.number, "safePush", source),
      onRequestChanges: () => openReviewRequest(repositoryPath, request.number, "requestChanges", source),
      onOpenPullRequest: () => openRequestOnGitHub(request),
      ...extra,
    },
    "rowMenu",
  )
  return reviewRequestMenu(items)
}

/**
 * A pull request that waits for the user's review, one 36 px line: the unread dot, the title (its issue key muted), its
 * labels, the muted meta ("#123 · alice · 8 files · +120 −30", the base when it is not the default branch), a dot for
 * failing or running checks, what it leaves to read and how long it waits. The meta gives way first, then the title,
 * then the labels, each cut with an ellipsis; what it leaves to read and its age never do. A click reviews it in the review window;
 * ⌘-click and O open it on GitHub, as the button on hover does. Reviews shows its pull requests in the same row, with
 * the mark, the meta and the age of how the user takes part, and the size of a row that is not the user's move in the
 * tooltip of its meta.
 */
export const ReviewRequestRow = memo(function ReviewRequestRow({
  request,
  repositoryPath,
  unread,
  tabStop,
  source = "yourTurn",
  meta,
  metaHint,
  lead,
  quiet = false,
  age,
  onShowBranch,
  onRemoveFromReviews,
  inStack = false,
}: ReviewRequestRowProps) {
  const actions = usePullRequestActions(request.number)
  const busy = useReviewAction(request.number)
  // the operations of the session only while an action of the pull request runs, waits or is busy behind one
  const words = useOperationWords(actions)
  const onClick = (event: MouseEvent) => {
    if (isMac ? event.metaKey : event.ctrlKey) {
      openRequestOnGitHub(request)
    } else if (onShowBranch === undefined) {
      openReviewRequest(repositoryPath, request.number, undefined, source)
    } else {
      onShowBranch()
    }
  }
  const files = formatFileCount(request.changedFiles)
  const size = `${files} · +${request.additions} −${request.deletions}`
  const hint = [metaHint, quiet ? size : undefined].filter(it => it !== undefined && it !== "").join(" · ")
  const metaLine = (
    // the meta gives way first
    <span className={META_CLASS_NAME} data-testid="review-request-meta">
      {meta ?? formatRequestMeta(request)}
      {!quiet && (
        <>
          {` · ${files} · `}
          <LineCounts additions={request.additions} deletions={request.deletions} colored={false} />
        </>
      )}
      {!request.baseIsDefault && !inStack && ` · → ${request.baseRefName}`}
    </span>
  )
  return (
    <ContextMenu>
      <ContextMenuTrigger
        render={
          <li
            data-testid="review-request-row"
            data-review-row={request.number}
            data-review-number={request.number}
            data-unread={unread || undefined}
            tabIndex={tabStop ? 0 : -1}
            aria-label={`Review #${request.number} ${request.title}`}
            className={cn(
              "group flex h-9 animate-in cursor-pointer items-center gap-2 px-4 text-sm duration-150 fade-in-0 outline-none bd-focus-inset hover:bg-muted focus-visible:bd-focus sm:px-6",
              inStack && "pl-10 sm:pl-12",
            )}
            onClick={onClick}
          />
        }
      >
        {/* the mark keeps its place on a row without one: the titles line up */}
        <span className="flex size-4 shrink-0 items-center justify-center">{lead ?? <UnreadDot unread={unread} />}</span>
        {/* the title is what the row is: it gives way once the meta is gone, the labels only after it */}
        <span className="min-w-0 truncate font-medium" data-testid="review-request-title">
          <PullRequestTitle title={request.title} lead={request.titleLead} />
        </span>
        <RequestLabels number={request.number} labels={request.labels ?? []} />
        <RequestTitleClaim number={request.number} />
        {hint === "" ? (
          metaLine
        ) : (
          <Tooltip>
            <TooltipTrigger render={metaLine} />
            <TooltipContent>{hint}</TooltipContent>
          </Tooltip>
        )}
        {request.checksState === "failing" && <StatusChipDot color="error" testId="review-request-checks-failing" />}
        {request.checksState === "pending" && <StatusChipDot color="warning" testId="review-request-checks-pending" />}
        <span className="ml-auto flex shrink-0 items-center gap-2">
          <Tooltip>
            <TooltipTrigger
              render={
                <Button
                  size="icon-xs"
                  variant="ghost"
                  aria-label={`Open pull request #${request.number}`}
                  data-testid="review-request-open"
                  className="text-muted-foreground opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                  tabIndex={-1}
                  onClick={event => {
                    event.stopPropagation()
                    openRequestOnGitHub(request)
                  }}
                />
              }
            >
              <ExternalLinkIcon />
            </TooltipTrigger>
            <TooltipContent>Open Pull Request</TooltipContent>
          </Tooltip>
          <RequestReadingChip number={request.number} />
          {age ?? <WaitingAge waitingSince={request.waitingSince} reRequested={request.reRequested} backToYou={request.backToYou} />}
        </span>
      </ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItems groups={requestMenu(request, repositoryPath, source, actions, words, busy, { onShowBranch, onRemoveFromReviews })} />
      </ContextMenuContent>
    </ContextMenu>
  )
})

export interface ReviewStackHeaderProps {
  /** The key of the stack ("stack:979") */
  stackKey: string
  /** Its layers the list shows, the bottom first */
  layers: readonly ReviewRequest[]
  /** All its layers, also those a filter left out */
  size: number
  open: boolean
  /** A layer is unread */
  unread: boolean
  /** The row the list's focus is on: the one tab stop of the list */
  tabStop: boolean
  onOpenChange: (open: boolean) => void
  /** Review Stack: all of its changes and each layer in one review window */
  onReview: () => void
}

/**
 * The header of a stack of requests, each into the branch of the one below (Rust's `RequestRow`), one 36 px line as a
 * request: its chevron, "Stack of 12 pull requests" and the unread dot of its layers, the muted meta ("#979–#991 · alice · +120 −30",
 * the base of its bottom when it is not the default branch), how long its longest waiting layer waits. A click, Enter
 * and → show its layers under it, ← hides them; collapsed it is one row of the card. Review Stack opens the stack in one
 * review window: the button on hover, D, and its menu (the right click, the context menu key, Shift+F10), which also
 * shows or hides the layers
 */
export const ReviewStackHeader = memo(function ReviewStackHeader({ stackKey, layers, size, open, unread, tabStop, onOpenChange, onReview }: ReviewStackHeaderProps) {
  const bottom = layers[0]!
  const oldest = layers.reduce((first, layer) => (layer.waitingSince < first.waitingSince ? layer : first))
  const additions = layers.reduce((sum, layer) => sum + layer.additions, 0)
  const deletions = layers.reduce((sum, layer) => sum + layer.deletions, 0)
  const Chevron = open ? ChevronDownIcon : ChevronRightIcon
  const menu: MenuItemGroups = [
    [{ id: "review-stack", label: "Review Stack", icon: FileDiffIcon, kbds: ["d"], onSelect: onReview }],
    [
      {
        id: "toggle-stack",
        label: open ? "Hide Pull Requests" : "Show Pull Requests",
        icon: open ? ChevronDownIcon : ChevronRightIcon,
        kbds: ["enter"],
        onSelect: () => onOpenChange(!open),
      },
    ],
  ]
  return (
    <ContextMenu>
      <ContextMenuTrigger
        render={
          <li
            data-testid="review-stack-row"
            data-review-row={stackKey}
            data-review-stack={stackKey}
            data-unread={unread || undefined}
            tabIndex={tabStop ? 0 : -1}
            aria-expanded={open}
            aria-label={`${formatStackTitle(size)} ${formatStackMeta(layers)}`}
            className="group flex h-9 animate-in cursor-pointer items-center gap-2 px-4 text-sm duration-150 fade-in-0 outline-none bd-focus-inset hover:bg-muted focus-visible:bd-focus sm:px-6"
            onClick={() => onOpenChange(!open)}
          />
        }
      >
        {/* the chevron in the place of the unread dot: the titles line up */}
        <span className="flex size-4 shrink-0 items-center justify-center">
          <Chevron aria-hidden className="size-3.5 text-muted-foreground" />
        </span>
        <span className="min-w-0 truncate font-medium" data-testid="review-stack-title">
          {formatStackTitle(size)}
        </span>
        {unread && <UnreadDot unread />}
        <span className={META_CLASS_NAME} data-testid="review-stack-meta">
          {`${formatStackMeta(layers)} · `}
          <LineCounts additions={additions} deletions={deletions} colored={false} />
          {!bottom.baseIsDefault && ` · → ${bottom.baseRefName}`}
        </span>
        <span className="ml-auto flex shrink-0 items-center gap-2">
          <Tooltip>
            <TooltipTrigger
              render={
                <Button
                  size="icon-xs"
                  variant="ghost"
                  aria-label={`Review the stack of #${bottom.number}`}
                  data-testid="review-stack-review"
                  className="text-muted-foreground opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                  tabIndex={-1}
                  onClick={event => {
                    event.stopPropagation()
                    onReview()
                  }}
                />
              }
            >
              <FileDiffIcon />
            </TooltipTrigger>
            <TooltipContent>Review Stack</TooltipContent>
          </Tooltip>
          <StackReadingChip bottom={bottom.number} />
          <WaitingAge waitingSince={oldest.waitingSince} reRequested={oldest.reRequested} backToYou={oldest.backToYou} />
        </span>
      </ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItems groups={menu} />
      </ContextMenuContent>
    </ContextMenu>
  )
})

/**
 * What the request leaves to read, read ahead (Rust's `RequestReading`), before its age: "Checking…" muted while it is
 * read, then its verdict as a chip, `success` when nothing is left to read, its hover the files not proven; nothing
 * while the session knows nothing of it (a repository that reads no review plans, a request it could not read)
 */
function RequestReadingChip({ number }: { number: number }) {
  const state = useRequestReading(number)
  if (state === undefined) {
    return null
  }
  const text = formatRequestReading(state)
  if (state.kind === "checking") {
    return <ReadingText text={text} testId="review-request-reading" />
  }
  const nothing = state.view.summary.verdict.kind === "nothingToRead"
  return <ReadingBadge text={text} nothing={nothing} hint={describeUnproven(state.view)} testId="review-request-reading" />
}

/**
 * What the layers of a stack leave to read, read ahead, before its age: "Checking…" while a layer is read, then their
 * sum ("1 of 7 with nothing to read · 14 changes to read"), `success` when no layer is left to read
 */
function StackReadingChip({ bottom }: { bottom: number }) {
  const state = useStackReading(bottom)
  if (state === undefined) {
    return null
  }
  const text = formatStackRowReading(state)
  if (state.kind === "checking") {
    return <ReadingText text={text} testId="review-stack-reading" />
  }
  return <ReadingBadge text={text} nothing={state.summary.toReadLayers === 0} hint={null} testId="review-stack-reading" />
}

/** "Checking…": quiet, in the place of the chip it becomes */
function ReadingText({ text, testId }: { text: string; testId: string }) {
  return (
    <span className="text-xs whitespace-nowrap text-muted-foreground" data-testid={testId} data-checking>
      {text}
    </span>
  )
}

/** A verdict as a chip, cut at its end in a narrow window; its hover what it leaves unproven, if anything */
function ReadingBadge({ text, nothing, hint, testId }: { text: string; nothing: boolean; hint: string | null; testId: string }) {
  const badge = (
    <Badge
      variant={nothing ? "success" : "outline"}
      className={cn("max-w-40 justify-start md:max-w-72", !nothing && "text-muted-foreground")}
      data-testid={testId}
      data-nothing={nothing || undefined}
    >
      <span className="truncate">{text}</span>
    </Badge>
  )
  if (hint === null) {
    return badge
  }
  return (
    <Tooltip>
      <TooltipTrigger render={badge} />
      <TooltipContent>{hint}</TooltipContent>
    </Tooltip>
  )
}

/** The author's word the diff of the request proves wrong (Rust's `ClaimConflict`), once read; null for none */
function useClaimConflict(number: number): ClaimConflict | null {
  const state = useRequestReading(number)
  return state?.kind === "read" ? state.view.conflict : null
}

/**
 * The word of the title the diff of the request proves wrong, a compact `warning` chip after its labels, "refactor: in
 * the title", its hover why and how much is left to read; hidden below `md`, where the title needs the room. A label
 * the diff proves wrong is its own chip in the warning tone (`RequestLabels`).
 */
function RequestTitleClaim({ number }: { number: number }) {
  const conflict = useClaimConflict(number)
  const text = conflict === null ? null : formatClaimConflict(conflict)
  if (conflict === null || text === null) {
    return null
  }
  return (
    <Tooltip>
      <TooltipTrigger render={<Badge variant="warning" className="hidden h-4 shrink-0 px-1.5 md:inline-flex" data-testid="review-request-claim" />}>{text}</TooltipTrigger>
      <TooltipContent className="max-w-80">{describeClaimConflict(conflict)}</TooltipContent>
    </Tooltip>
  )
}

/**
 * The labels of the pull request on GitHub, muted chips after the title (the first two, then "+3"), the words of its
 * author, which nothing decides by; the one its diff proves wrong first, in the warning tone, its hover why and how much
 * is left to read. They give way after the meta, each cut with an ellipsis.
 */
function RequestLabels({ number, labels }: { number: number; labels: readonly string[] }) {
  const conflict = useClaimConflict(number)
  if (labels.length === 0) {
    return null
  }
  const claimed = claimedLabel(conflict)
  const why = conflict === null ? null : describeClaimConflict(conflict)
  const { shown, more } = cutList(orderLabels(labels, claimed), SHOWN_LABELS)
  return (
    // the chips are items of the row: each gives way on its own
    <span className="hidden sm:contents" data-testid="review-request-labels">
      {shown.map(label => {
        const wrong = why !== null && label === claimed
        const chip = (
          <Badge
            variant={wrong ? "warning" : "secondary"}
            // a label gives way only once the title is gone: its weight so far below the title's that it loses no fraction
            // of a pixel (and no ellipsis) while the title has any room
            className={cn("h-4 min-w-0 shrink-[0.0001] justify-start px-1.5", !wrong && "text-muted-foreground")}
            data-testid="review-request-label"
            data-claim={wrong || undefined}
          >
            <span className="truncate">{label}</span>
          </Badge>
        )
        return (
          <Fragment key={label}>
            {wrong ? (
              <Tooltip>
                <TooltipTrigger render={chip} />
                <TooltipContent className="max-w-80">{why}</TooltipContent>
              </Tooltip>
            ) : (
              chip
            )}
          </Fragment>
        )
      })}
      {more > 0 && <span className="shrink-0 text-xs text-muted-foreground">{`+${more}`}</span>}
    </span>
  )
}

/**
 * The node of a branch of the table, as its row has it, its hover the row's: where the branch is on its way into
 * master; nothing while the view has no node of it
 */
export function BranchNodeMark({ branch }: { branch: string }) {
  const node = useLandingNode(branch)
  if (node === null) {
    return null
  }
  return (
    <LandingNodeHover node={node} subject={{ kind: "branch", name: branch }} trigger={<span className="flex" data-testid="review-entry-node" data-kind={node.kind} />}>
      <LaneNodeDot kind={node.kind} />
    </LandingNodeHover>
  )
}

/** The dot of an unread request; its place kept on a read one */
function UnreadDot({ unread }: { unread: boolean }) {
  return <span aria-hidden data-testid={unread ? "review-request-unread" : undefined} className={cn("size-1.5 rounded-full", unread && "bg-primary")} />
}

/**
 * The title with the issue key it starts with muted (Rust split it there, `titleLead`), as a branch row has its key:
 * the words are what the eye reads
 */
function PullRequestTitle({ title, lead }: { title: string; lead: IssueLead | null | undefined }) {
  if (lead == null || lead.text === "") {
    return title
  }
  return (
    <>
      <span className="font-normal text-muted-foreground">{lead.key}</span> {lead.text}
    </>
  )
}

/**
 * "waiting 2 days", "alice replied 10 minutes ago", "re-requested 3 hours ago": muted, `warning` after a working day;
 * the leaf on the shared clock, so a tick re-renders only it
 */
export function WaitingAge({
  waitingSince,
  reRequested,
  backToYou,
  testId = "review-request-age",
}: Pick<ReviewRequest, "waitingSince" | "reRequested" | "backToYou"> & { testId?: string }) {
  const now = useNow(RELATIVE_TIME_TICK_MS)
  const old = hasWaitedWorkingDay(waitingSince * 1000, now)
  return (
    <span data-testid={testId} data-old={old || undefined} className={cn("text-xs whitespace-nowrap tabular-nums", old ? "text-warning" : "text-muted-foreground")}>
      {formatWaiting({ waitingSince, reRequested, backToYou }, now)}
    </span>
  )
}

export interface ReturnedPullRequestRowProps {
  item: ReturnedPullRequest
  /** The row the list's focus is on: the one tab stop of the list */
  tabStop: boolean
}

/**
 * An own pull request a reviewer handed back, one 36 px line as a request: the title, the muted meta ("#812 · changes
 * requested by alice") and its branch. A click and Enter show the branch row, where the changes are made; D reviews the
 * branch in its window; ⌘-click and O open the pull request, as the button on hover does; the menu has Review Changes,
 * Open Pull Request and Copy Pull Request Link.
 */
export const ReturnedPullRequestRow = memo(function ReturnedPullRequestRow({ item, tabStop }: ReturnedPullRequestRowProps) {
  const { branchName, pullRequest } = item
  const showBranch = () => requestReveal({ branchName, commitIds: [] })
  // the review of its branch, as D on the branch row opens it
  const branch = useBranchSync(state => state.branches[branchName])
  const actions = useRowPart(branchName, "actions")
  const reviewBlocker = branch === undefined ? `${branchName} is not in the table` : reviewBlockerOf(branch, actions)
  const reviewBranch = () => {
    if (reviewBlocker === null) {
      void openBranchReviewWindow(branchName)
    }
  }
  const menu: MenuItemGroups = [
    [
      {
        id: "review-changes",
        label: "Review Changes",
        icon: FileDiffIcon,
        kbds: ["d"],
        disabled: reviewBlocker !== null,
        ...(reviewBlocker !== null && { description: reviewBlocker }),
        onSelect: reviewBranch,
      },
    ],
    [
      { id: "open-pull-request", label: "Open Pull Request", icon: GitPullRequestIcon, kbds: ["o"], onSelect: () => openPullRequest(pullRequest) },
      { id: "copy-pull-request-link", label: "Copy Pull Request Link", icon: LinkIcon, onSelect: () => void copyText(pullRequest.url) },
    ],
  ]
  return (
    <ContextMenu>
      <ContextMenuTrigger
        render={
          <li
            data-testid="returned-pull-request-row"
            data-returned-branch={branchName}
            tabIndex={tabStop ? 0 : -1}
            aria-label={`Show ${branchName}: #${pullRequest.number} ${pullRequest.title}, changes requested`}
            className="group flex h-9 cursor-pointer items-center gap-2 px-4 text-sm outline-none bd-focus-inset hover:bg-muted focus-visible:bd-focus sm:px-6"
            onClick={event => {
              if (isMac ? event.metaKey : event.ctrlKey) {
                openPullRequest(pullRequest)
              } else {
                showBranch()
              }
            }}
            onKeyDown={event => {
              if (event.target !== event.currentTarget || event.altKey || event.metaKey || event.ctrlKey) {
                return
              }
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault()
                showBranch()
              }
              // the physical key, in any keyboard layout: the keys of Open Pull Request and Review Changes in the row's menu
              else if (event.code === "KeyO" && !event.shiftKey) {
                event.preventDefault()
                openPullRequest(pullRequest)
              } else if (event.code === "KeyD" && !event.shiftKey) {
                event.preventDefault()
                reviewBranch()
              }
            }}
          />
        }
      >
        {/* in the place of the unread dot of a request: the titles line up */}
        <span className="flex size-4 shrink-0 items-center justify-center">
          <BranchNodeMark branch={branchName} />
        </span>
        <span className="min-w-0 truncate font-medium" data-testid="returned-pull-request-title">
          <PullRequestTitle title={pullRequest.title} lead={pullRequest.titleLead} />
        </span>
        <span className={META_CLASS_NAME} data-testid="returned-pull-request-meta">
          {formatReturnedMeta(pullRequest)}
        </span>
        <span className="ml-auto flex shrink-0 items-center gap-2">
          <Tooltip>
            <TooltipTrigger
              render={
                <Button
                  size="icon-xs"
                  variant="ghost"
                  aria-label={`Open pull request #${pullRequest.number}`}
                  className="text-muted-foreground opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                  tabIndex={-1}
                  onClick={event => {
                    event.stopPropagation()
                    openPullRequest(pullRequest)
                  }}
                />
              }
            >
              <ExternalLinkIcon />
            </TooltipTrigger>
            <TooltipContent>Open Pull Request</TooltipContent>
          </Tooltip>
          <span className="text-xs whitespace-nowrap text-muted-foreground" data-testid="returned-pull-request-branch">
            {branchName}
          </span>
        </span>
      </ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItems groups={menu} />
      </ContextMenuContent>
    </ContextMenu>
  )
})
