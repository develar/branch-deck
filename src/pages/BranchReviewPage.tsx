import { useEffect, useEffectEvent, useMemo, useRef, useState } from "react"
import type { ReactNode } from "react"
import { emit } from "@tauri-apps/api/event"
import { ShieldAlertIcon, UploadIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { Kbd } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { ReviewKeyHints } from "@/components/commit/CommitPreviewDialog"
import { CommitReview } from "@/components/commit/CommitReview"
import { BranchReviewStackBar } from "@/components/review/BranchReviewStackBar"
import { Callout } from "@/components/shared/Callout"
import { PreviewDialogFooter } from "@/components/shared/PreviewDialogFrame"
import { useSubWindowData } from "@/hooks/useSubWindowData"
import { useTauriListen } from "@/hooks/useTauriListen"
import type { BranchReviewCommit, BranchReviewSegment } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { BRANCH_ACTION, BRANCH_REVIEW_ACTIONS_CHANGED, BRANCH_TIPS_CHANGED, keepSegmentRange, segmentRangeSubject } from "@/lib/branch-review"
import type { BranchActionRequest, BranchReviewActionsChanged, BranchTipsChanged, SegmentRange } from "@/lib/branch-review"
import { formatCommitCount } from "@/lib/commit"
import type { DiffSubject } from "@/lib/commit-files"
import { diffSubjectId } from "@/lib/commit-files"
import { notifyError } from "@/lib/errors"
import { formatShortHash } from "@/lib/hash"
import { SAFE_PUSH_DESCRIPTION } from "@/lib/patronus"
import { openPullRequestUrl } from "@/lib/pull-request"
import { filterEmptyText, filterPaths, reviewFilterChips, sameFilter } from "@/lib/review-owners"
import type { ReviewFilter, ReviewFilterChip } from "@/lib/review-owners"
import { dropFilesOf, useFilesOfCommit } from "@/stores/commit-files"
import { dropPatchesOf } from "@/stores/commit-patch"
import type { BranchReviewActions, BranchReviewWindowData } from "@/types/sub-window"

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
  // a sync that moved a tip of the chain reads it again
  const [generation, setGeneration] = useState(0)
  const [filter, setFilter] = useState<ReviewFilter>(data.group === null ? { kind: "all" } : { kind: "group", name: data.group })
  const [position, setPosition] = useState("")
  // what the main window says the window can do with each branch of the chain
  const [actions, setActions] = useState(data.actions)

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
  }, [data, generation])

  // the main window tells when a sync moved the tips of the branches
  useTauriListen<BranchTipsChanged>(BRANCH_TIPS_CHANGED, (payload) => {
    if (payload.repositoryPath === data.repositoryPath && state.status === "loaded" && state.view.segments.some(segment => payload.tips[segment.name] !== segment.head)) {
      setGeneration(it => it + 1)
    }
  })
  useTauriListen<BranchReviewActionsChanged>(BRANCH_REVIEW_ACTIONS_CHANGED, (payload) => {
    if (payload.repositoryPath === data.repositoryPath) {
      setActions(payload.branches)
    }
  })

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
        </div>
      </Card>
      <Card className="min-h-0 flex-1 flex-row gap-0 overflow-hidden py-0">
        <CommitList segments={selectedSegments} commitId={commitId} onSelect={id => setView({ commitId: id })} />
        <div className="min-w-0 flex-1">
          <CommitReview
            subject={subject}
            initialPath={data.path}
            onPositionChange={setPosition}
            pathFilter={files === undefined ? null : filterPaths(filter, files, data.groups, owners)}
            filteredEmptyText={filterEmptyText(filter)}
            fileOwners={owners}
            rangeNoun="branch"
          />
        </div>
      </Card>
      <ReviewFooter position={position} repositoryPath={data.repositoryPath} branchName={target} actions={actions[target]} />
    </Page>
  )
}

interface ReviewFooterProps {
  position: string
  repositoryPath: string
  /** The branch the actions act on */
  branchName: string
  /** Undefined while the main window shows another repository or lacks the branch */
  actions: BranchReviewActions | undefined
}

/**
 * The footer below the islands, on the canvas: the position ("File 2 / 5"), the keys of the review, Escape that closes
 * the window, then the actions on the branch: O opens its pull request, S asks the main window to open its Safe Push
 * form, "Push" to push it, without a key (P is the previous change). The window doesn't own the branches: the main
 * window runs what it asks for (`branch-action`) and says why it can't.
 */
function ReviewFooter({ position, repositoryPath, branchName, actions }: ReviewFooterProps) {
  const pullRequest = actions?.pullRequest ?? null
  const safePush = actions?.safePush ?? null
  const push = actions?.push ?? null
  const ask = (action: BranchActionRequest["action"]) => {
    const request: BranchActionRequest = { repositoryPath, branchName, action }
    emit(BRANCH_ACTION, request).catch((error: unknown) => notifyError(action === "push" ? `Cannot push ${branchName}` : `Cannot start the Safe Push of ${branchName}`, error))
  }

  const handleKey = useEffectEvent((event: KeyboardEvent) => {
    if (event.defaultPrevented || event.altKey || event.metaKey || event.ctrlKey || event.shiftKey) {
      return
    }
    // the physical keys, as in the branch preview
    if (event.code === "KeyO" && pullRequest !== null) {
      event.preventDefault()
      openPullRequestUrl(pullRequest.url)
    }
    else if (event.code === "KeyS" && safePush !== null && safePush.reason === null) {
      event.preventDefault()
      ask("safe-push")
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
        <ReviewKeyHints count={1} escape="Close" />
        {pullRequest !== null && (
          <Button variant="ghost" size="xs" data-testid="branch-review-open-pull-request" onClick={() => openPullRequestUrl(pullRequest.url)}>
            <Kbd>O</Kbd>
            Open Pull Request
          </Button>
        )}
        {safePush !== null && (
          <FooterAction testId="branch-review-safe-push" reason={safePush.reason} description={SAFE_PUSH_DESCRIPTION} onClick={() => ask("safe-push")}>
            <Kbd>S</Kbd>
            Safe Push…
          </FooterAction>
        )}
        {push !== null && (
          <FooterAction testId="branch-review-push" reason={push.reason} description={`Pushes ${branchName} from the main window`} onClick={() => ask("push")}>
            <UploadIcon data-icon="inline-start" />
            Push
          </FooterAction>
        )}
      </PreviewDialogFooter>
    </div>
  )
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

function CommitRow({ commit, selected, onSelect }: { commit: BranchReviewCommit, selected: boolean, onSelect: (id: string) => void }) {
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
