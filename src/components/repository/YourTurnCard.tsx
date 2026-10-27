import { useEffect, useRef, useState } from "react"
import type { KeyboardEvent, Ref } from "react"
import { cn } from "cn"
import { ExternalLinkIcon, GitPullRequestIcon, RefreshCwIcon, SettingsIcon, UserCheckIcon } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { Spinner } from "@/components/ui/spinner"
import { ReturnedPullRequestRow, ReviewRequestRow } from "@/components/repository/ReviewRequestRow"
import { ReviewStatusLine } from "@/components/repository/ReviewStatusLine"
import { FilteredOnGitHub } from "@/components/reviews/FilteredOnGitHub"
import { CollapsibleCard } from "@/components/shared/CollapsibleCard"
import { DropdownMenuItems } from "@/components/shared/DropdownMenuItems"
import { HelpList, HelpPopover, HelpSection } from "@/components/shared/HelpPopover"
import { MoreActionsMenu } from "@/components/shared/MoreActionsMenu"
import { useAvailabilityAction } from "@/hooks/useAvailabilityAction"
import { RELATIVE_TIME_TICK_MS, useNow } from "@/hooks/useNow"
import { useReturnedToYou } from "@/hooks/useReturnedToYou"
import { saveSettledLayout, useLayoutSettled, useSettledLayout } from "@/hooks/useSettledLayout"
import type { ReviewRequest } from "@/lib/bindings"
import { neighbourRow } from "@/lib/branch-row-keys"
import type { ReturnedPullRequest } from "@/lib/outgoing-attention"
import { formatAvailabilityHint, openPullRequestUrl } from "@/lib/pull-request"
import { formatOldestWaiting, hasWaitedWorkingDay, pickVisibleRequests, repositoryOfPullRequestUrl, reviewRequestKey, reviewRequestsSearchUrl } from "@/lib/review-requests"
import { handleReviewRowKeys } from "@/lib/review-row-keys"
import { openSettings } from "@/lib/settings-window"
import { formatRelativeTime } from "@/lib/time"
import { typedReference } from "@/presenters/reviews"
import { showMainView } from "@/stores/main-view"
import { useSelectedProjectPath } from "@/stores/repository"
import {
  openRequestOnGitHub,
  openReviewRequest,
  refreshReviewRequests,
  useIsRefreshingReviewRequests,
  useReviewRequests,
  useReviewRequestsAvailability,
  useReviewRequestsCheckedAt,
  useReviewRequestsTotalCount,
  useUnreadReviewRequests,
} from "@/stores/review-requests"
import { setReviewRequestsCollapsed, useRecentProjects } from "@/stores/settings/repositorySettings"

/**
 * The place of Your Turn above the branches: the island after the first sync (`shown`), and while the facts of the
 * remote are not in the height it had once they were in the last time (`useSettledLayout`), holding the island's header
 * as it checks, so the requests that come move no row; once they are in, its height is kept for the next start
 */
export function YourTurnSlot({ shown, reserve }: { shown: boolean; reserve: boolean }) {
  const path = useSelectedProjectPath()
  const settled = useLayoutSettled()
  const layout = useSettledLayout()
  const reserved = reserve ? (layout?.yourTurn ?? 0) : 0
  const cardRef = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (!settled || path === undefined) {
      return
    }
    // the height the settled island gives (0 without it), once its frame is laid out
    const frame = requestAnimationFrame(() => saveSettledLayout(path, { yourTurn: Math.round(cardRef.current?.getBoundingClientRect().height ?? 0) }))
    return () => cancelAnimationFrame(frame)
  }, [settled, path])
  if (settled || reserved === 0) {
    return shown ? <YourTurnCard cardRef={cardRef} /> : null
  }
  return (
    <div className="flex flex-col" style={{ minHeight: reserved }} data-testid="your-turn-slot" aria-busy="true">
      {shown ? <YourTurnCard cardRef={cardRef} checking /> : <YourTurnChecking />}
    </div>
  )
}

/** The header of Your Turn while GitHub hasn't answered yet, where the island stood the last time */
function YourTurnChecking() {
  return (
    <Card className="flex-1 gap-0 py-0" data-testid="your-turn-checking">
      <div className="flex items-center gap-2 px-4 py-2.5 sm:px-6">
        <UserCheckIcon className="size-4 shrink-0 text-muted-foreground" />
        <h2 className="shrink-0 text-sm font-semibold text-foreground">Your Turn</h2>
        <Spinner className="size-3 text-muted-foreground" />
        <span className="text-xs text-muted-foreground">Checking GitHub…</span>
      </div>
    </Card>
  )
}

/**
 * "Your Turn": the island of what waits for the user in a review, above the branches: the pull requests of others that
 * ask for the user's review (requested by name), waiting the longest first, then the user's own pull requests a reviewer
 * handed back with changes requested. Hidden while neither waits, off GitHub, without a GitHub token or with a rejected
 * one; a failed refresh keeps the rows. Open by default, collapsed per repository.
 */
export function YourTurnCard({ checking = false, cardRef }: { checking?: boolean; cardRef?: Ref<HTMLDivElement> }) {
  const requests = useReviewRequests()
  const returned = useReturnedToYou()
  const repositoryPath = useSelectedProjectPath()
  const collapsed = useRecentProjects(projects => projects[0]?.reviewRequestsCollapsed === true)
  // the first entrance slides in, as the Landed island; later the rows only fade
  const [entered] = useState(() => requests.length > 0 || returned.length > 0)
  if ((requests.length === 0 && returned.length === 0) || repositoryPath === undefined) {
    // its place is held while GitHub checks: the header says so
    return checking ? <YourTurnChecking /> : null
  }
  return (
    <CollapsibleCard
      ref={cardRef}
      data-testid="your-turn-card"
      open={!collapsed}
      onOpenChange={open => setReviewRequestsCollapsed(repositoryPath, !open)}
      className={cn(!entered && "animate-in duration-300 fade-in-0 slide-in-from-top-2")}
      header={<YourTurnHeader requests={requests} returned={returned.length} />}
      help={<YourTurnHelpPopover />}
      actions={<MoreActionsMenu name="Your Turn" menu={<YourTurnMenu requests={requests} returned={returned} />} />}
    >
      <YourTurnBody requests={requests} returned={returned} repositoryPath={repositoryPath} />
    </CollapsibleCard>
  )
}

/**
 * The title and the count of both kinds (the requests that wait, which may be more than GitHub lists), open or
 * collapsed, as every island keeps its count; collapsed, "oldest waiting 4 days · 1 returned" after it
 */
function YourTurnHeader({ requests, returned }: { requests: readonly ReviewRequest[]; returned: number }) {
  const totalCount = useReviewRequestsTotalCount()
  const oldest = requests[0]
  return (
    <div className="flex min-w-0 items-center gap-2" data-testid="your-turn-header">
      <UserCheckIcon className="size-4 shrink-0 text-muted-foreground" />
      <h2 className="shrink-0 text-sm font-semibold text-foreground">Your Turn</h2>
      <Badge variant="secondary" data-testid="your-turn-count">
        {(oldest === undefined ? 0 : totalCount) + returned}
      </Badge>
      <span className="hidden min-w-0 truncate text-xs text-muted-foreground group-data-[state=closed]:inline" data-testid="your-turn-summary">
        {oldest !== undefined && <OldestWaiting oldest={oldest} />}
        {returned > 0 && `${oldest === undefined ? "" : " · "}${returned} returned`}
      </span>
    </div>
  )
}

function OldestWaiting({ oldest }: { oldest: ReviewRequest }) {
  const now = useNow(RELATIVE_TIME_TICK_MS)
  return <span className={cn(hasWaitedWorkingDay(oldest.waitingSince * 1000, now) && "text-warning")}>{formatOldestWaiting(oldest, now)}</span>
}

function YourTurnHelpPopover() {
  return (
    <HelpPopover title="Your Turn" data-testid="your-turn-help">
      <p>
        The open pull requests that ask for your review by name, the longest waiting first, then your own ones a reviewer handed back. Requests of a team you are in and drafts are
        not shown, nor a request you answered with a comment until the other side replies or pushes.
      </p>
      <HelpSection title="Ages">
        <HelpList>
          <li>Muted while a request is young.</li>
          <li>
            <span className="text-warning">Highlighted</span>
            {" after one working day; weekends don't count."}
          </li>
          <li>&quot;re-requested&quot;: you reviewed it before and were asked again.</li>
          <li>&quot;replied&quot;, &quot;new commits&quot;: you commented, and the other side answered since.</li>
        </HelpList>
      </HelpSection>
      <p>GitHub is asked every five minutes, for the repository of this checkout or the one it is mirrored to.</p>
    </HelpPopover>
  )
}

/**
 * Refresh, with when GitHub was asked; "Open All on GitHub" of the requests; "Notification Settings…". Refresh is here,
 * not a footer row, so the island is its rows
 */
function YourTurnMenu({ requests, returned }: { requests: readonly ReviewRequest[]; returned: readonly ReturnedPullRequest[] }) {
  const checkedAt = useReviewRequestsCheckedAt()
  const isRefreshing = useIsRefreshingReviewRequests()
  const url = requests[0]?.url ?? returned[0]?.pullRequest.url
  const repository = url === undefined ? null : repositoryOfPullRequestUrl(url)
  return (
    <DropdownMenuItems
      groups={[
        [
          {
            id: "refresh-review-requests",
            label: "Refresh",
            icon: RefreshCwIcon,
            disabled: isRefreshing,
            ...(checkedAt !== null && { description: `Checked ${formatRelativeTime(checkedAt)}` }),
            onSelect: () => void refreshReviewRequests(),
          },
        ],
        repository === null || requests.length === 0
          ? []
          : [{ label: "Open All on GitHub", icon: ExternalLinkIcon, onSelect: () => openPullRequestUrl(reviewRequestsSearchUrl(repository)) }],
        [{ id: "show-all-reviews", label: "Show All Reviews", icon: GitPullRequestIcon, kbds: ["mod", "2"], onSelect: () => showMainView("reviews") }],
        [{ label: "Notification Settings…", icon: SettingsIcon, onSelect: () => openSettings({ page: "notifications" }) }],
      ]}
    />
  )
}

export interface YourTurnBodyProps {
  requests: readonly ReviewRequest[]
  returned: readonly ReturnedPullRequest[]
  repositoryPath: string
  /**
   * The words of the filter of Reviews, which `requests` and `returned` already match: every match shows, and while
   * GitHub has more than it lists, "First 50 of 63 filtered · Search on GitHub" with them
   */
  filterText?: string
}

/**
 * The rows (3, then "Show N More"; unread ones always), "50 of 63" while GitHub has more than it lists, expanded or
 * not, the returned pull requests, and while GitHub can't be asked the line that says why; filtered, every match
 */
export function YourTurnBody({ requests, returned, repositoryPath, filterText = "" }: YourTurnBodyProps) {
  const unread = useUnreadReviewRequests()
  const totalCount = useReviewRequestsTotalCount()
  const [isExpanded, setExpanded] = useState(false)
  const filtered = filterText.trim() !== ""
  const expanded = isExpanded && !filtered
  const visible = expanded || filtered ? requests : pickVisibleRequests(requests, unread)
  const hidden = requests.length - visible.length
  // GitHub lists the 50 oldest: the count of the header is its total
  const moreOnGitHub = totalCount > requests.length
  const [focused, setFocused] = useState<number | null>(null)
  const tabStop = visible.some(it => it.number === focused) ? focused : visible[0]?.number

  const handleKeyDown = (event: KeyboardEvent<HTMLUListElement>) =>
    handleReviewRowKeys(
      event,
      visible.map(it => it.number),
      {
        onEnter: number => openReviewRequest(repositoryPath, number),
        // D reviews the changes, as D in the review shows them
        onReview: number => openReviewRequest(repositoryPath, number, "review"),
        onOpen: number => {
          const request = visible.find(it => it.number === number)
          if (request !== undefined) {
            openRequestOnGitHub(request)
          }
        },
      },
      setFocused,
    )

  return (
    <div className="flex flex-col divide-y divide-border">
      {requests.length > 0 && (
        <ul aria-label="Pull requests waiting for your review" className="flex flex-col divide-y divide-border" data-testid="review-request-list" onKeyDown={handleKeyDown}>
          {visible.map(request => (
            <ReviewRequestRow
              key={request.number}
              request={request}
              repositoryPath={repositoryPath}
              unread={unread.has(reviewRequestKey(request))}
              tabStop={request.number === tabStop}
            />
          ))}
        </ul>
      )}
      {filtered
        ? moreOnGitHub &&
          requests.length > 0 && (
            <div className="flex items-center gap-3 px-4 py-1 sm:px-6">
              <FilteredOnGitHub count={requests.length} totalCount={totalCount} url={filteredSearchUrl(requests, filterText)} />
            </div>
          )
        : (hidden > 0 || expanded || moreOnGitHub) && (
            <div className="flex items-center gap-3 px-2 py-1 sm:px-4">
              {(hidden > 0 || expanded) && (
                <Button variant="ghost" size="xs" className="text-muted-foreground" data-testid="review-requests-show-more" onClick={() => setExpanded(!expanded)}>
                  {expanded ? "Show Less" : `Show ${hidden} More`}
                </Button>
              )}
              {moreOnGitHub && <MoreOnGitHub requests={requests} totalCount={totalCount} />}
            </div>
          )}
      {returned.length > 0 && <ReturnedList returned={returned} />}
      <YourTurnStatusLine />
    </div>
  )
}

/** The search of the requests on GitHub with the words of the filter; none for a number or a link, which names one pull request */
function filteredSearchUrl(requests: readonly ReviewRequest[], text: string): string | null {
  const repository = repositoryOfPullRequestUrl(requests[0]!.url)
  return repository === null || typedReference(text) !== null ? null : reviewRequestsSearchUrl(repository, text)
}

/** "50 of 63 · Open All on GitHub": GitHub returns the 50 oldest */
function MoreOnGitHub({ requests, totalCount }: { requests: readonly ReviewRequest[]; totalCount: number }) {
  const repository = repositoryOfPullRequestUrl(requests[0]!.url)
  return (
    <span className="flex items-center gap-2 text-xs text-muted-foreground" data-testid="review-requests-more-on-github">
      {`${requests.length} of ${totalCount}`}
      {repository !== null && (
        <Button variant="link" size="xs" className="h-auto px-0" onClick={() => openPullRequestUrl(reviewRequestsSearchUrl(repository))}>
          Open All on GitHub
        </Button>
      )}
    </span>
  )
}

/** The own pull requests handed back, as rows after the requests: ↑/↓ between them, Enter shows the branch, O opens it */
function ReturnedList({ returned }: { returned: readonly ReturnedPullRequest[] }) {
  const [focused, setFocused] = useState<string | null>(null)
  const tabStop = returned.some(it => it.branchName === focused) ? focused : returned[0]?.branchName
  const handleKeyDown = (event: KeyboardEvent<HTMLUListElement>) => {
    if (event.defaultPrevented || event.altKey || event.metaKey || event.ctrlKey || event.shiftKey) {
      return
    }
    const row = (event.target as HTMLElement).closest<HTMLElement>("[data-returned-branch]")
    const name = row?.dataset.returnedBranch ?? null
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault()
      const next = neighbourRow(
        returned.map(it => it.branchName),
        name,
        event.key === "ArrowDown" ? 1 : -1,
      )
      const target = next === null ? null : event.currentTarget.querySelector<HTMLElement>(`[data-returned-branch="${CSS.escape(next)}"]`)
      if (target != null) {
        setFocused(next)
        target.focus()
      }
    }
  }
  return (
    <ul aria-label="Your pull requests returned with changes requested" className="flex flex-col divide-y divide-border" data-testid="returned-list" onKeyDown={handleKeyDown}>
      {returned.map(item => (
        <ReturnedPullRequestRow key={item.branchName} item={item} tabStop={item.branchName === tabStop} />
      ))}
    </ul>
  )
}

/**
 * While GitHub can't be asked: the hint, how old the rows are, and Refresh ("GitHub: offline · checked 5 minutes ago");
 * nothing otherwise, Refresh and the time being in "⋯"
 */
function YourTurnStatusLine() {
  const availability = useReviewRequestsAvailability()
  const checkedAt = useReviewRequestsCheckedAt()
  const isRefreshing = useIsRefreshingReviewRequests()
  const hintAction = useAvailabilityAction(availability)
  const hint = formatAvailabilityHint(availability)
  if (hint === null) {
    return null
  }
  return (
    <div className="flex flex-col gap-1 px-4 py-2 text-xs sm:px-6">
      <ReviewStatusLine
        hint={hint}
        hintAction={hintAction}
        checkedLabel="Checked"
        checkedAt={checkedAt}
        ageWithHint
        isRefreshing={isRefreshing}
        onRefresh={() => void refreshReviewRequests()}
        refreshLabel="Refresh review requests"
        testId="review-requests-status-line"
        refreshTestId="review-requests-refresh"
      />
    </div>
  )
}
