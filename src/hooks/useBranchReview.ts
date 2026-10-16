import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import type { KeyboardEvent as ReactKeyboardEvent } from "react"
import type { BranchReviewSegment } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { focusRange, keepSegmentRange, segmentRangeSubject } from "@/lib/branch-review"
import type { BranchReviewFocus, SegmentRange } from "@/lib/branch-review"
import type { DiffSubject } from "@/lib/commit-files"
import { diffSubjectId } from "@/lib/commit-files"
import { ALL_CHANGES, isSingleCommit, keepCommit, rangePosition, segmentItems } from "@/lib/range-review"
import type { RangeReviewItem } from "@/lib/range-review"
import type { filterPaths } from "@/lib/review-owners"
import type { ReviewFilter } from "@/lib/review-owners"
import type { ReviewPosition } from "@/lib/review-position"
import { formatReviewPosition } from "@/lib/review-position"
import { dropFilesOf, useFilesOfCommit } from "@/stores/commit-files"
import { dropPatchesOf } from "@/stores/commit-patch"
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
  readonly branchPrefix: string
  readonly baselineBranch: string
  /** The reviewed branch last, its stack parents before it, root first */
  readonly chain: readonly string[]
  /** The reviewer groups the code-owner check asks for, when it ran */
  readonly groups: readonly BranchReviewGroup[]
}

/** What the review knows of the repository: the heads the last sync built (a branch not there has left) */
export interface BranchReviewSync {
  /** The heads are known (the main window shows the repository) */
  readonly known: boolean
  readonly tips: Readonly<Record<string, string>>
}

interface ReviewView {
  segments: BranchReviewSegment[]
  /** The shown part of the chain */
  range: SegmentRange
  /** One commit of the part; null for all its changes */
  commitId: string | null
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
    }
  }
  return {
    segments,
    range: keepSegmentRange(previous.segments, segments, previous.range),
    commitId: keepCommit(previous.segments, segments, previous.commitId),
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
  } | null
}

/** The state of a review of a branch: read, read again when a sync moved a tip of it, its range dropped at the end */
export function useBranchReview(source: BranchReviewSource, focus: BranchReviewFocus, sync: BranchReviewSync): BranchReviewModel {
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

  const { repositoryPath, branchPrefix, baselineBranch, chain } = source
  const chainKey = chain.join("\n")
  // the focus of the first read only: a later read keeps what the user chose
  const initialFocus = useRef(focus)
  useEffect(() => {
    let current = true
    void commands.getBranchReview({ repositoryPath, branchPrefix, baselineBranch, chain: chainKey.split("\n") }).then(result => {
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
  }, [repositoryPath, branchPrefix, baselineBranch, chainKey, readTips])

  const view = state.status === "loaded" ? state.view : null
  const setView = (change: Partial<ReviewView>) => setState(previous => (previous.status === "loaded" ? { status: "loaded", view: { ...previous.view, ...change } } : previous))
  const rangeSubject = view === null ? null : segmentRangeSubject(view.segments, view.range)
  const subject: DiffSubject | null = view?.commitId == null ? rangeSubject : { kind: "commit", id: view.commitId }
  const files = useFilesOfCommit(subject === null ? "" : diffSubjectId(subject))?.files
  const paths = useMemo(() => files?.map(file => file.path) ?? [], [files])
  const owners = useFileOwners(repositoryPath, baselineBranch, paths)

  // a range is read for this review only: drop it once another one shows, and at the end of the review
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

  let loaded: BranchReviewModel["loaded"] = null
  if (view !== null && subject !== null) {
    const items = segmentItems(view.segments.slice(view.range.from, view.range.to + 1))
    const selected = view.commitId ?? ALL_CHANGES
    loaded = {
      segments: view.segments,
      range: view.range,
      items,
      selected,
      subject,
      files,
      owners,
      target: view.segments[view.range.to]!.name,
      position: formatReviewPosition({ file: position, ...rangePosition(items, selected) }),
    }
  }
  return {
    source,
    state,
    gone,
    filter,
    setFilter,
    setRange: range => setView({ range, commitId: null }),
    select: key => setView({ commitId: key === ALL_CHANGES ? null : key }),
    showPosition,
    lastPath,
    loaded,
  }
}

/** Where the review is now, for the same review elsewhere (moved into its window) */
export function currentFocus(model: BranchReviewModel): BranchReviewFocus {
  const loaded = model.loaded
  return {
    ...(loaded !== null && { range: { from: loaded.segments[loaded.range.from]!.name, to: loaded.segments[loaded.range.to]!.name } }),
    filter: model.filter,
    commitId: loaded === null || loaded.selected === ALL_CHANGES ? null : loaded.selected,
    path: model.lastPath,
    groups: [...model.source.groups],
  }
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
  if (event.defaultPrevented || event.altKey || event.metaKey || event.ctrlKey || event.shiftKey) {
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
