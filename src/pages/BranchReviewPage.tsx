import { useCallback, useEffect, useEffectEvent, useMemo, useRef, useState } from "react"
import type { ReactNode } from "react"
import { GitPullRequestCreateIcon, ShieldAlertIcon, UploadIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { Kbd } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { CommitReview } from "@/components/commit/CommitReview"
import { BranchReviewStackBar } from "@/components/review/BranchReviewStackBar"
import { ReviewKeyHints } from "@/components/review/ReviewKeyHints"
import { Callout } from "@/components/shared/Callout"
import { PreviewDialogFooter } from "@/components/shared/PreviewDialogFrame"
import { askMainWindow, useLinkedFeedback, useLinkedState } from "@/hooks/useMainWindowLink"
import { useSubWindowData } from "@/hooks/useSubWindowData"
import type { BranchReviewSegment, ReviewCommit } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { BRANCH_REVIEW_WINDOW, branchGoneReason, keepSegmentRange, segmentRangeSubject } from "@/lib/branch-review"
import type { SegmentRange } from "@/lib/branch-review"
import { formatCommitCount } from "@/lib/commit"
import type { DiffSubject } from "@/lib/commit-files"
import { diffSubjectId } from "@/lib/commit-files"
import { notifyError } from "@/lib/errors"
import { formatShortHash } from "@/lib/hash"
import type { ReviewPosition } from "@/lib/review-position"
import { formatReviewPosition } from "@/lib/review-position"
import { openPullRequestUrl } from "@/lib/pull-request"
import { filterEmptyText, filterPaths, reviewFilterChips, sameFilter } from "@/lib/review-owners"
import type { ReviewFilter, ReviewFilterChip } from "@/lib/review-owners"
import { dropFilesOf, useFilesOfCommit } from "@/stores/commit-files"
import { dropPatchesOf } from "@/stores/commit-patch"
import { otherRepositoryReason } from "@/stores/review-requests"
import type { BranchReviewFeedback, BranchReviewItem, BranchReviewRequest, BranchReviewWindowData } from "@/types/sub-window"

interface ReviewView {
  segments: BranchReviewSegment[]
  /** The shown part of the chain */
  range: SegmentRange
  /** One commit of the part; null for all its changes */
  commitId: string | null
}

type ReviewState
  = | { status: "loading" }
    | { status: "loaded", view: ReviewView }
    | { status: "error", message: string }

/** The view after the segments were read: the first one shows the part asked for, a later one keeps the part shown */
function nextView(previous: ReviewView | null, segments: BranchReviewSegment[], initialRange: BranchReviewWindowData["range"]): ReviewView {
  if (previous === null) {
    const last = segments.length - 1
    return { segments, range: initialRange === "all" ? { from: 0, to: last } : { from: last, to: last }, commitId: null }
  }
  const { commitId } = previous
  return {
    segments,
    range: keepSegmentRange(previous.segments, segments, previous.range),
    commitId: commitId !== null && segments.some(segment => segment.commits.some(commit => commit.id === commitId)) ? commitId : null,
  }
}

/**
 * The review window of a branch: its stack from the baseline (a click picks the part, Shift+click extends it), the
 * commits of the part ("All Changes" or one commit), the files filtered by a reviewer group of the code-owner check or
 * an owner by the `OWNERSHIP` files, and the diffs of the commit review. A sync that moves a tip of the stack reads it
 * again, keeping the part. The footer has the keys and the actions on the branch of the part (`ReviewFooter`).
 */
export function BranchReviewPage() {
  const data = useSubWindowData<BranchReviewWindowData>()
  if (data === null) {
    return null
  }
  // a new open (another branch, or the same one from another place) starts over
  return <BranchReview key={data.openedAt} data={data} />
}

function BranchReview({ data }: { data: BranchReviewWindowData }) {
  const reviewed = data.chain.at(-1)!
  const [state, setState] = useState<ReviewState>({ status: "loading" })
  const [filter, setFilter] = useState<ReviewFilter>(data.group === null ? { kind: "all" } : { kind: "group", name: data.group })
  // the file on screen: a review read again (a sync moved a tip, another commit, another filter) starts at it
  const [position, setPosition] = useState<ReviewPosition | null>(null)
  const [lastPath, setLastPath] = useState(data.path)
  const showPosition = useCallback((next: ReviewPosition) => {
    setPosition(next)
    if (next.path !== null) {
      setLastPath(next.path)
    }
  }, [])
  // what the main window says of the chain: the heads a sync built, the actions of each branch
  const linked = useLinkedState(BRANCH_REVIEW_WINDOW, data.state)
  // the actions of each branch while the main window showed the repository and had it: they stay (those that need the
  // main window disabled) while it shows another one, and for a branch that left
  const [seenItems, setSeenItems] = useState(data.state.items)
  const [knownItems, setKnownItems] = useState(data.state.items)
  if (linked.selected && linked.items !== seenItems) {
    setSeenItems(linked.items)
    setKnownItems({ ...knownItems, ...linked.items })
  }
  // the branches of the chain the last sync no longer built: they landed or were renamed
  const gone = linked.selected ? data.chain.filter(name => linked.tips[name] === undefined) : []
  useLinkedFeedback<BranchReviewFeedback>(BRANCH_REVIEW_WINDOW, feedback => notifyError(feedback.title, feedback.message))

  // a sync that moved a tip of the stack reads it again, once per set of heads (a branch that left keeps the last view)
  const tipsKey = JSON.stringify(linked.tips)
  const [readTips, setReadTips] = useState(tipsKey)
  const moved = state.status === "loaded" && linked.selected && gone.length === 0 && state.view.segments.some(segment => linked.tips[segment.name] !== segment.head)
  if (moved && readTips !== tipsKey) {
    setReadTips(tipsKey)
  }

  useEffect(() => {
    let current = true
    void commands.getBranchReview({ repositoryPath: data.repositoryPath, branchPrefix: data.branchPrefix, baselineBranch: data.baselineBranch, chain: data.chain }).then((result) => {
      if (!current) {
        return
      }
      if (result.status === "error") {
        setState({ status: "error", message: result.error.message })
      }
      else {
        setState(previous => ({ status: "loaded", view: nextView(previous.status === "loaded" ? previous.view : null, result.data.segments, data.range) }))
      }
    })
    return () => {
      current = false
    }
  }, [data, readTips])

  const view = state.status === "loaded" ? state.view : null
  const setView = (change: Partial<ReviewView>) => setState(previous => previous.status === "loaded" ? { status: "loaded", view: { ...previous.view, ...change } } : previous)
  const segments = view?.segments ?? null
  const range = view?.range ?? null
  const commitId = view?.commitId ?? null
  const rangeSubject = segments === null || range === null ? null : segmentRangeSubject(segments, range)
  const subject: DiffSubject | null = commitId === null ? rangeSubject : { kind: "commit", id: commitId }
  const subjectId = subject === null ? null : diffSubjectId(subject)
  const files = useFilesOfCommit(subjectId ?? "")?.files
  const paths = useMemo(() => files?.map(file => file.path) ?? [], [files])
  const owners = useFileOwners(data, paths)

  // a range is read for this window only: drop it once another one shows, and when the window closes
  const rangeId = rangeSubject === null ? null : diffSubjectId(rangeSubject)
  useEffect(() => {
    if (rangeId === null) {
      return
    }
    return () => {
      dropFilesOf([rangeId])
      dropPatchesOf([rangeId])
    }
  }, [rangeId])

  if (state.status === "error") {
    return (
      <Page>
        <Callout tone="destructive" title={`Cannot read the changes of ${reviewed}`}>{state.message}</Callout>
      </Page>
    )
  }
  if (segments === null || range === null || subject === null || subjectId === null) {
    return (
      <Page>
        <p className="flex items-center gap-2 text-sm text-muted-foreground">
          <Spinner className="size-4" />
          Loading changes…
        </p>
      </Page>
    )
  }
  const selectedSegments = segments.slice(range.from, range.to + 1)
  // the actions are of the branch at the reviewed end of the part: for the whole stack, the reviewed branch
  const target = segments[range.to]!.name
  return (
    <Page>
      <Card className="shrink-0 gap-2 py-2">
        <div className="flex flex-col gap-2 px-3">
          <BranchReviewStackBar
            baselineBranch={data.baselineBranch}
            segments={segments}
            range={range}
            onRangeChange={next => setView({ range: next, commitId: null })}
          />
          {files !== undefined && <OwnerFilter chips={reviewFilterChips(files, data.groups, owners)} filter={filter} onFilterChange={setFilter} />}
          {gone.length > 0 && <p className="text-xs text-muted-foreground" data-testid="branch-review-gone">{gone.map(branchGoneReason).join("; ")}</p>}
        </div>
      </Card>
      <Card className="min-h-0 flex-1 flex-row gap-0 overflow-hidden py-0">
        <CommitList segments={selectedSegments} commitId={commitId} onSelect={id => setView({ commitId: id })} />
        <div className="min-w-0 flex-1">
          <CommitReview
            subject={subject}
            initialPath={lastPath}
            onPositionChange={showPosition}
            pathFilter={files === undefined ? null : filterPaths(filter, files, data.groups, owners)}
            filteredEmptyText={filterEmptyText(filter)}
            fileOwners={owners}
            rangeNoun="branch"
          />
        </div>
      </Card>
      <ReviewFooter
        position={formatReviewPosition({ file: position })}
        repositoryPath={data.repositoryPath}
        branchName={target}
        items={shownItems(knownItems[target] ?? [], { repositoryPath: data.repositoryPath, branchName: target, selected: linked.selected, gone: gone.includes(target) })}
      />
    </Page>
  )
}

/**
 * The actions of the footer as the main window last sent them: those it runs are disabled with the reason while it shows
 * another repository or the branch left; a pull request opens from here anyway
 */
function shownItems(items: readonly BranchReviewItem[], { repositoryPath, branchName, selected, gone }: { repositoryPath: string, branchName: string, selected: boolean, gone: boolean }): readonly BranchReviewItem[] {
  if (selected && !gone) {
    return items
  }
  return items.map((item) => {
    if (item.url !== undefined) {
      return item
    }
    const reason = gone ? branchGoneReason(branchName) : otherRepositoryReason(repositoryPath, item.id === "push" ? `push ${branchName}` : `start the Safe Push of ${branchName}`)
    return { ...item, disabled: true, description: reason }
  })
}

interface ReviewFooterProps {
  position: string
  repositoryPath: string
  /** The branch the actions act on */
  branchName: string
  items: readonly BranchReviewItem[]
}

/**
 * The footer below the islands, on the canvas: the position ("File 2 / 5"), the keys of the review, Escape that closes
 * the window, then the actions on the branch, the items of its menu: O opens its pull request (or Create Pull Request
 * the form of one), S asks the main window to open its Safe Push form, "Push" to push it, without a key (P is the
 * previous change). The window doesn't own the branches: the main window runs what it asks for and says why it can't.
 */
function ReviewFooter({ position, repositoryPath, branchName, items }: ReviewFooterProps) {
  const ask = (itemId: BranchReviewRequest["itemId"]) => void askMainWindow<BranchReviewRequest>(BRANCH_REVIEW_WINDOW, { repositoryPath, branchName, itemId })
  const run = (item: BranchReviewItem) => {
    if (item.url !== undefined) {
      openPullRequestUrl(item.url)
    }
    else if (item.id === "safe-push" || item.id === "push") {
      ask(item.id)
    }
  }

  const handleKey = useEffectEvent((event: KeyboardEvent) => {
    if (event.defaultPrevented || event.altKey || event.metaKey || event.ctrlKey || event.shiftKey) {
      return
    }
    // the physical keys, as in the branch preview: the key an item names
    const item = items.find(it => it.kbds?.length === 1 && event.code === `Key${it.kbds[0]!.toUpperCase()}`)
    if (item !== undefined) {
      event.preventDefault()
      if (item.disabled !== true) {
        run(item)
      }
    }
  })
  // the whole window: the focus may be on the page itself
  useEffect(() => {
    const listener = (event: KeyboardEvent) => handleKey(event)
    window.addEventListener("keydown", listener)
    return () => window.removeEventListener("keydown", listener)
  }, [])

  return (
    <div className="shrink-0" data-testid="branch-review-footer">
      <PreviewDialogFooter className="border-t-0 px-1 py-0" start={<span className="tabular-nums" data-testid="branch-review-position">{position}</span>}>
        <ReviewKeyHints commits={false} escape="Close" />
        {items.map(item => (
          <FooterAction
            key={item.id}
            testId={`branch-review-${item.id}`}
            reason={item.disabled === true ? (item.description ?? null) : null}
            description={item.description ?? (item.id === "push" ? `Pushes ${branchName} from the main window` : item.label)}
            onClick={() => run(item)}
          >
            <FooterActionIcon item={item} />
            {item.label}
          </FooterAction>
        ))}
      </PreviewDialogFooter>
    </div>
  )
}

/** The key of an item (`O`, `S`), else its icon */
function FooterActionIcon({ item }: { item: BranchReviewItem }) {
  const key = item.kbds?.length === 1 ? item.kbds[0]! : null
  if (key !== null) {
    return <Kbd>{key.toUpperCase()}</Kbd>
  }
  return item.id === "create-pull-request" ? <GitPullRequestCreateIcon data-icon="inline-start" /> : <UploadIcon data-icon="inline-start" />
}

/** A ghost `xs` button of the footer, disabled with the reason on hover, its description otherwise */
function FooterAction({ testId, reason, description, onClick, children }: { testId: string, reason: string | null, description: string, onClick: () => void, children: ReactNode }) {
  const disabled = reason !== null
  return (
    <Tooltip>
      <TooltipTrigger
        render={(
          <Button
            variant="ghost"
            size="xs"
            disabled={disabled}
            // the hover says why; Base UI keeps the button focusable, so the dimming is ours
            focusableWhenDisabled
            className={cn(disabled && "opacity-50")}
            data-testid={testId}
            onClick={onClick}
          />
        )}
      >
        {children}
      </TooltipTrigger>
      <TooltipContent>{reason ?? description}</TooltipContent>
    </Tooltip>
  )
}

function Page({ children }: { children: ReactNode }) {
  return <div data-testid="branch-review" className="flex h-dvh flex-col gap-2 bg-canvas p-2 text-sm">{children}</div>
}

/** The owners of paths by the `OWNERSHIP` files of the baseline, read once per path for the window */
function useFileOwners(data: BranchReviewWindowData, paths: readonly string[]): ReadonlyMap<string, string> {
  const [owners, setOwners] = useState<ReadonlyMap<string, string>>(new Map())
  const asked = useRef(new Set<string>())
  useEffect(() => {
    const missing = paths.filter(path => !asked.current.has(path))
    if (missing.length === 0) {
      return
    }
    for (const path of missing) {
      asked.current.add(path)
    }
    void commands.getFileOwners({ repositoryPath: data.repositoryPath, baselineBranch: data.baselineBranch, paths: missing }).then((result) => {
      if (result.status === "ok" && result.data.length > 0) {
        setOwners(previous => new Map([...previous, ...result.data.map(owner => [owner.path, owner.owner] as const)]))
      }
    })
  }, [data, paths])
  return owners
}

function OwnerFilter({ chips, filter, onFilterChange }: { chips: readonly ReviewFilterChip[], filter: ReviewFilter, onFilterChange: (filter: ReviewFilter) => void }) {
  // "All" alone filters nothing
  if (chips.length === 1) {
    return null
  }
  return (
    <div className="flex flex-wrap items-center gap-1.5" data-testid="branch-review-owners" role="group" aria-label="Files of">
      {chips.map(chip => <FilterChip key={chip.label} chip={chip} pressed={sameFilter(chip.filter, filter)} onPress={() => onFilterChange(chip.filter)} />)}
    </div>
  )
}

function FilterChip({ chip, pressed, onPress }: { chip: ReviewFilterChip, pressed: boolean, onPress: () => void }) {
  const button = (
    <Button
      size="xs"
      variant="outline"
      aria-pressed={pressed}
      disabled={chip.count === null}
      data-testid="branch-review-owner"
      data-owner={chip.label}
      className={cn(pressed && "border-primary/50 bg-primary/10 text-primary hover:bg-primary/15 hover:text-primary")}
      onClick={onPress}
    >
      {chip.needsApproval && <ShieldAlertIcon data-icon="inline-start" className="text-warning" />}
      {chip.label}
      {chip.count !== null && <span className="text-muted-foreground tabular-nums">{chip.count}</span>}
    </Button>
  )
  if (!chip.needsApproval) {
    return button
  }
  return (
    <Tooltip>
      {/* a span: a disabled button gets no pointer events */}
      <TooltipTrigger render={<span />}>{button}</TooltipTrigger>
      <TooltipContent>{chip.count === null ? "Needs approval; Patronus doesn't tell which files" : "Needs approval for a Safe Push"}</TooltipContent>
    </Tooltip>
  )
}

/** "All Changes" and the commits of the shown part, newest first, under the name of their branch when there are several */
function CommitList({ segments, commitId, onSelect }: { segments: readonly BranchReviewSegment[], commitId: string | null, onSelect: (id: string | null) => void }) {
  const count = segments.reduce((sum, segment) => sum + segment.commits.length, 0)
  return (
    <nav aria-label="Commits" className="flex w-64 shrink-0 flex-col gap-0.5 overflow-y-auto border-r border-border p-2" data-testid="branch-review-commits">
      <CommitButton selected={commitId === null} onClick={() => onSelect(null)}>
        <span className="flex-1 truncate font-medium">All Changes</span>
        <span className="text-xs text-muted-foreground">{formatCommitCount(count)}</span>
      </CommitButton>
      {[...segments].reverse().map(segment => (
        <div key={segment.name} className="flex flex-col gap-0.5">
          {segments.length > 1 && <p className="truncate px-2 pt-2 text-xs text-muted-foreground">{segment.name}</p>}
          {segment.commits.map(commit => <CommitRow key={commit.id} commit={commit} selected={commit.id === commitId} onSelect={onSelect} />)}
        </div>
      ))}
    </nav>
  )
}

function CommitRow({ commit, selected, onSelect }: { commit: ReviewCommit, selected: boolean, onSelect: (id: string) => void }) {
  return (
    <CommitButton selected={selected} title={commit.subject} onClick={() => onSelect(commit.id)}>
      <span className="flex-1 truncate">{commit.subject}</span>
      <span className="font-mono text-xs text-muted-foreground">{formatShortHash(commit.id)}</span>
    </CommitButton>
  )
}

function CommitButton({ selected, title, onClick, children }: { selected: boolean, title?: string, onClick: () => void, children: ReactNode }) {
  return (
    <button
      type="button"
      aria-pressed={selected}
      title={title}
      data-testid="branch-review-commit"
      className={cn("flex h-7 w-full items-center gap-2 rounded-md px-2 text-left outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring/50", selected && "bg-primary/15 hover:bg-primary/15")}
      onClick={onClick}
    >
      {children}
    </button>
  )
}
