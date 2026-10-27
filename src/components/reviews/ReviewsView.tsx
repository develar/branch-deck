import { useEffect, useRef, useState } from "react"
import type { KeyboardEvent, ReactNode, RefObject } from "react"
import { ExternalLinkIcon, GitPullRequestIcon, RefreshCwIcon, UserCheckIcon } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Spinner } from "@/components/ui/spinner"
import { ReviewRequestRow } from "@/components/repository/ReviewRequestRow"
import { YourTurnBody } from "@/components/repository/YourTurnCard"
import { Callout } from "@/components/shared/Callout"
import { CollapsibleCard } from "@/components/shared/CollapsibleCard"
import { DropdownMenuItems } from "@/components/shared/DropdownMenuItems"
import { MoreActionsMenu } from "@/components/shared/MoreActionsMenu"
import { useAvailabilityAction } from "@/hooks/useAvailabilityAction"
import { RELATIVE_TIME_TICK_MS, useNow } from "@/hooks/useNow"
import { useReturnedToYou } from "@/hooks/useReturnedToYou"
import type { PullRequestAvailability, ReviewEntry, ReviewSection } from "@/lib/bindings"
import { describeError } from "@/lib/errors"
import { formatAvailabilityHint, openPullRequestUrl } from "@/lib/pull-request"
import { openPullRequestReviewWindow } from "@/lib/pull-request-review-window"
import { handleReviewRowKeys } from "@/lib/review-row-keys"
import { formatRelativeTime } from "@/lib/time"
import { showReviews } from "@/model/demand"
import { REVIEW_SECTION_TITLES, formatReviewAge, formatReviewMeta, reviewSectionSearchUrl, reviewsSearchUrl } from "@/presenters/reviews"
import { requestReveal } from "@/stores/commit-move"
import { useSelectedProjectPath } from "@/stores/repository"
import { openRequestOnGitHub, openReviewRequest, useReviewRequests, useReviewRequestsAvailability } from "@/stores/review-requests"
import { linkPullRequest, onLinkFieldFocus, refreshReviews, reviewRows, unlinkPullRequest, useIsRefreshingReviews, useReviews } from "@/stores/reviews"

/**
 * The Reviews view of the main window (spec/repository/reviews.spec.md): every open pull request of the repository the
 * user takes part in on GitHub. Your Turn's rows lead it ("Waiting for You"), then the sections Rust derives (added by
 * link, requested from the user's teams, reviewed by the user, the user's own), each pull request once. The field adds
 * a pull request by its link, as when the author forgot to ask the user. The session reads the rest of Reviews only
 * while the view shows (`showReviews`, the demand); a state of GitHub is the state of the view, never a toast.
 */
export function ReviewsView({ visible, headingRef }: { visible: boolean; headingRef: RefObject<HTMLHeadingElement | null> }) {
  const repositoryPath = useSelectedProjectPath()
  const reviews = useReviews()
  const requests = useReviewRequests()
  const returned = useReturnedToYou()
  const availability = useReviewRequestsAvailability()
  // the session reads the rest of Reviews while the view shows, focused or not
  useEffect(() => (visible ? showReviews() : undefined), [visible])
  // the first show loads this chunk after the switch: the focus the hidden branches dropped comes to the heading here
  useEffect(() => {
    if (document.activeElement === null || document.activeElement === document.body) {
      headingRef.current?.focus({ preventScroll: true })
    }
  }, [headingRef])
  const repository = availability?.status === "available" ? availability : null
  const loading = reviews.head.state.kind === "loading"
  const empty = requests.length === 0 && returned.length === 0 && reviews.sections.length === 0
  return (
    <div className="flex flex-col gap-2" data-testid="reviews-view" aria-busy={loading}>
      <div className="flex min-h-9 flex-wrap items-center gap-3 px-1">
        <h1 ref={headingRef} tabIndex={-1} className="text-sm font-semibold text-foreground outline-none">
          Reviews
        </h1>
        {repository !== null && <span className="min-w-0 truncate text-xs text-muted-foreground" data-testid="reviews-repository">{`${repository.owner}/${repository.name}`}</span>}
        {repositoryPath !== undefined && repository !== null && <LinkField repositoryPath={repositoryPath} />}
        <div className="ml-auto flex items-center gap-2">
          {loading && <Spinner className="size-3 text-muted-foreground" />}
          {repositoryPath !== undefined && <MoreActionsMenu name="Reviews" menu={<ReviewsMenu repositoryPath={repositoryPath} repository={repository} />} />}
        </div>
      </div>
      <ReviewsState availability={availability} empty={empty && !loading} repository={repository} />
      {repositoryPath !== undefined && (requests.length > 0 || returned.length > 0) && (
        <CollapsibleCard
          data-testid="reviews-section-waiting"
          defaultOpen
          header={<SectionHeader icon={<UserCheckIcon className="size-4 shrink-0 text-muted-foreground" />} title="Waiting for You" count={requests.length + returned.length} />}
        >
          <YourTurnBody requests={requests} returned={returned} repositoryPath={repositoryPath} />
        </CollapsibleCard>
      )}
      {repositoryPath !== undefined &&
        reviews.sections.map(section => <ReviewSectionCard key={section.kind} section={section} repositoryPath={repositoryPath} repository={repository} />)}
    </div>
  )
}

/** The title of a section with its count, open or collapsed, as every island keeps its count */
function SectionHeader({ icon, title, count }: { icon: ReactNode; title: string; count: number }) {
  return (
    <div className="flex min-w-0 items-center gap-2">
      {icon}
      <h2 className="shrink-0 text-sm font-semibold text-foreground">{title}</h2>
      <Badge variant="secondary">{count}</Badge>
    </div>
  )
}

/**
 * "Add pull request by link or #number": Enter asks Rust for it (`link_pull_request`), which keeps an open one; the
 * review window opens on it. A refusal is said under the field, not in a toast
 */
function LinkField({ repositoryPath }: { repositoryPath: string }) {
  const [input, setInput] = useState("")
  const [error, setError] = useState<string | null>(null)
  const [adding, setAdding] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)
  // the palette's "Add Pull Request by Link…": once the view shows, a frame later
  useEffect(() => onLinkFieldFocus(() => requestAnimationFrame(() => inputRef.current?.focus())), [])
  const add = () => {
    const value = input.trim()
    if (value === "" || adding) {
      return
    }
    setAdding(true)
    void linkPullRequest(repositoryPath, value)
      .then(answer => {
        if (answer.kind === "refused") {
          setError(describeError(answer.error))
          return
        }
        setError(null)
        setInput("")
        const { requests } = reviewRows(repositoryPath)
        void openPullRequestReviewWindow(
          repositoryPath,
          answer.entry.request,
          requests.findIndex(request => request.number === answer.entry.request.number),
          null,
          "reviews",
        )
      })
      .finally(() => setAdding(false))
  }
  return (
    <div className="flex min-w-48 flex-1 flex-col gap-1 sm:max-w-96">
      <Input
        ref={inputRef}
        value={input}
        placeholder="Add pull request by link or #number"
        aria-label="Add pull request by link or #number"
        aria-invalid={error !== null || undefined}
        aria-describedby={error === null ? undefined : "reviews-link-error"}
        disabled={adding}
        data-testid="reviews-link-input"
        onChange={event => {
          setInput(event.target.value)
          setError(null)
        }}
        onKeyDown={event => {
          if (event.key === "Enter") {
            event.preventDefault()
            add()
          } else if (event.key === "Escape" && input !== "") {
            event.preventDefault()
            setInput("")
            setError(null)
          }
        }}
      />
      {error !== null && (
        <p id="reviews-link-error" className="px-2.5 text-xs text-destructive" data-testid="reviews-link-error">
          {error}
        </p>
      )}
    </div>
  )
}

/** Refresh with when GitHub was asked, and every open pull request the user takes part in on GitHub */
function ReviewsMenu({ repositoryPath, repository }: { repositoryPath: string; repository: { owner: string; name: string } | null }) {
  const reviews = useReviews()
  const refreshing = useIsRefreshingReviews()
  return (
    <DropdownMenuItems
      groups={[
        [
          {
            id: "refresh-reviews",
            label: "Refresh",
            icon: RefreshCwIcon,
            disabled: refreshing,
            ...(reviews.checkedAt !== 0 && { description: `Checked ${formatRelativeTime(reviews.checkedAt * 1000)}` }),
            onSelect: () => void refreshReviews(repositoryPath),
          },
        ],
        repository === null
          ? []
          : [{ id: "open-reviews-on-github", label: "Open on GitHub", icon: ExternalLinkIcon, onSelect: () => openPullRequestUrl(reviewsSearchUrl(repository)) }],
      ]}
    />
  )
}

/**
 * What stands in the way of Reviews, as the state of the view: not on GitHub, no token or a rejected one (Sign In), no
 * access, a rate limit; and nothing at all to show
 */
function ReviewsState({ availability, empty, repository }: { availability: PullRequestAvailability | null; empty: boolean; repository: { owner: string; name: string } | null }) {
  const action = useAvailabilityAction(availability)
  if (availability?.status === "notGitHub") {
    return (
      <Callout tone="info" title="The pull requests of this repository are not on GitHub" data-testid="reviews-state">
        Reviews shows the pull requests of the GitHub repository the branches are pushed to.
      </Callout>
    )
  }
  const hint = formatAvailabilityHint(availability)
  if (hint !== null) {
    return (
      <Callout tone="warning" title="Cannot read the reviews of GitHub" data-testid="reviews-state">
        <p>{hint}</p>
        {action !== null && (
          <div className="-ml-2">
            <Button size="xs" variant="ghost" onClick={action.onClick}>
              {action.label}
            </Button>
          </div>
        )}
      </Callout>
    )
  }
  if (empty && repository !== null) {
    return (
      <p className="px-1 text-sm text-muted-foreground" data-testid="reviews-empty">
        {`No open pull requests involve you in ${repository.owner}/${repository.name}`}
      </p>
    )
  }
  return null
}

/** A section of Reviews: its rows, every one, and "50 of 63 · Open All on GitHub" when GitHub has more */
function ReviewSectionCard({ section, repositoryPath, repository }: { section: ReviewSection; repositoryPath: string; repository: { owner: string; name: string } | null }) {
  const [focused, setFocused] = useState<number | null>(null)
  const entries = section.entries
  const tabStop = entries.some(entry => entry.request.number === focused) ? focused : entries[0]?.request.number
  const search = repository === null ? null : reviewSectionSearchUrl(section.kind, repository)
  const entryOf = (number: number) => entries.find(entry => entry.request.number === number)
  const handleKeyDown = (event: KeyboardEvent<HTMLUListElement>) =>
    handleReviewRowKeys(
      event,
      entries.map(entry => entry.request.number),
      {
        // an own pull request of the table shows its branch, as Your Turn's returned rows do
        onEnter: number => {
          const involvement = entryOf(number)?.involvement
          if (involvement?.kind === "authored" && involvement.branch !== null) {
            requestReveal({ branchName: involvement.branch, commitIds: [] })
          } else {
            openReviewRequest(repositoryPath, number, null, "reviews")
          }
        },
        onReview: number => openReviewRequest(repositoryPath, number, "review", "reviews"),
        onOpen: number => {
          const entry = entryOf(number)
          if (entry !== undefined) {
            openRequestOnGitHub(entry.request)
          }
        },
      },
      setFocused,
    )
  return (
    <CollapsibleCard
      data-testid={`reviews-section-${section.kind}`}
      defaultOpen
      header={
        <SectionHeader icon={<GitPullRequestIcon className="size-4 shrink-0 text-muted-foreground" />} title={REVIEW_SECTION_TITLES[section.kind]} count={section.totalCount} />
      }
    >
      <div className="flex flex-col divide-y divide-border">
        <ul aria-label={REVIEW_SECTION_TITLES[section.kind]} className="flex flex-col divide-y divide-border" onKeyDown={handleKeyDown}>
          {entries.map(entry => (
            <ReviewEntryRow key={entry.request.number} entry={entry} repositoryPath={repositoryPath} tabStop={entry.request.number === tabStop} />
          ))}
        </ul>
        {section.totalCount > entries.length && search !== null && (
          <span className="flex items-center gap-2 px-4 py-1 text-xs text-muted-foreground sm:px-6" data-testid="reviews-more-on-github">
            {`${entries.length} of ${section.totalCount}`}
            <Button variant="link" size="xs" className="h-auto px-0" onClick={() => openPullRequestUrl(search)}>
              Open All on GitHub
            </Button>
          </span>
        )}
      </div>
    </CollapsibleCard>
  )
}

/** A pull request of Reviews in the row of a request: its meta and age by how the user takes part */
function ReviewEntryRow({ entry, repositoryPath, tabStop }: { entry: ReviewEntry; repositoryPath: string; tabStop: boolean }) {
  const now = useNow(RELATIVE_TIME_TICK_MS)
  const { request, involvement } = entry
  const branch = involvement.kind === "authored" ? involvement.branch : null
  return (
    <ReviewRequestRow
      request={request}
      repositoryPath={repositoryPath}
      unread={false}
      tabStop={tabStop}
      source="reviews"
      meta={formatReviewMeta(entry)}
      age={
        <span className="text-xs whitespace-nowrap text-muted-foreground tabular-nums" data-testid="review-request-age">
          {formatReviewAge(entry, now)}
        </span>
      }
      {...(branch !== null && { onShowBranch: () => requestReveal({ branchName: branch, commitIds: [] }) })}
      {...(involvement.kind === "linked" && { onRemoveFromReviews: () => void unlinkPullRequest(repositoryPath, request.number) })}
    />
  )
}
