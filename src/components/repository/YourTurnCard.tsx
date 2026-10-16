import { useMemo, useState } from "react"
import type { KeyboardEvent } from "react"
import { ChevronDownIcon, ExternalLinkIcon, Settings2Icon, UserCheckIcon } from "lucide-react"
import { cn } from "cn"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { ReviewRequestRow } from "@/components/repository/ReviewRequestRow"
import { ReviewStatusLine } from "@/components/repository/ReviewStatusLine"
import { CollapsibleCard } from "@/components/shared/CollapsibleCard"
import { DropdownMenuItems } from "@/components/shared/DropdownMenuItems"
import { HelpList, HelpPopover, HelpSection } from "@/components/shared/HelpPopover"
import { MoreActionsMenu } from "@/components/shared/MoreActionsMenu"
import { RELATIVE_TIME_TICK_MS, useNow } from "@/hooks/useNow"
import type { ReviewRequest } from "@/lib/bindings"
import { neighbourRow } from "@/lib/branch-row-keys"
import { collectReturnedToYou, formatReturnedItem } from "@/lib/outgoing-attention"
import { formatAvailabilityHint, openPullRequestUrl } from "@/lib/pull-request"
import { formatCollapsedSummary, hasWaitedWorkingDay, pickVisibleRequests, repositoryOfPullRequestUrl, reviewRequestKey, reviewRequestsSearchUrl } from "@/lib/review-requests"
import { useBranchTableRows } from "@/stores/branch-sync"
import { requestReveal } from "@/stores/commit-move"
import { useRepositoryPullRequestMap } from "@/stores/pull-requests"
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
import { setSettingsDialogOpen } from "@/stores/settings-dialog"
import { setReviewRequestsCollapsed, useRecentProjects } from "@/stores/settings/repositorySettings"

/**
 * "Your Turn": the island of the pull requests of others that wait for the user's review (requested by name), above the
 * branches, waiting the longest first; "Returned to you" names the user's own pull requests a reviewer handed back.
 * Hidden while nothing waits, off GitHub, without `gh` or logged out; a failed refresh keeps the rows. Open by default,
 * collapsed per repository.
 */
export function YourTurnCard() {
  const requests = useReviewRequests()
  const repositoryPath = useSelectedProjectPath()
  const collapsed = useRecentProjects(projects => projects[0]?.reviewRequestsCollapsed === true)
  // the first entrance slides in, as the Landed island; later the rows only fade
  const [entered] = useState(() => requests.length > 0)
  if (requests.length === 0 || repositoryPath === undefined) {
    return null
  }
  return (
    <CollapsibleCard
      data-testid="your-turn-card"
      open={!collapsed}
      onOpenChange={open => setReviewRequestsCollapsed(repositoryPath, !open)}
      className={cn(!entered && "animate-in duration-300 fade-in-0 slide-in-from-top-2")}
      header={<YourTurnHeader requests={requests} />}
      actions={(
        <>
          <YourTurnHelpPopover />
          <MoreActionsMenu name="Your Turn" menu={<YourTurnMenu requests={requests} />} />
        </>
      )}
    >
      <YourTurnBody requests={requests} repositoryPath={repositoryPath} />
    </CollapsibleCard>
  )
}

/** The title and the count; collapsed, "3 reviews · oldest waiting 4 days" instead of the count */
function YourTurnHeader({ requests }: { requests: readonly ReviewRequest[] }) {
  const totalCount = useReviewRequestsTotalCount()
  return (
    <div className="flex items-center justify-between gap-3" data-testid="your-turn-header">
      <div className="flex min-w-0 items-center gap-2">
        <UserCheckIcon className="size-4 shrink-0 text-muted-foreground" />
        <h2 className="shrink-0 text-sm font-semibold text-foreground">Your Turn</h2>
        <Badge variant="secondary" className="group-data-[state=closed]:hidden" data-testid="your-turn-count">{totalCount}</Badge>
        <span className="hidden min-w-0 truncate text-xs text-muted-foreground group-data-[state=closed]:inline" data-testid="your-turn-summary">
          <CollapsedSummary count={totalCount} oldest={requests[0]!} />
        </span>
      </div>
      <ChevronDownIcon className="size-4 shrink-0 text-muted-foreground transition-transform duration-200 group-data-[state=open]:rotate-180" />
    </div>
  )
}

function CollapsedSummary({ count, oldest }: { count: number, oldest: ReviewRequest }) {
  const now = useNow(RELATIVE_TIME_TICK_MS)
  const summary = formatCollapsedSummary(count, oldest, now)
  return (
    <>
      {`${summary.count} · `}
      <span className={cn(hasWaitedWorkingDay(oldest.waitingSince * 1000, now) && "text-warning")}>{summary.age}</span>
    </>
  )
}

function YourTurnHelpPopover() {
  return (
    <HelpPopover label="About Your Turn" data-testid="your-turn-help">
      <HelpSection title="What is shown">
        <p>The open pull requests that ask for your review by name, waiting the longest first. Requests of a team you are in are not shown, nor drafts.</p>
      </HelpSection>
      <HelpSection title="Ages">
        <HelpList>
          <li>Muted while a request is young.</li>
          <li>
            <span className="text-warning">Warning</span>
            {" "}
            after one working day: weekends don't count.
          </li>
          <li>&quot;re-requested&quot;: you reviewed before and were asked again.</li>
        </HelpList>
      </HelpSection>
      <HelpSection title="Where from">
        <p>The GitHub repository of origin, or the one a Space checkout is mirrored to (in owner/name). GitHub is asked every five minutes.</p>
      </HelpSection>
    </HelpPopover>
  )
}

function YourTurnMenu({ requests }: { requests: readonly ReviewRequest[] }) {
  const repository = requests[0] === undefined ? null : repositoryOfPullRequestUrl(requests[0].url)
  return (
    <DropdownMenuItems groups={[
      repository === null ? [] : [{ label: "Open All on GitHub", icon: ExternalLinkIcon, onSelect: () => openPullRequestUrl(reviewRequestsSearchUrl(repository)) }],
      [{ label: "Notification Settings…", icon: Settings2Icon, onSelect: () => setSettingsDialogOpen(true) }],
    ]}
    />
  )
}

interface YourTurnBodyProps {
  requests: readonly ReviewRequest[]
  repositoryPath: string
}

/** The rows (3, then "Show N More"; unread ones always), "50 of 63", "Returned to you" and the status line */
function YourTurnBody({ requests, repositoryPath }: YourTurnBodyProps) {
  const unread = useUnreadReviewRequests()
  const totalCount = useReviewRequestsTotalCount()
  const [expanded, setExpanded] = useState(false)
  const visible = expanded ? requests : pickVisibleRequests(requests, unread)
  const hidden = requests.length - visible.length
  const [focused, setFocused] = useState<number | null>(null)
  const tabStop = visible.some(it => it.number === focused) ? focused : visible[0]?.number

  const handleKeyDown = (event: KeyboardEvent<HTMLUListElement>) => {
    if (event.defaultPrevented || event.altKey || event.metaKey || event.ctrlKey) {
      return
    }
    const row = (event.target as HTMLElement).closest<HTMLElement>("[data-review-number]")
    const number = row?.dataset.reviewNumber ?? null
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault()
      const next = neighbourRow(visible.map(it => String(it.number)), number, event.key === "ArrowDown" ? 1 : -1)
      const target = next === null ? null : event.currentTarget.querySelector<HTMLElement>(`[data-review-number="${next}"]`)
      if (target != null) {
        setFocused(Number(next))
        target.focus()
      }
      return
    }
    if (row == null || number === null || event.target !== row) {
      return
    }
    if ((event.key === "Enter" || event.key === " ") && !event.shiftKey) {
      event.preventDefault()
      openReviewRequest(repositoryPath, Number(number))
    }
    // the physical key, in any keyboard layout: the key of Open Pull Request in the row's menu
    else if (event.code === "KeyO" && !event.shiftKey) {
      const request = visible.find(it => it.number === Number(number))
      if (request !== undefined) {
        event.preventDefault()
        openRequestOnGitHub(request)
      }
    }
    // the context menu key and Shift+F10: the menu of the row, at its start
    else if (event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey)) {
      event.preventDefault()
      const rect = row.getBoundingClientRect()
      row.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: rect.left + 24, clientY: rect.bottom, button: 2 }))
    }
  }

  return (
    <div className="flex flex-col divide-y divide-border">
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
      {(hidden > 0 || expanded) && (
        <div className="flex items-center gap-3 px-2 py-1 sm:px-4">
          <Button variant="ghost" size="xs" className="text-muted-foreground" data-testid="review-requests-show-more" onClick={() => setExpanded(!expanded)}>
            {expanded ? "Show Less" : `Show ${hidden} More`}
          </Button>
          {expanded && totalCount > requests.length && <MoreOnGitHub requests={requests} totalCount={totalCount} />}
        </div>
      )}
      <div className="flex flex-col gap-1 px-4 py-2 text-xs sm:px-6">
        <ReturnedToYou />
        <YourTurnStatusLine />
      </div>
    </div>
  )
}

/** "50 of 63 · Open All on GitHub": GitHub returns the 50 oldest */
function MoreOnGitHub({ requests, totalCount }: { requests: readonly ReviewRequest[], totalCount: number }) {
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

/** "Returned to you: auth (changes requested) · ui (changes requested)": a name reveals its row */
function ReturnedToYou() {
  const rows = useBranchTableRows()
  const pullRequests = useRepositoryPullRequestMap()
  const returned = useMemo(() => collectReturnedToYou(rows, pullRequests), [rows, pullRequests])
  if (returned.length === 0) {
    return null
  }
  return (
    <p className="flex flex-wrap items-center gap-x-1 text-muted-foreground" data-testid="returned-to-you">
      <span>Returned to you:</span>
      {returned.map((item, index) => (
        <span key={item.branchName} className="flex items-center gap-1">
          {index > 0 && <span aria-hidden>·</span>}
          <button
            type="button"
            className="cursor-pointer rounded-sm text-foreground outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/30"
            data-branch-name={item.branchName}
            onClick={() => requestReveal({ branchName: item.branchName, commitIds: [] })}
          >
            {formatReturnedItem(item)}
          </button>
        </span>
      ))}
    </p>
  )
}

/** "Checked just now · Refresh"; after a failure the hint, and how old the rows are */
function YourTurnStatusLine() {
  const availability = useReviewRequestsAvailability()
  const checkedAt = useReviewRequestsCheckedAt()
  const isRefreshing = useIsRefreshingReviewRequests()
  return (
    <ReviewStatusLine
      hint={formatAvailabilityHint(availability)}
      checkedLabel="Checked"
      checkedAt={checkedAt}
      ageWithHint
      isRefreshing={isRefreshing}
      onRefresh={() => void refreshReviewRequests("manual")}
      refreshLabel="Refresh review requests"
      testId="review-requests-status-line"
      refreshTestId="review-requests-refresh"
    />
  )
}
