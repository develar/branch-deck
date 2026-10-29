import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import type { KeyboardEvent as ReactKeyboardEvent } from "react"
import { useReviewPlan } from "@/hooks/useReviewPlan"
import type { ReviewPlanState } from "@/hooks/useReviewPlan"
import type { BranchReviewSegment, ReviewSubject } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { focusRange, keepSegmentRange, moveView, segmentRangeSubject } from "@/lib/branch-review"
import type { BranchReviewFocus, SegmentRange } from "@/lib/branch-review"
import type { DiffSubject } from "@/lib/commit-files"
import { diffSubjectId } from "@/lib/commit-files"
import { isTypingTarget } from "@/lib/preview-keys"
import { ALL_CHANGES, SINCE_REVIEW, isSingleCommit, keepCommit, localSinceReview, rangePosition, segmentItems, shownInterdiff } from "@/lib/range-review"
import type { RangeReviewItem } from "@/lib/range-review"
import { filterPaths } from "@/lib/review-owners"
import type { ReviewFilter } from "@/lib/review-owners"
import { markBranchReviewed } from "@/lib/review-plan"
import type { ReviewPosition } from "@/lib/review-position"
import { formatReviewPosition } from "@/lib/review-position"
import { readBranchReview } from "@/lib/review-window-reads"
import { showToast } from "@/lib/toast-message"
import { formatMarkedReviewed } from "@/presenters/review-plan"
import { dropFilesOf, useFilesOfCommit } from "@/stores/commit-files"
import { dropFileDiffsOf } from "@/stores/file-diffs"
import { useAppSettings } from "@/stores/settings/appSettings"
import type { BranchReviewGroup } from "@/types/sub-window"

/*
 * The review of a branch (spec/branch/branch-review.spec.md), in the dialog of the branch preview and in its window: its
 * stack from the baseline (a click picks the part, Shift+click extends it), the commits of the part beside the diffs
 * (`RangeReview`), the files filtered by a reviewer group of the code-owner check or an owner by the `OWNERSHIP` files.
 * A sync that moves a tip of the stack reads it again, keeping the part, the commit and the file. The parts it renders
 * are in `@/components/review/BranchReview`.
 */

/** What the review reads: the chain of the branch in its repository */
export interface BranchReviewSource {
  readonly repositoryPath: string
  readonly baselineBranch: string
  /** The reviewed branch last, its stack parents before it, root first */
  readonly chain: readonly string[]
  /** The reviewer groups the code-owner check asks for, when it ran */
  readonly groups: readonly BranchReviewGroup[]
  /**
   * The whole stack of the root `chain` names alone: its stack branch `{prefix}/stack/<root>`, one part of every commit
   * of the stack (`GetBranchReviewParams.wholeStack`)
   */
  readonly wholeStack?: boolean
  /**
   * The review plan is on for the repository (`branchdeck.reviewExperimental`, spec/branch/review-plan.spec.md): Mark
   * Reviewed, "Since Your Review", the API & Risk card and the dependency order of the files; off, the review is as
   * it is without them
   */
  readonly reviewExperimental?: boolean
}

/** What the review knows of the repository: the heads the last sync built (a branch not there has left) */
export interface BranchReviewSync {
  /** The heads are known: the window's session has a table a sync grouped */
  readonly known: boolean
  readonly tips: Readonly<Record<string, string>>
}

interface ReviewView {
  segments: BranchReviewSegment[]
  /** The shown part of the chain */
  range: SegmentRange
  /** One commit of the part; null for all its changes */
  commitId: string | null
  /** "Since Your Review": the column lists only the commits added or rewritten since the review of the branch */
  since: boolean
}

type ReviewState = { status: "loading" } | { status: "loaded"; view: ReviewView } | { status: "error"; message: string }

/** The view after the segments were read: the first one shows the part asked for, a later one keeps the part shown */
function nextView(previous: ReviewView | null, segments: BranchReviewSegment[], focus: BranchReviewFocus): ReviewView {
  if (previous === null) {
    const commitId = focus.commitId ?? null
    return {
      segments,
      range: focusRange(segments, focus.range),
      commitId: commitId !== null && segments.some(segment => segment.commits.some(it => it.id === commitId)) ? commitId : null,
      since: false,
    }
  }
  return {
    segments,
    range: keepSegmentRange(previous.segments, segments, previous.range),
    commitId: keepCommit(previous.segments, segments, previous.commitId),
    since: previous.since,
  }
}

export interface BranchReviewModel {
  readonly source: BranchReviewSource
  readonly state: ReviewState
  /** The branches of the chain the last sync no longer built: they landed or were renamed */
  readonly gone: readonly string[]
  readonly filter: ReviewFilter
  readonly setFilter: (filter: ReviewFilter) => void
  readonly setRange: (range: SegmentRange) => void
  readonly select: (key: string) => void
  readonly showPosition: (position: ReviewPosition) => void
  /** The file on screen, where a review read again starts */
  readonly lastPath: string | null
  /** Once loaded: the column, the shown subject, the files and their owners */
  readonly loaded: {
    readonly segments: readonly BranchReviewSegment[]
    readonly range: SegmentRange
    readonly items: readonly RangeReviewItem[]
    readonly selected: string
    readonly subject: DiffSubject
    readonly files: Parameters<typeof filterPaths>[1] | undefined
    readonly owners: ReadonlyMap<string, string>
    /** The branch the actions act on: the reviewed end of the part (for the whole stack, the reviewed branch) */
    readonly target: string
    /** "Commit 1 / 3 · File 1 / 2" */
    readonly position: string
    /**
     * The review plan of the part, with the flag on; null with it off. Its interdiff only when the part is one branch:
     * the reviewed pair is on the base of that branch alone
     */
    readonly plan: ReviewPlanState | null
    /** "Since Your Review" is shown: the card heads a commit of it too, with what its rewrite changed */
    readonly sinceShown: boolean
    /** The paths in the order the files show, with Order: Dependencies and a plan; null for the tree order */
    readonly fileOrder: readonly string[] | null
    /** Mark Reviewed of the target, with the flag on, of a branch (not of the whole stack); null without it */
    readonly markReviewed: (() => void) | null
  } | null
}

/**
 * A move to a commit of the chain on screen (D on a commit row of one of its branches), by the open that asked for it
 * (`openedAt`); applied once the review is read and while `held` is false (a dialog of the window is open)
 */
export interface BranchReviewMove {
  readonly openedAt: number
  readonly focus: BranchReviewFocus | null
  readonly held: boolean
}

/** The state of a review of a branch: read, read again when a sync moved a tip of it, its range dropped at the end */
export function useBranchReview(source: BranchReviewSource, focus: BranchReviewFocus, sync: BranchReviewSync, move?: BranchReviewMove): BranchReviewModel {
  const [state, setState] = useState<ReviewState>({ status: "loading" })
  const [filter, setFilter] = useState<ReviewFilter>(focus.filter ?? { kind: "all" })
  // the file on screen: a review read again (a sync moved a tip, another commit, another filter) starts at it
  const [position, setPosition] = useState<ReviewPosition | null>(null)
  const [lastPath, setLastPath] = useState(focus.path ?? null)
  const showPosition = useCallback((next: ReviewPosition) => {
    setPosition(next)
    if (next.path !== null) {
      setLastPath(next.path)
    }
  }, [])

  const gone = sync.known ? source.chain.filter(name => sync.tips[name] === undefined) : []
  // a sync that moved a tip of the stack reads it again, once per set of heads (a branch that left keeps the last view)
  const tipsKey = JSON.stringify(sync.tips)
  const [readTips, setReadTips] = useState(tipsKey)
  const moved = state.status === "loaded" && sync.known && gone.length === 0 && state.view.segments.some(segment => sync.tips[segment.name] !== segment.head)
  if (moved && readTips !== tipsKey) {
    setReadTips(tipsKey)
  }

  const { repositoryPath, baselineBranch, chain } = source
  const wholeStack = source.wholeStack === true
  const chainKey = chain.join("\n")
  // the focus of the first read only: a later read keeps what the user chose
  const initialFocus = useRef(focus)
  useEffect(() => {
    let current = true
    void readBranchReview({ repositoryPath, baselineBranch, chain: chainKey.split("\n"), wholeStack }).then(result => {
      if (!current) {
        return
      }
      if (result.status === "error") {
        setState({ status: "error", message: result.error.message })
      } else {
        setState(previous => ({ status: "loaded", view: nextView(previous.status === "loaded" ? previous.view : null, result.data.segments, initialFocus.current) }))
      }
    })
    return () => {
      current = false
    }
  }, [repositoryPath, baselineBranch, chainKey, wholeStack, readTips])

  // a move to a commit of the chain: once the review is read, not under an open dialog
  const [movedAt, setMovedAt] = useState(move?.openedAt)
  // the filter of the files stays unless it hides every file of the commit moved to; checked once its files are read
  const [checksFilter, setChecksFilter] = useState(false)
  if (move !== undefined && move.focus !== null && move.openedAt !== movedAt && state.status === "loaded" && !move.held) {
    setMovedAt(move.openedAt)
    setState({ status: "loaded", view: moveView(state.view, move.focus) })
    if (move.focus.path != null) {
      setLastPath(move.focus.path)
    }
    setChecksFilter(filter.kind !== "all")
  }

  const view = state.status === "loaded" ? state.view : null
  const setView = (change: Partial<ReviewView>) => setState(previous => (previous.status === "loaded" ? { status: "loaded", view: { ...previous.view, ...change } } : previous))
  const rangeSubject = view === null ? null : segmentRangeSubject(view.segments, view.range)

  // the review plan of the part (spec/branch/review-plan.spec.md): a branch on the base of its part, the whole stack as
  // its range; "Since Your Review" only of one branch, whose base is the one Mark Reviewed keeps
  const experimental = source.reviewExperimental === true
  const [planGeneration, setPlanGeneration] = useState(0)
  const fileOrderSetting = useAppSettings(state => state.reviewFileOrder)
  const planPart = experimental && view !== null ? reviewPlanPart(view.segments, view.range, wholeStack) : null
  const plan = useReviewPlan(
    planPart === null || rangeSubject?.kind !== "range"
      ? null
      : { repositoryPath, subject: planPart.subject, sinceReviewed: planPart.ownBranch && view!.since, head: rangeSubject.head, generation: planGeneration },
  )
  const interdiff = shownInterdiff(plan.plan, planPart?.ownBranch === true, rangeSubject?.kind === "range" ? rangeSubject.head : null)
  const sinceReview = localSinceReview(interdiff)
  const sinceShown = view?.since === true && sinceReview !== null
  // the shown item: the commit chosen, else the scope; "Since Your Review" of commits that are not one range shows the
  // newest of them, the column lists only them
  const sinceKey = sinceReview === null ? ALL_CHANGES : sinceReview.scope === null ? (sinceReview.newestFirst[0] ?? ALL_CHANGES) : SINCE_REVIEW
  const selected = view?.commitId ?? (sinceShown ? sinceKey : ALL_CHANGES)
  let subject: DiffSubject | null = rangeSubject
  if (selected === SINCE_REVIEW && sinceReview?.scope != null) {
    subject = { kind: "range", base: sinceReview.scope.base, head: sinceReview.scope.head }
  } else if (selected !== ALL_CHANGES && selected !== SINCE_REVIEW) {
    subject = { kind: "commit", id: selected }
  }
  const files = useFilesOfCommit(subject === null ? "" : diffSubjectId(subject))?.files
  const paths = useMemo(() => files?.map(file => file.path) ?? [], [files])
  const owners = useFileOwners(repositoryPath, baselineBranch, paths)
  if (checksFilter && files !== undefined) {
    setChecksFilter(false)
    if (filterPaths(filter, files, source.groups, owners)?.size === 0) {
      setFilter({ kind: "all" })
    }
  }

  // a range is read for this review only: drop it once another one shows, and at the end of the review
  const rangeId = rangeSubject === null ? null : diffSubjectId(rangeSubject)
  useEffect(() => {
    if (rangeId === null) {
      return
    }
    return () => {
      dropFilesOf([rangeId])
      dropFileDiffsOf([rangeId])
    }
  }, [rangeId])

  // the range of "Since Your Review" is read for this review only, as the range of the part
  const sinceId = subject?.kind === "range" && subject !== rangeSubject ? diffSubjectId(subject) : null
  useEffect(() => {
    if (sinceId === null) {
      return
    }
    return () => {
      dropFilesOf([sinceId])
      dropFileDiffsOf([sinceId])
    }
  }, [sinceId])

  let loaded: BranchReviewModel["loaded"] = null
  if (view !== null && subject !== null) {
    const items = segmentItems(view.segments.slice(view.range.from, view.range.to + 1), sinceReview, sinceShown)
    const target = view.segments[view.range.to]!
    loaded = {
      segments: view.segments,
      range: view.range,
      items,
      selected,
      subject,
      files,
      owners,
      target: target.name,
      position: formatReviewPosition({ file: position, ...rangePosition(items, selected) }),
      plan: experimental ? (plan.plan === null || plan.plan.interdiff === interdiff ? plan : { ...plan, plan: { ...plan.plan, interdiff } }) : null,
      sinceShown,
      fileOrder: experimental && fileOrderSetting === "dependencies" ? (plan.plan?.order ?? null) : null,
      markReviewed:
        planPart?.markable === true
          ? () =>
              void markBranchReviewed(repositoryPath, target.name, target.base).then(marked => {
                if (marked !== null) {
                  showToast({ title: formatMarkedReviewed(target.name, marked.changed) })
                  setPlanGeneration(generation => generation + 1)
                  setView({ since: false, commitId: view.since ? null : view.commitId })
                }
              })
          : null,
    }
  }
  return {
    source,
    state,
    gone,
    filter,
    setFilter,
    setRange: range => setView({ range, commitId: null, since: false }),
    select: key => setView(key === ALL_CHANGES ? { commitId: null, since: false } : key === SINCE_REVIEW ? { commitId: null, since: true } : { commitId: key }),
    showPosition,
    lastPath,
    loaded,
  }
}

/**
 * What the plan of the part of the review is read of: the reviewed branch on the base of the part (its own branch only
 * when the part is that branch alone, the base Mark Reviewed keeps), the whole stack as its range, never marked
 */
function reviewPlanPart(
  segments: readonly BranchReviewSegment[],
  range: SegmentRange,
  wholeStack: boolean,
): { subject: ReviewSubject; ownBranch: boolean; markable: boolean } | null {
  const first = segments[range.from]
  const last = segments[range.to]
  if (first === undefined || last === undefined) {
    return null
  }
  if (wholeStack) {
    return { subject: { kind: "range", base: first.base, head: last.head }, ownBranch: false, markable: false }
  }
  return { subject: { kind: "branch", name: last.name, base: first.base }, ownBranch: range.from === range.to, markable: true }
}

/** ⌥↑/⌥↓ step through the commits of the part (it has a column) */
export function hasCommitKeys(model: BranchReviewModel): boolean {
  return model.loaded !== null && !isSingleCommit(model.loaded.items)
}

/** An action on the branch in the review: the items of its menu (`BranchReviewItem`, a `MenuItem` of the main window) */
export interface BranchReviewAction {
  readonly id: string
  readonly label: string
  readonly kbds?: readonly string[]
  readonly disabled?: boolean
  readonly description?: string
}

/** The action whose single key the event is (O, S); the physical key, in any keyboard layout */
export function keyedAction<T extends BranchReviewAction>(actions: readonly T[], event: KeyboardEvent | ReactKeyboardEvent): T | undefined {
  // a letter typed into the filter of the files is no action
  if (event.defaultPrevented || event.altKey || event.metaKey || event.ctrlKey || event.shiftKey || isTypingTarget(event)) {
    return undefined
  }
  return actions.find(it => it.kbds?.length === 1 && event.code === `Key${it.kbds[0]!.toUpperCase()}`)
}

/** The owners of paths by the `OWNERSHIP` files of the baseline, read once per path for the review */
function useFileOwners(repositoryPath: string, baselineBranch: string, paths: readonly string[]): ReadonlyMap<string, string> {
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
    void commands.getFileOwners({ repositoryPath, baselineBranch, paths: missing }).then(result => {
      if (result.status === "ok" && result.data.length > 0) {
        setOwners(previous => new Map([...previous, ...result.data.map(owner => [owner.path, owner.owner] as const)]))
      }
    })
  }, [repositoryPath, baselineBranch, paths])
  return owners
}
