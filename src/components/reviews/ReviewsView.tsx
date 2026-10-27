import { useEffect, useRef, useState } from "react"
import type { KeyboardEvent, ReactNode, RefObject } from "react"
import { ExternalLinkIcon, GitPullRequestIcon, RefreshCwIcon, UserCheckIcon } from "lucide-react"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { YourTurnBody } from "@/components/repository/YourTurnCard"
import { FilteredOnGitHub } from "@/components/reviews/FilteredOnGitHub"
import { ReviewEntryRow } from "@/components/reviews/ReviewEntryRow"
import { Callout } from "@/components/shared/Callout"
import { CollapsibleCard } from "@/components/shared/CollapsibleCard"
import { DropdownMenuItems } from "@/components/shared/DropdownMenuItems"
import { HelpList, HelpPopover, HelpSection } from "@/components/shared/HelpPopover"
import { MoreActionsMenu } from "@/components/shared/MoreActionsMenu"
import { SearchField } from "@/components/shared/SearchField"
import { useAvailabilityAction } from "@/hooks/useAvailabilityAction"
import { useReturnedToYou } from "@/hooks/useReturnedToYou"
import type { PullRequestAvailability, ReviewEntry, ReviewRequest, ReviewSection } from "@/lib/bindings"
import { describeError } from "@/lib/errors"
import type { ReturnedPullRequest } from "@/lib/outgoing-attention"
import { formatAvailabilityHint, openPullRequestUrl } from "@/lib/pull-request"
import { openPullRequestReviewWindow } from "@/lib/pull-request-review-window"
import { handleReviewRowKeys } from "@/lib/review-row-keys"
import { matchesAllWords, searchWords } from "@/lib/text-search"
import { formatRelativeTime } from "@/lib/time"
import { showReviews } from "@/model/demand"
import {
  REVIEW_SECTION_TITLES,
  formatReviewsNoMatch,
  pickVisibleEntries,
  returnedTexts,
  reviewEntryTexts,
  reviewRequestTexts,
  reviewSectionSearchUrl,
  reviewsSearchUrl,
  shownPullRequestOf,
  typedReference,
} from "@/presenters/reviews"
import { requestReveal } from "@/stores/commit-move"
import { useSelectedProjectPath } from "@/stores/repository"
import { openRequestOnGitHub, openReviewRequest, useReviewRequests, useReviewRequestsAvailability, useReviewRequestsTotalCount } from "@/stores/review-requests"
import { linkPullRequest, onLinkFieldFocus, refreshReviews, reviewRows, useIsRefreshingReviews, useReviews } from "@/stores/reviews"

/**
 * The Reviews view of the main window (spec/repository/reviews.spec.md): every open pull request of the repository the
 * user takes part in on GitHub. Your Turn's rows lead it ("Waiting for You"), then the sections Rust derives (added by
 * link, requested from the user's teams, reviewed by the user, the user's own), each pull request once. The field
 * filters every section by the words typed, and adds a pull request by its link, as when the author forgot to ask the
 * user. The session reads the rest of Reviews only while the view shows (`showReviews`, the demand); a state of GitHub
 * is the state of the view, never a toast.
 */
export function ReviewsView({ visible, headingRef }: { visible: boolean; headingRef: RefObject<HTMLHeadingElement | null> }) {
  const repositoryPath = useSelectedProjectPath()
  const reviews = useReviews()
  const requests = useReviewRequests()
  const requestsTotalCount = useReviewRequestsTotalCount()
  const returned = useReturnedToYou()
  const availability = useReviewRequestsAvailability()
  const viewRef = useRef<HTMLDivElement>(null)
  // the session reads the rest of Reviews while the view shows, focused or not
  useEffect(() => (visible ? showReviews() : undefined), [visible])
  // the first show loads this chunk after the switch: the focus the hidden branches dropped comes to the heading here
  useEffect(() => {
    if (document.activeElement === null || document.activeElement === document.body) {
      headingRef.current?.focus({ preventScroll: true })
    }
  }, [headingRef])
  // the text of the field by repository: another repository starts without a filter, a switch of the views keeps it
  const [filterTexts, setFilterTexts] = useState<Readonly<Record<string, string>>>({})
  const filterText = repositoryPath === undefined ? "" : (filterTexts[repositoryPath] ?? "")
  // the sections the user collapsed, and those collapsed while filtering, which clearing the filter forgets
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set())
  const [filterCollapsed, setFilterCollapsed] = useState<ReadonlySet<string>>(new Set())
  const [adding, setAdding] = useState(false)
  const [linkError, setLinkError] = useState<string | null>(null)

  const repository = availability?.status === "available" ? availability : null
  const loading = reviews.head.state.kind === "loading"
  const empty = requests.length === 0 && returned.length === 0 && reviews.sections.length === 0
  const words = searchWords(filterText)
  const reference = typedReference(filterText)
  const filtering = words.length > 0

  // every row of the view in its order, what Enter on it does: the lookup of a typed number or link
  const rows =
    repositoryPath === undefined
      ? []
      : rowsOf(
          repositoryPath,
          requests,
          returned,
          reviews.sections.flatMap(section => section.entries),
        )
  // a pasted link of a shown pull request shows that one; words show the rows that have every one of them
  const linked = reference?.kind === "link" ? shownPullRequestOf(reference, rows) : undefined
  const matches = (number: number, texts: readonly string[]) => (linked === undefined ? matchesAllWords(texts, words) : number === linked.number)
  const shownRequests = filtering ? requests.filter(request => matches(request.number, reviewRequestTexts(request))) : requests
  const shownReturned = filtering ? returned.filter(item => matches(item.pullRequest.number, returnedTexts(item))) : returned
  const shownSections = reviews.sections
    .map(section => ({ section, entries: filtering ? section.entries.filter(entry => matches(entry.request.number, reviewEntryTexts(entry))) : section.entries }))
    .filter(it => it.entries.length > 0)
  const shown =
    repositoryPath === undefined
      ? []
      : rowsOf(
          repositoryPath,
          shownRequests,
          shownReturned,
          shownSections.flatMap(it => it.entries),
        )
  // GitHub returned only a part of a list: the filter saw only that part
  const isCut = requestsTotalCount > requests.length || reviews.sections.some(section => section.totalCount > section.entries.length)

  const setFilterText = (text: string) => {
    if (repositoryPath === undefined) {
      return
    }
    setFilterTexts(texts => ({ ...texts, [repositoryPath]: text }))
    setLinkError(null)
    if (searchWords(text).length === 0) {
      setFilterCollapsed(new Set())
    }
  }
  const isOpen = (kind: string) => !(filtering ? filterCollapsed : collapsed).has(kind)
  const setOpen = (kind: string, open: boolean) => (filtering ? setFilterCollapsed : setCollapsed)(set => withMember(set, kind, !open))
  const focusFirstRow = () => viewRef.current?.querySelector<HTMLElement>("[data-review-number], [data-returned-branch]")?.focus()

  // "Add pull request by link or #number": Rust reads it (`link_pull_request`) and keeps an open one; the review window
  // opens on it. A refusal is said under the field, not in a toast
  const addLink = () => {
    const value = filterText.trim()
    if (repositoryPath === undefined || value === "" || adding) {
      return
    }
    setAdding(true)
    void linkPullRequest(repositoryPath, value)
      .then(answer => {
        if (answer.kind === "refused") {
          setLinkError(describeError(answer.error))
          return
        }
        setFilterText("")
        const { requests: reviewed } = reviewRows(repositoryPath)
        void openPullRequestReviewWindow(
          repositoryPath,
          answer.entry.request,
          reviewed.findIndex(request => request.number === answer.entry.request.number),
          null,
          "reviews",
        )
      })
      .finally(() => setAdding(false))
  }
  // Enter: a number or a link of a shown pull request opens it, of another one adds it; words open their one match, or
  // move the focus to the first of their matches
  const onEnter = () => {
    if (reference !== null) {
      const row = shownPullRequestOf(reference, rows)
      if (row === undefined) {
        addLink()
      } else {
        row.open()
      }
    } else if (filtering && shown.length === 1) {
      shown[0]!.open()
    } else if (filtering && shown.length > 1) {
      focusFirstRow()
    }
  }

  return (
    <div ref={viewRef} className="flex flex-col gap-2" data-testid="reviews-view" aria-busy={loading}>
      <div className="flex min-h-9 flex-wrap items-center gap-3 px-1">
        <h1 ref={headingRef} tabIndex={-1} className="text-sm font-semibold text-foreground outline-none">
          Reviews
        </h1>
        {repository !== null && <span className="min-w-0 truncate text-xs text-muted-foreground" data-testid="reviews-repository">{`${repository.owner}/${repository.name}`}</span>}
        {repositoryPath !== undefined && (repository !== null || !empty) && (
          <ReviewsFilterField
            value={filterText}
            onValueChange={setFilterText}
            onEnter={onEnter}
            onLeave={() => headingRef.current?.focus()}
            onArrowDown={focusFirstRow}
            busy={adding}
            error={linkError}
          />
        )}
        <ReviewsHelpPopover />
        <div className="ml-auto flex items-center gap-2">
          {loading && <Spinner className="size-3 text-muted-foreground" />}
          {repositoryPath !== undefined && <MoreActionsMenu name="Reviews" menu={<ReviewsMenu repositoryPath={repositoryPath} repository={repository} />} />}
        </div>
      </div>
      {/* under the header, not under the field: the header keeps its height */}
      {linkError !== null && (
        <p id="reviews-link-error" className="px-1 text-xs text-destructive" data-testid="reviews-link-error">
          {linkError}
        </p>
      )}
      <ReviewsState availability={availability} empty={empty && !loading && !filtering} repository={repository} />
      {filtering && shown.length === 0 && (
        <NoMatch
          text={filterText}
          onAdd={reference !== null && linkError === null ? addLink : null}
          searchUrl={isCut && repository !== null && reference === null ? reviewsSearchUrl(repository, filterText) : null}
          onClear={() => setFilterText("")}
        />
      )}
      {repositoryPath !== undefined && (shownRequests.length > 0 || shownReturned.length > 0) && (
        <CollapsibleCard
          data-testid="reviews-section-waiting"
          open={isOpen("waiting")}
          onOpenChange={open => setOpen("waiting", open)}
          header={
            <SectionHeader
              icon={<UserCheckIcon className="size-4 shrink-0 text-muted-foreground" />}
              title="Waiting for You"
              count={filtering ? `${shownRequests.length + shownReturned.length} of ${requests.length + returned.length}` : requests.length + returned.length}
            />
          }
        >
          <YourTurnBody requests={shownRequests} returned={shownReturned} repositoryPath={repositoryPath} filterText={filterText} />
        </CollapsibleCard>
      )}
      {repositoryPath !== undefined &&
        shownSections.map(({ section, entries }) => (
          <ReviewSectionCard
            key={section.kind}
            section={section}
            entries={entries}
            filterText={filtering ? filterText : ""}
            open={isOpen(section.kind)}
            onOpenChange={open => setOpen(section.kind, open)}
            repositoryPath={repositoryPath}
            repository={repository}
          />
        ))}
    </div>
  )
}

/** A row of the view by its pull request, and what Enter on it does */
interface ShownRow {
  readonly number: number
  readonly url: string
  readonly open: () => void
}

/** The rows of the view in its order: the requests by name, the returned own pull requests, the entries of the sections */
function rowsOf(repositoryPath: string, requests: readonly ReviewRequest[], returned: readonly ReturnedPullRequest[], entries: readonly ReviewEntry[]): ShownRow[] {
  return [
    ...requests.map(request => ({ number: request.number, url: request.url, open: () => openReviewRequest(repositoryPath, request.number) })),
    ...returned.map(item => ({ number: item.pullRequest.number, url: item.pullRequest.url, open: () => requestReveal({ branchName: item.branchName, commitIds: [] }) })),
    ...entries.map(entry => ({ number: entry.request.number, url: entry.request.url, open: () => openEntry(repositoryPath, entry) })),
  ]
}

/** Enter on a pull request of a section: an own one of a branch of the table shows the branch, as Your Turn's returned rows do; the rest open the review */
function openEntry(repositoryPath: string, entry: ReviewEntry): void {
  const { involvement } = entry
  if (involvement.kind === "authored" && involvement.branch !== null) {
    requestReveal({ branchName: involvement.branch, commitIds: [] })
  } else {
    openReviewRequest(repositoryPath, entry.request.number, null, "reviews")
  }
}

function withMember(set: ReadonlySet<string>, key: string, present: boolean): ReadonlySet<string> {
  if (set.has(key) === present) {
    return set
  }
  const next = new Set(set)
  if (present) {
    next.add(key)
  } else {
    next.delete(key)
  }
  return next
}

/** The title of a section with its count ("1 of 63" while filtering), open or collapsed, as every island keeps its count */
function SectionHeader({ icon, title, count }: { icon: ReactNode; title: string; count: number | string }) {
  return (
    <div className="flex min-w-0 items-center gap-2">
      {icon}
      <h2 className="shrink-0 text-sm font-semibold text-foreground">{title}</h2>
      <Badge variant="secondary">{count}</Badge>
    </div>
  )
}

/**
 * "Filter, or paste a link or #number": the search of the view (`/` focuses it), which also takes a link; while Rust
 * reads one, read-only with the focus kept, and its refusal under the header (`reviews-link-error`). The palette's "Add Pull Request by Link…" focuses
 * it with its text selected, so a paste replaces the filter
 */
function ReviewsFilterField({
  value,
  onValueChange,
  onEnter,
  onLeave,
  onArrowDown,
  busy,
  error,
}: {
  value: string
  onValueChange: (value: string) => void
  onEnter: () => void
  onLeave: () => void
  onArrowDown: () => void
  busy: boolean
  error: string | null
}) {
  const inputRef = useRef<HTMLInputElement>(null)
  // the palette's "Add Pull Request by Link…": once the view shows, a frame later
  useEffect(
    () =>
      onLinkFieldFocus(() =>
        requestAnimationFrame(() => {
          inputRef.current?.focus()
          inputRef.current?.select()
        }),
      ),
    [],
  )
  return (
    <div className="flex min-w-48 flex-1 sm:max-w-96">
      <SearchField
        inputRef={inputRef}
        value={value}
        onValueChange={onValueChange}
        label="Filter, or paste a link or #number"
        onLeave={onLeave}
        onArrowDown={onArrowDown}
        onEnter={onEnter}
        busy={busy}
        invalid={error !== null}
        {...(error !== null && { describedBy: "reviews-link-error" })}
        className="w-full"
        data-testid="reviews-filter-input"
      />
    </div>
  )
}

/** What each section holds and in which order, what the marks say, what the filter sees */
function ReviewsHelpPopover() {
  return (
    <HelpPopover title="Reviews" data-testid="reviews-help">
      <p>Every open pull request of this repository you take part in on GitHub, each once, in the first section that has it.</p>
      <HelpSection title="Sections">
        <HelpList>
          <li>Waiting for You: the requests by name and your pull requests a reviewer handed back, as Your Turn has them.</li>
          <li>Added by Link: the pull requests you added, the newest first.</li>
          <li>Requested from Your Teams: the longest waiting first.</li>
          <li>Reviewed by You and Your Pull Requests: your moves first, then the latest update.</li>
        </HelpList>
      </HelpSection>
      <HelpSection title="Marks">
        <HelpList>
          <li>
            <span className="text-primary">●</span> Your move: new commits since a review GitHub no longer counts, a link not reviewed yet, your pull request with changes
            requested.
          </li>
          <li>Your pull request of a branch: the node of its row, where the branch is on its way into master.</li>
          <li>Muted: your approval, your request for changes, your comment. Its size and the reviewers by name are in the tooltip of the meta.</li>
        </HelpList>
      </HelpSection>
      <p>A section shows your moves and three more rows; the filter looks at every row GitHub returned, the first 50 of each section.</p>
    </HelpPopover>
  )
}

/**
 * The line of a filter that left the view empty: what found nothing; Add Pull Request when the text names one (Enter
 * does it too); Search on GitHub while GitHub returned only a part of a list; Clear Filter
 */
function NoMatch({ text, onAdd, searchUrl, onClear }: { text: string; onAdd: (() => void) | null; searchUrl: string | null; onClear: () => void }) {
  return (
    <div className="flex flex-wrap items-center gap-3 px-1 text-sm text-muted-foreground" data-testid="reviews-filter-empty">
      <span className="min-w-0 truncate">{formatReviewsNoMatch(text)}</span>
      {onAdd !== null && (
        <Button variant="link" size="xs" className="h-auto px-0" onClick={onAdd}>
          Add Pull Request
          <Kbd>Enter</Kbd>
        </Button>
      )}
      {searchUrl !== null && (
        <Button variant="link" size="xs" className="h-auto px-0" onClick={() => openPullRequestUrl(searchUrl)}>
          Search on GitHub
        </Button>
      )}
      <Button variant="link" size="xs" className="h-auto px-0" onClick={onClear}>
        Clear Filter
      </Button>
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

/**
 * A section of Reviews: the user's moves and three more rows, then "Show N More", and "50 of 63 · Open All on GitHub"
 * when GitHub has more; filtered, every match (`entries`) and "First 50 of 63 filtered · Search on GitHub" with the
 * typed words
 */
function ReviewSectionCard({
  section,
  entries,
  filterText,
  open,
  onOpenChange,
  repositoryPath,
  repository,
}: {
  section: ReviewSection
  entries: readonly ReviewEntry[]
  filterText: string
  open: boolean
  onOpenChange: (open: boolean) => void
  repositoryPath: string
  repository: { owner: string; name: string } | null
}) {
  const [focused, setFocused] = useState<number | null>(null)
  const [expanded, setExpanded] = useState(false)
  const filtering = filterText !== ""
  // the user's moves and three more, as Your Turn shows its rows; filtered, every match
  const moves = new Set(section.moves)
  const visible = expanded || filtering ? entries : pickVisibleEntries(entries, moves)
  const hidden = entries.length - visible.length
  const tabStop = visible.some(entry => entry.request.number === focused) ? focused : visible[0]?.request.number
  const allOnGitHub = repository === null ? null : reviewSectionSearchUrl(section.kind, repository)
  // the words of the filter search GitHub; a number or a link names one pull request, which Enter adds
  const search = repository === null || typedReference(filterText) !== null ? null : reviewSectionSearchUrl(section.kind, repository, filterText)
  const isCut = section.totalCount > section.entries.length
  const entryOf = (number: number) => entries.find(entry => entry.request.number === number)
  const handleKeyDown = (event: KeyboardEvent<HTMLUListElement>) =>
    handleReviewRowKeys(
      event,
      visible.map(entry => entry.request.number),
      {
        onEnter: number => {
          const entry = entryOf(number)
          if (entry !== undefined) {
            openEntry(repositoryPath, entry)
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
      open={open}
      onOpenChange={onOpenChange}
      header={
        <SectionHeader
          icon={<GitPullRequestIcon className="size-4 shrink-0 text-muted-foreground" />}
          title={REVIEW_SECTION_TITLES[section.kind]}
          count={filtering ? `${entries.length} of ${section.totalCount}` : section.totalCount}
        />
      }
    >
      <div className="flex flex-col divide-y divide-border">
        <ul aria-label={REVIEW_SECTION_TITLES[section.kind]} className="flex flex-col divide-y divide-border" onKeyDown={handleKeyDown}>
          {visible.map(entry => (
            <ReviewEntryRow
              key={entry.request.number}
              entry={entry}
              move={moves.has(entry.request.number)}
              repositoryPath={repositoryPath}
              tabStop={entry.request.number === tabStop}
            />
          ))}
        </ul>
        {(hidden > 0 || expanded || isCut) && (
          <div className="flex items-center gap-3 px-2 py-1 sm:px-4">
            {!filtering && (hidden > 0 || expanded) && (
              <Button variant="ghost" size="xs" className="text-muted-foreground" data-testid="reviews-show-more" onClick={() => setExpanded(!expanded)}>
                {expanded ? "Show Less" : `Show ${hidden} More`}
              </Button>
            )}
            {isCut &&
              (filtering ? (
                <FilteredOnGitHub count={section.entries.length} totalCount={section.totalCount} url={search} />
              ) : (
                allOnGitHub !== null && (
                  <span className="flex items-center gap-2 text-xs text-muted-foreground" data-testid="reviews-more-on-github">
                    {`${entries.length} of ${section.totalCount}`}
                    <Button variant="link" size="xs" className="h-auto px-0" onClick={() => openPullRequestUrl(allOnGitHub)}>
                      Open All on GitHub
                    </Button>
                  </span>
                )
              ))}
          </div>
        )}
      </div>
    </CollapsibleCard>
  )
}
