import { useCallback, useEffect, useEffectEvent, useMemo, useRef, useState } from "react"
import type { KeyboardEvent, ReactNode } from "react"
import { cn } from "cn"
import { useDefaultLayout } from "react-resizable-panels"
import { Button } from "@/components/ui/button"
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "@/components/ui/resizable"
import { CommitReviewCode, PendingFileHeaders } from "@/components/diff/CommitReviewCode"
import type { CommitReviewCodeHandle, NextSection } from "@/components/diff/CommitReviewCode"
import type { DiffSection, DiffViewFile } from "@/components/diff/DiffView"
import { FileTree } from "@/components/files/FileTree"
import type { FileTreeBadges, FileTreeCommitRows, FileTreeMarks, FileTreeProgress } from "@/components/files/FileTree"
import { ReviewTitleText } from "@/components/review/ReviewTitleText"
import { Callout } from "@/components/shared/Callout"
import { LoadingLine } from "@/components/shared/LoadingLine"
import { SearchField } from "@/components/shared/SearchField"
import { SectionLabel } from "@/components/shared/SectionLabel"
import { HeadingCardContext, useHeadingCardState } from "@/hooks/useHeadingCard"
import { SelectReviewFileContext, followLink } from "@/hooks/useSelectReviewFile"
import type { ReviewLink, ReviewNavigator } from "@/hooks/useSelectReviewFile"
import { StoryHighlightContext } from "@/hooks/useStoryHighlight"
import type { StoryHighlight } from "@/hooks/useStoryHighlight"
import type { CommitFile, DeclarationTarget, DiffRevisions, MechanicalKind } from "@/lib/bindings"
import type { DiffSubject } from "@/lib/commit-files"
import { diffSubjectId, parseDiffSubjectId } from "@/lib/commit-files"
import type { FileDiff } from "@/lib/diff/file-diff-model"
import { getHighlightClient } from "@/lib/diff/highlight/highlighter"
import { PROLOGUE_KEY } from "@/lib/diff/layout"
import type { LayoutPart } from "@/lib/diff/layout"
import type { HeadingPart, SectionUnits } from "@/lib/diff/rows"
import { withEngineCounts } from "@/lib/diff/view-files"
import { FILE_FILTER_MIN_FILES, filterFilesByText, formatNoFileMatch } from "@/lib/file-filter"
import {
  buildFileTree,
  buildReviewTree,
  concernRowKey,
  dependencyRows,
  flattenTreeFiles,
  foldedGroups,
  getInitiallyCollapsed,
  groupRowKey,
  hasReviewGroups,
  listsCommits,
  mechanicalKindsOf,
  revealInTree,
  sectionRows,
  toggleInSet,
} from "@/lib/file-tree"
import type { FileTreeCommit, FileTreeNode, ReviewCommitItem } from "@/lib/file-tree"
import type { MenuItemGroups } from "@/lib/menu"
import { isTypingTarget } from "@/lib/preview-keys"
import { formatRangeWithoutChanges } from "@/lib/range-review"
import { reviewKeyAction } from "@/lib/review-keys"
import type { ReviewPlace } from "@/lib/review-place"
import { readOutsideExcerpt, readOutsidePlace } from "@/lib/review-plan"
import type { ReviewPosition } from "@/lib/review-position"
import { markWindowContentReady, markSubWindowOpenPhase } from "@/lib/sub-window"
import { presentDependencyBadge, presentFileBadges } from "@/presenters/review-plan"
import type { DependencyBadge, ReviewFacts } from "@/presenters/review-plan"
import { formatSeen, formatStepCount, formatStepDivider, formatStepNumber, placeCommits, presentChapters, presentGroupCounts, presentOwnerMarks } from "@/presenters/review-steps"
import type { Chapter } from "@/presenters/review-steps"
import { presentGroupOpenings, presentOpenings, storyMarks } from "@/presenters/review-story"
import type { StoryMark } from "@/presenters/review-story"
import { presentNarrative, presentStepBar } from "@/presenters/review-units"
import type { NarrativeSection, StepBarView } from "@/presenters/review-units"
import { getFilesOfCommit, loadFilesOfSubject, useFilesOfCommit } from "@/stores/commit-files"
import { loadFileDiffs, useFileDiffs, whenFileDiffsEnded } from "@/stores/file-diffs"
import { getDiffRepositoryPath } from "@/stores/repository"
import { markCardsSeen, useSeenPaths } from "@/stores/review-seen"
import { setReviewSidebar, useAppSettings } from "@/stores/settings/appSettings"
import { useDiffViewMode, useDiffWhitespacePolicy } from "@/stores/settings/diff-view-settings"

// the highlight worker starts with the page of a review, beside the reads of its files and diffs, not after them
getHighlightClient()

/** The files a review shows (the search of its sidebar applied) and all of them */
export interface FilesCount {
  readonly shown: number
  readonly total: number
}

/**
 * The files of the chosen owner chip (openspec/specs/branch/branch-review/spec.md, "Code owners"): they filter the tree and diffs;
 * empty (the owner has no files here) does not filter
 */
export interface OwnerMarks {
  /** The name of the chip: "Design" */
  readonly label: string
  readonly paths: ReadonlySet<string>
}

/**
 * The commits of a range shown whole with its plan (openspec/specs/branch/review-tour/spec.md, "In the Review Window"): the tree
 * lists them in the steps they tell or follow up, above the files
 */
export interface CommitRows {
  /** The commits of the range, as its column lists them */
  readonly items: readonly ReviewCommitItem[]
  /** Shows the commit of the key alone: Enter on its row */
  readonly onShow: (key: string) => void
  /** The right-click menu of a commit row: Show Commit Alone, then the menu of the commit */
  readonly menu: (item: ReviewCommitItem) => MenuItemGroups
}

export interface CommitReviewProps {
  /** A commit, or the range of a pull request */
  subject: DiffSubject
  /** The file shown first; the first file of the tree by default */
  initialPath: string | null
  /**
   * A link of a document beside the review (the Overview tab of a pull request) that opened it: followed once, as its
   * link in the review would be, as soon as the review is read (the file at its line, the use outside, the opening of
   * the step)
   */
  initialLink?: ReviewLink | null
  /** The selected file, for the footer ("File 3 / 12") and for a review that starts again at it */
  onPositionChange: (position: ReviewPosition) => void
  /** The files of the chosen owner chip: the tree and diffs show only these; none without */
  ownerMarks?: OwnerMarks | null
  /** Clears the chosen owner chip before revealing a file it hides */
  onClearOwnerFilter?: () => void
  /** The code owner of a file by its path, in the header of its diff */
  fileOwners?: ReadonlyMap<string, string>
  /** What the range is, in the error: "Cannot read the changes of the pull request" */
  rangeNoun?: string
  /**
   * Above the files in the sidebar: the commits of a range (`RangeReview`), its scopes while the tree lists the commits
   * (`treeListsCommits`: a search that keeps no group of a step, or no tree, gives them back to the column)
   */
  sidebarTop?: (treeListsCommits: boolean) => ReactNode
  /** The commits the tree lists in the steps of the plan; none without */
  commitRows?: CommitRows | null
  /**
   * The header of the files in the sidebar, of the number of files shown and of all of them (null while none show):
   * "Files 3 of 9" by default, the owner chips of a branch review
   */
  filesHeader?: (count: FilesCount | null) => ReactNode
  /**
   * The first block of the document of the review, before the first section: the overview of the plan, the commit a
   * range shows alone; its links select a changed file with `useSelectReviewFile`. Above the headers and the messages
   * while the diffs are read
   */
  prologue?: ReactNode
  /** What ⌥↑/⌥↓ show next: read once the subject is, so the step is instant */
  neighbours?: readonly DiffSubject[]
  /**
   * The facts of the review plan of the subject: the badges of the rows, the mechanical files folded into one group at
   * the end of the tree; the plain tree without them
   */
  reviewFacts?: ReviewFacts | null
  /**
   * A scope of a range with commits (`RangeReview`): when it changes no file, the review says that its commits cancel
   * each other out and offers its first commit, instead of "No changed files"; none for a commit
   */
  emptyRange?: EmptyRange | null
}

/** The commits of a shown scope of a range, for its review when the scope changes no file */
export interface EmptyRange {
  readonly commitCount: number
  /** Shows the oldest commit of the scope alone */
  readonly onShowFirstCommit: () => void
}

/**
 * The review of a commit or of the range of a pull request (`base..head`): a sidebar (the commits of a range, the
 * header of the files, their directory tree) beside the diffs of all of them in one virtualized view. The split is
 * there in every state, so what the sidebar holds keeps its focus while another commit or filter loads.
 * The tree follows the diffs as they scroll, a file selected in the tree scrolls the diffs to it. J and K step through
 * the files, N and P through the blocks of changed lines. From `FILE_FILTER_MIN_FILES` files on the sidebar has a
 * search of the files ("/"): the tree and the diffs show the files with every word in their path, also of the next
 * commit (⌥↑/⌥↓). An owner chip (`ownerMarks`) filters the tree and diffs to its files and steers the same keys.
 */
export function CommitReview({
  subject,
  initialPath,
  initialLink = null,
  onPositionChange,
  ownerMarks = null,
  onClearOwnerFilter,
  fileOwners,
  rangeNoun = "pull request",
  sidebarTop,
  commitRows = null,
  filesHeader,
  prologue,
  neighbours,
  reviewFacts = null,
  emptyRange = null,
}: CommitReviewProps) {
  const subjectId = diffSubjectId(subject)
  const commitFiles = useFilesOfCommit(subjectId)
  const policy = useDiffWhitespacePolicy()
  const entry = useFileDiffs(subject, policy)
  const [filesFailed, setFilesFailed] = useState<string | null>(null)
  const [filterText, setFilterText] = useState("")
  const allFiles = commitFiles?.files
  const ownerPaths = ownerMarks === null || ownerMarks.paths.size === 0 ? null : ownerMarks.paths
  const ownerFiles = useMemo(
    () =>
      allFiles === undefined || ownerPaths === null ? allFiles : allFiles.filter(file => ownerPaths.has(file.path) || (file.oldPath !== null && ownerPaths.has(file.oldPath))),
    [allFiles, ownerPaths],
  )
  const shownFiles = useMemo(() => (ownerFiles === undefined ? undefined : filterFilesByText(ownerFiles, filterText)), [ownerFiles, filterText])
  // the count of the last subject read: the search stays while the next one loads
  const [fileCount, setFileCount] = useState(0)
  if (allFiles !== undefined && allFiles.length !== fileCount) {
    setFileCount(allFiles.length)
  }
  const isSearchShown = filterText !== "" || fileCount >= FILE_FILTER_MIN_FILES

  // a file a link of a mark selects while the search hides it: the search is cleared, then it is selected once, and the
  // tree reports that (`onRevealApplied`); a subject or search other than those the click left forgets it before
  const [reveal, setReveal] = useState<PendingReveal | null>(null)
  if (reveal !== null && (reveal.subjectId !== subjectId || reveal.filterText !== filterText)) {
    setReveal(null)
  }
  const forgetReveal = useCallback(() => setReveal(null), [])
  const revealHidden = useCallback(
    (place: ReviewPlace) => {
      // a removed declaration names the path before a rename
      const file = commitFiles?.files.find(it => it.path === place.path) ?? commitFiles?.files.find(it => it.oldPath === place.path)
      if (file === undefined) {
        return
      }
      const hiddenByOwner = ownerPaths !== null && !ownerPaths.has(file.path) && (file.oldPath === null || !ownerPaths.has(file.oldPath))
      if (hiddenByOwner) {
        onClearOwnerFilter?.()
      }
      const hiddenBySearch = filterFilesByText([file], filterText).length === 0
      if (hiddenBySearch) {
        setFilterText("")
      }
      setReveal({ subjectId, place: { ...place, path: file.path }, filterText: hiddenBySearch ? "" : filterText })
    },
    [commitFiles, filterText, subjectId, ownerPaths, onClearOwnerFilter],
  )
  const revealPlace = reveal !== null && shownFiles?.some(file => file.path === reveal.place.path) === true ? reveal.place : null
  const revealPath = revealPlace?.path ?? null

  // by the id: the subject object may be new on every render
  useEffect(() => {
    const current = parseDiffSubjectId(subjectId)
    void loadFilesOfSubject(current).then(() => {
      if (getFilesOfCommit(subjectId) === undefined) {
        setFilesFailed(subjectId)
      }
    })
  }, [subjectId])
  // the backend sends the diff of the file the review opens at first; the others follow in the order of the tree
  const priorityPath = initialPath
  useEffect(() => {
    loadFileDiffs(parseDiffSubjectId(subjectId), policy, { priorityPaths: priorityPath === null ? [] : [priorityPath] })
  }, [subjectId, policy, priorityPath])

  const diffsLoaded = entry?.kind === "loaded"
  useEffect(() => {
    if (commitFiles !== undefined) {
      markSubWindowOpenPhase("files")
    }
  }, [commitFiles])
  useEffect(() => {
    if (diffsLoaded) {
      markSubWindowOpenPhase("diffs")
    }
  }, [diffsLoaded])
  // the commits before and after: read once the shown one's diffs are all read, never beside its stream
  const neighbourIds = neighbours?.map(diffSubjectId).join("\n") ?? ""
  useEffect(() => {
    if (neighbourIds === "") {
      return
    }
    return whenFileDiffsEnded(parseDiffSubjectId(subjectId), policy, () => {
      for (const id of neighbourIds.split("\n")) {
        const neighbour = parseDiffSubjectId(id)
        void loadFilesOfSubject(neighbour)
        loadFileDiffs(neighbour, policy)
      }
    })
  }, [subjectId, neighbourIds, policy])

  let message: ReactNode = null
  let loaded: LoadedReview | null = null
  if (filesFailed === subjectId || entry?.kind === "error") {
    message = (
      <div className="p-6">
        <Callout tone="destructive" title={`Cannot read the changes of the ${subject.kind === "commit" ? "commit" : rangeNoun}`}>
          {entry?.kind === "error" ? entry.message : null}
        </Callout>
      </div>
    )
  } else if (shownFiles === undefined) {
    message = (
      <LoadingLine key={subjectId} className="p-6">
        Loading changes…
      </LoadingLine>
    )
  } else if (shownFiles.length === 0 && allFiles !== undefined && allFiles.length > 0) {
    message = (
      <div data-testid="review-files-no-match" className="flex items-center gap-3 p-6 text-sm text-muted-foreground">
        <span className="min-w-0 truncate">{formatNoFileMatch(filterText)}</span>
        <Button variant="link" size="xs" className="h-auto px-0" onClick={() => setFilterText("")}>
          Clear Filter
        </Button>
      </div>
    )
  } else if (shownFiles.length === 0 && emptyRange !== null && emptyRange.commitCount > 0) {
    message = (
      <div data-testid="review-range-empty" className="flex items-center gap-3 p-6 text-sm text-muted-foreground">
        <span className="min-w-0">{formatRangeWithoutChanges(emptyRange.commitCount)}</span>
        <Button variant="link" size="xs" className="h-auto px-0" onClick={emptyRange.onShowFirstCommit}>
          Show First Commit
        </Button>
      </div>
    )
  } else if (shownFiles.length === 0) {
    message = <p className="p-6 text-sm text-muted-foreground">No changed files</p>
  } else {
    const askedShown = initialPath !== null && shownFiles.some(file => file.path === initialPath)
    loaded = {
      // the selection and the collapsed directories belong to one commit or range; an owner chip keeps them
      key: subjectId,
      // the counts of the engine, none until its diffs are read: git's numstat counts what the policy ignores too
      // with the plan a block of the imports alone is played down: no stop of N and P, nothing of it to read
      files: withEngineCounts(shownFiles, entry?.kind === "loaded" ? entry.files : undefined, reviewFacts !== null),
      totalCount: allFiles?.length ?? shownFiles.length,
      diffs: entry?.kind === "loaded" ? entry.files : null,
      diffsComplete: entry?.kind === "loaded" && entry.complete,
      initialPath: revealPath ?? (askedShown ? initialPath : null),
      // a file of the changes an owner chip does not show now: its files may be still read (a new head, whose groups the
      // check tells later), so the first file stands in for it
      awaitedPath: revealPath === null && !askedShown && initialPath !== null && allFiles?.some(file => file.path === initialPath) === true ? initialPath : null,
      revealPath,
      revealPlace,
      // the commits of the sides: a declaration outside the review is read at them
      revisions: subject.kind === "commit" ? { old: `${subject.id}^`, new: subject.id } : { old: subject.base, new: subject.head },
      reviewFacts,
    }
  }
  // an error and an empty subject are the content: the window shows them
  const settledWithoutDiffs = message !== null && shownFiles !== undefined
  useEffect(() => {
    if (settledWithoutDiffs || filesFailed === subjectId) {
      markWindowContentReady()
    }
  })
  return (
    <ReviewLayout
      subjectId={subjectId}
      loaded={loaded}
      message={message}
      onPositionChange={onPositionChange}
      fileOwners={fileOwners}
      ownerMarks={ownerMarks}
      sidebarTop={sidebarTop}
      commitRows={commitRows}
      filesHeader={filesHeader}
      prologue={prologue}
      initialLink={initialLink}
      search={isSearchShown ? { text: filterText, onTextChange: setFilterText } : null}
      onRevealHidden={revealHidden}
      filesRead={commitFiles !== undefined && commitFiles.files.length > 0}
      onRevealApplied={forgetReveal}
    />
  )
}

/**
 * The focusable frame of every state of the review: while another commit loads, has no files or cannot be read, the
 * focus stays here (the diffs that had it are gone), so the keys of the surface (⌥↑/⌥↓, Escape) still reach it, and
 * the keys of the review (`onKeyDown`) are pressed here too: ⌘F while nothing of the subject shows yet
 */
function ReviewRoot({
  subjectId,
  ready,
  onPositionChange,
  onKeyDown,
  children,
}: {
  subjectId: string
  ready: boolean
  onPositionChange: (position: ReviewPosition) => void
  onKeyDown: (event: KeyboardEvent<HTMLDivElement>) => void
  children: ReactNode
}) {
  const root = useRef<HTMLDivElement>(null)
  useEffect(() => {
    // the diffs take the focus when they mount; the diffs of the last subject took it with them to the body, until the
    // next ones mount it is here
    const element = root.current
    const active = document.activeElement
    if (element !== null && (active === null || active === document.body)) {
      element.focus({ preventScroll: true })
    }
  }, [subjectId])
  useEffect(() => {
    if (!ready) {
      onPositionChange({ path: null, index: -1, count: 0 })
    }
  }, [ready, onPositionChange])
  return (
    <div ref={root} tabIndex={-1} data-review-root="" data-testid="commit-review" className="h-full min-h-0 outline-none" onKeyDown={onKeyDown}>
      {children}
    </div>
  )
}

/** A place a link of a mark showed while the search hid its file, at the last path of the file, and the search that shows it */
interface PendingReveal {
  subjectId: string
  place: ReviewPlace
  /** The search after the click: empty when the search hid the file */
  filterText: string
}

/** The files of the shown subject once read, and their diffs once read too: the tree does not wait for the diffs */
interface LoadedReview {
  /** The subject: a new key starts the tree and the diffs over */
  key: string
  /** The files the search kept */
  files: readonly CommitFile[]
  /** The files before the search: "Files 2 of 18" */
  totalCount: number
  /** The files whose diff arrived, in the order of git */
  diffs: readonly FileDiff[] | null
  /** Every diff of the subject arrived: a file without one has none */
  diffsComplete: boolean
  initialPath: string | null
  /**
   * The file the review was asked to open at (`CommitReviewProps.initialPath`, the file on screen of the review before)
   * while the changes have it and the shown files lack it: the first file stands in for it until a filter shows it
   */
  awaitedPath: string | null
  /** The file a link of a mark selected while the search hid it, now shown: the tree selects it */
  revealPath: string | null
  /** Its place, which the diffs show once the tree selected it */
  revealPlace: ReviewPlace | null
  revisions: DiffRevisions
  /** The facts of the review plan: the badges, the Mechanical group */
  reviewFacts: ReviewFacts | null
}

/** The search of the files: the text belongs to the window, not to one subject */
interface FileSearch {
  text: string
  onTextChange: (text: string) => void
}

interface ReviewLayoutProps {
  /** The subject shown or still read: `LoadedReview.key` once its files are read */
  subjectId: string
  loaded: LoadedReview | null
  /** Where the diffs are while there are none: loading, the error, no files */
  message: ReactNode
  onPositionChange: (position: ReviewPosition) => void
  fileOwners: ReadonlyMap<string, string> | undefined
  ownerMarks: OwnerMarks | null
  sidebarTop: CommitReviewProps["sidebarTop"]
  commitRows: CommitRows | null
  filesHeader: CommitReviewProps["filesHeader"]
  prologue: ReactNode
  initialLink: ReviewLink | null
  search: FileSearch | null
  /** A link of a mark shows a place of a changed file that the search hides */
  onRevealHidden: (place: ReviewPlace) => void
  /** The subject has changed files: the card selects one also while the search hides every file */
  filesRead: boolean
  /** The tree selected `LoadedReview.revealPath`: it is not selected again */
  onRevealApplied: () => void
}

/** The tree state of one loaded subject (`LoadedReview.key`) and its active filters */
interface TreeSelection {
  key: string | null
  filterText: string
  ownerFilterKey: string
  /** The chosen owner chip whose files `ownerFilterKey` names (`OwnerMarks.label`): its files may change, it stays */
  chip: string | null
  collapsed: ReadonlySet<string>
  selectedPath: string | null
  /** The row of the selected file when it is not its main row: the file "also in" another concern of a review */
  selectedRow: string | null
  /** The tree has the groups of a review: a plan that comes after the files folds the Mechanical group once */
  grouped: boolean
  /** The file the diffs of this key open at (`LoadedReview.initialPath` at the start of the key) */
  initialPath: string | null
  /** The first file of the tree, selected because nothing else was: a plan that comes after the files moves it */
  defaultPath: string | null
  /**
   * The file the first file stands in for, the file on screen the files of the chip lack now (`LoadedReview.awaitedPath`
   * at the start of the key, or a file the files of the same chip hid): selected once they show it, unless the reader
   * moved from the first file
   */
  awaited: string | null
  /** The awaited file once a filter showed it: the diffs go to it */
  arrived: string | null
  /** The file of a link this selection selected (`LoadedReview.revealPath`) */
  revealed: string | null
}

/** The first file stands in for the awaited one: the reader has not moved from it */
function standsIn(selection: TreeSelection): boolean {
  return selection.awaited !== null && selection.selectedPath !== null && selection.selectedPath === selection.defaultPath
}

/**
 * The selection once the shown files changed (another filter, the plan): the awaited file in place of the first file
 * that stood in for it, once they show it
 */
function withAwaited(selection: TreeSelection, nodes: readonly FileTreeNode[], files: readonly CommitFile[]): TreeSelection {
  if (selection.awaited === null) {
    return selection
  }
  if (!standsIn(selection)) {
    return { ...selection, awaited: null }
  }
  const awaited = selection.awaited
  if (!files.some(file => file.path === awaited)) {
    return selection
  }
  return {
    ...selection,
    selectedPath: awaited,
    selectedRow: null,
    defaultPath: null,
    awaited: null,
    arrived: awaited,
    collapsed: revealInTree(selection.collapsed, awaited, nodes),
  }
}

function initialSelection(
  loaded: LoadedReview | null,
  filterText: string,
  ownerFilterKey: string,
  chip: string | null,
  nodes: readonly FileTreeNode[],
  fileCount: number,
  firstPath: string | null,
): TreeSelection {
  if (loaded === null) {
    return {
      key: null,
      filterText,
      ownerFilterKey,
      chip,
      collapsed: new Set(),
      selectedPath: null,
      selectedRow: null,
      grouped: false,
      initialPath: null,
      defaultPath: null,
      awaited: null,
      arrived: null,
      revealed: null,
    }
  }
  // what a filter found shows whole: its directories open
  const narrowed = filterText.trim() !== "" || ownerFilterKey !== ""
  const initial = narrowed ? new Set<string>() : getInitiallyCollapsed(nodes, fileCount)
  return {
    key: loaded.key,
    filterText,
    ownerFilterKey,
    chip,
    collapsed: loaded.initialPath === null ? initial : revealInTree(initial, loaded.initialPath, nodes),
    selectedPath: loaded.initialPath ?? firstPath,
    selectedRow: null,
    grouped: hasReviewGroups(nodes),
    initialPath: loaded.initialPath,
    defaultPath: loaded.initialPath === null ? firstPath : null,
    awaited: loaded.initialPath === null ? loaded.awaitedPath : null,
    arrived: null,
    revealed: loaded.revealPath,
  }
}

/**
 * Another filter: the directories as a new tree has them, the selected file kept while the filter keeps it; the file a
 * link revealed (`revealPath`) selected, else the awaited file once the filter shows it (`withAwaited`). A file on
 * screen that the files of the same chip hide (the groups of a new head not told yet, the owners read) is awaited, the
 * first file standing in for it; another chip or search the reader chose selects the first file of what it keeps
 */
function filteredSelection(
  previous: TreeSelection,
  filterText: string,
  ownerFilterKey: string,
  chip: string | null,
  nodes: readonly FileTreeNode[],
  files: readonly CommitFile[],
  revealPath: string | null,
): TreeSelection {
  const kept = revealPath ?? previous.selectedPath
  const shown = files.some(file => file.path === kept)
  const selectedPath = shown ? kept : (files[0]?.path ?? null)
  const sameChip = revealPath === null && filterText === previous.filterText && chip === previous.chip
  const awaited = !sameChip ? null : standsIn(previous) ? previous.awaited : shown ? null : kept
  const narrowed = filterText.trim() !== "" || ownerFilterKey !== ""
  const collapsed = narrowed ? new Set<string>() : getInitiallyCollapsed(nodes, files.length)
  return withAwaited(
    {
      ...previous,
      filterText,
      ownerFilterKey,
      chip,
      collapsed: selectedPath === null ? collapsed : revealInTree(collapsed, selectedPath, nodes),
      selectedPath,
      selectedRow: null,
      grouped: hasReviewGroups(nodes),
      // the file that stands in for the awaited one: the reader has not moved from it
      defaultPath: awaited !== null ? selectedPath : previous.defaultPath,
      awaited,
      revealed: revealPath,
    },
    nodes,
    files,
  )
}

/**
 * The plan came, or went: the Mechanical group and the generated files of each concern start collapsed unless they
 * hold the selected file. A file selected only as the first of the tree without the plan, which may be a generated
 * one, gives way to the first of the new tree, or to the awaited file the new tree shows (`withAwaited`)
 */
function groupedSelection(previous: TreeSelection, nodes: readonly FileTreeNode[], files: readonly CommitFile[]): TreeSelection {
  const grouped = hasReviewGroups(nodes)
  if (!grouped) {
    return withAwaited({ ...previous, grouped, selectedRow: null }, nodes, files)
  }
  const firstPath = files[0]?.path ?? null
  const moved = previous.selectedPath !== null && previous.selectedPath === previous.defaultPath
  const selectedPath = moved ? firstPath : previous.selectedPath
  const collapsed = new Set([...previous.collapsed, ...foldedGroups(nodes)])
  return withAwaited(
    {
      ...previous,
      grouped,
      selectedPath,
      selectedRow: null,
      defaultPath: moved ? firstPath : previous.defaultPath,
      collapsed: selectedPath === null ? collapsed : revealInTree(collapsed, selectedPath, nodes),
    },
    nodes,
    files,
  )
}

/** The concern of the selected tree row, or the main row of the selected path */
function activeConcernGroup(nodes: readonly FileTreeNode[], selectedKey: string | null): string | null {
  if (selectedKey === null) {
    return null
  }
  const visit = (items: readonly FileTreeNode[], concernKey: string | null): string | null => {
    for (const node of items) {
      const currentConcern = node.kind === "directory" && node.group === "concern" ? node.key : concernKey
      if (node.key === selectedKey || (node.kind === "file" && node.file.path === selectedKey)) {
        return currentConcern
      }
      if (node.kind === "directory") {
        const found = visit(node.children, currentConcern)
        if (found !== null) {
          return found
        }
      }
    }
    return null
  }
  return visit(nodes, null)
}

const NO_MECHANICAL: ReadonlyMap<string, MechanicalKind> = new Map()
const NO_ROWS = { rowOf: new Map<string, string>(), sectionOf: new Map<string, string>() }

/** A section of the story that shows its file whole */
const NO_UNITS: SectionUnits = { marks: new Map(), elsewhere: new Map() }
const NO_STEPS_WITHOUT_CARD: StepsWithoutCard = { before: new Map(), after: [] }

/** The step the first section of a step starts, for the divider above its card: its id, which a click shows, and its words */
function stepOf(section: NarrativeSection, chapterOf: ReadonlyMap<string, Chapter>): DiffSection["step"] {
  const chapter = section.concernHeading === null || section.concern === null ? undefined : chapterOf.get(section.concern)
  return chapter === undefined ? null : { id: chapter.id, number: formatStepNumber(chapter.number), title: chapter.title }
}

/** The steps told without a card of their own (`stepsWithoutCard`) */
interface StepsWithoutCard {
  /** By the key of the section whose opening theirs stand above: the first section of the next step with a card */
  readonly before: ReadonlyMap<string, readonly NonNullable<DiffSection["step"]>[]>
  /** Those after the last step with a card: their openings are the epilogue of the diffs */
  readonly after: readonly NonNullable<DiffSection["step"]>[]
}

/**
 * The steps told without a card of their own (no section: another step's shows all they change, or nothing of them
 * lands), in the order of the story: each above the opening of the next step with a card, else after the last card
 */
function stepsWithoutCard(chapters: readonly Chapter[], narrative: readonly NarrativeSection[]): StepsWithoutCard {
  const starts = new Map<string, string>()
  for (const section of narrative) {
    if (section.ghost === null && section.concern !== null && section.concernHeading !== null && !starts.has(section.concern)) {
      starts.set(section.concern, section.key)
    }
  }
  const shown = new Set(narrative.flatMap(section => (section.ghost === null && section.concern !== null ? [section.concern] : [])))
  const before = new Map<string, NonNullable<DiffSection["step"]>[]>()
  let waiting: NonNullable<DiffSection["step"]>[] = []
  for (const chapter of chapters.filter(it => it.told)) {
    const key = starts.get(chapter.id)
    if (key !== undefined && waiting.length > 0) {
      before.set(key, waiting)
      waiting = []
    } else if (key === undefined && !shown.has(chapter.id)) {
      waiting.push({ id: chapter.id, number: formatStepNumber(chapter.number), title: chapter.title })
    }
  }
  return { before, after: waiting }
}

/** The words of the step every section of a step is of, for the hover of the map of the files; null outside the steps */
function stepHeadingOf(section: NarrativeSection, chapterOf: ReadonlyMap<string, Chapter>): string | null {
  const chapter = section.concern === null ? undefined : chapterOf.get(section.concern)
  return chapter === undefined ? null : formatStepDivider(chapter)
}

/**
 * A use a change left outside (`ghost`), as a section of the story: its file at the head (`target`, unread while
 * undefined) around its line, the lines before and after it as context, a heading before the line; no change, so N
 * and P pass it (an empty block played down as the imports are)
 */
function ghostFile(key: string, path: string, ghost: { line: number; heading: HeadingPart }, target: DeclarationTarget | undefined): DiffViewFile {
  const blobId = target?.outside?.blobId ?? null
  const lineCount = target?.outside?.lineCount ?? 0
  const at = ghost.line - 1
  const file: CommitFile = { path, status: "modified", oldPath: null, additions: 0, deletions: 0, binary: false, oldId: blobId, newId: blobId, submodule: false }
  const units: SectionUnits = { marks: new Map([[0, [ghost.heading]]]), elsewhere: new Map() }
  const section = { key, units, targets: new Map(), heading: "Left outside", step: null, stepHeading: null }
  if (target === undefined || blobId === null) {
    return { file, diff: undefined, pending: true, section }
  }
  const diff: FileDiff = {
    path,
    oldPath: null,
    status: "modified",
    oldId: blobId,
    newId: blobId,
    // a view of one file opened on purpose: it never starts collapsed
    collapse: null,
    // no file of the counts: N and P pass it, as its empty start says
    reading: null,
    content: {
      kind: "text",
      oldLineCount: lineCount,
      newLineCount: lineCount,
      additions: 0,
      deletions: 0,
      lineEnds: null,
      finalNewline: null,
      emptyLastLine: { old: true, new: true },
      lineLevelOnly: false,
      blocks: [{ oldStart: at, oldEnd: at, newStart: at, newEnd: at, kind: "changed", inlinePairs: false, oldRanges: [], newRanges: [] }],
      oldScopes: [],
      newScopes: [],
      importBlocks: [0],
      // nothing to read: no start
      marks: { runs: [], moves: [], renames: [], starts: [] },
    },
  }
  return { file, diff, quietImports: true, section }
}

/**
 * The diffs show `place` of a shown file: its line with its name selected, or its header without one; the key of the
 * section that shows its line, undefined where the header shows instead
 */
function showPlace(code: CommitReviewCodeHandle, place: ReviewPlace): string | undefined {
  const line =
    place.line === null
      ? null
      : {
          path: place.path,
          side: place.side,
          line: place.line - 1,
          column: 0,
          endColumn: 0,
          ...(place.name !== null && { name: place.name }),
          ...(place.unit !== undefined && { unit: place.unit }),
        }
  const shown = line === null ? undefined : code.revealDeclaration(line, null)
  if (shown === undefined) {
    code.scrollToFile(place.path)
  }
  return shown
}

/**
 * The width of the sidebar until its divider is dragged: what the names of a tree need, not a share of the window, so
 * a maximized window gives the diffs the rest. The titles of the steps of a story (subjects of about 100 characters)
 * fit in one line up to about 40 characters and wrap to a second line after; at 19rem a title of 30 already wrapped
 */
const SIDEBAR_WIDTH = "24rem"

/**
 * The sidebar beside the diffs, in every state of the review: the parts of the sidebar (the commits, the header of the
 * files) stay mounted, only the tree and the diffs start over for another subject.
 */
function ReviewLayout({
  subjectId,
  loaded,
  message,
  onPositionChange,
  fileOwners,
  ownerMarks,
  sidebarTop,
  commitRows,
  filesHeader,
  prologue,
  initialLink,
  search,
  onRevealHidden,
  filesRead,
  onRevealApplied,
}: ReviewLayoutProps) {
  const diffViewMode = useDiffViewMode()
  const sidebarShown = useAppSettings(state => state.reviewSidebar)
  const { defaultLayout, onLayoutChanged } = useDefaultLayout({ id: "review-sidebar", storage: localStorage, onlySaveAfterUserInteractions: true })
  const loadedFiles = loaded?.files
  const reviewFacts = loaded?.reviewFacts ?? null
  // the steps of the story of the plan as the tree, each with its files (openspec/specs/branch/review-tour/spec.md), what no evidence
  // joined under Other changes, the mechanical ones in one group at the end, as Rust told, classed and ordered them
  const mechanical = useMemo(() => (reviewFacts === null ? NO_MECHANICAL : mechanicalKindsOf(reviewFacts)), [reviewFacts])
  const chapters = useMemo(() => (reviewFacts === null ? [] : presentChapters(reviewFacts)), [reviewFacts])
  // the commits of a range shown whole, each in the step it tells or follows up
  const commitItems = commitRows?.items
  const commitPlaces = useMemo(
    () => (reviewFacts === null || commitItems === undefined ? undefined : placeCommits(reviewFacts.steps.steps, commitItems, reviewFacts.steps.source.kind)),
    [reviewFacts, commitItems],
  )
  const filterText = search?.text ?? ""
  // what a search or owner chip found: the commits of the groups it keeps
  const searched = filterText.trim() !== ""
  const ownerPaths = ownerMarks === null || ownerMarks.paths.size === 0 ? null : ownerMarks.paths
  const ownerFiltered = ownerPaths !== null
  const ownerFilterKey = ownerPaths === null ? "" : [...ownerPaths].sort().join("\n")
  const narrowed = searched || ownerFiltered
  const nodes = useMemo(() => {
    if (loadedFiles === undefined) {
      return []
    }
    return reviewFacts === null ? buildFileTree(loadedFiles) : buildReviewTree(loadedFiles, mechanical, chapters, commitPlaces, narrowed)
  }, [loadedFiles, reviewFacts, mechanical, chapters, commitPlaces, narrowed])
  const files = useMemo(() => flattenTreeFiles(nodes), [nodes])
  const chip = ownerMarks?.label ?? null
  const [selection, setSelection] = useState<TreeSelection>(() => initialSelection(loaded, filterText, ownerFilterKey, chip, nodes, files.length, files[0]?.path ?? null))
  const loadedKey = loaded?.key ?? null
  if (selection.key !== loadedKey) {
    setSelection(initialSelection(loaded, filterText, ownerFilterKey, chip, nodes, files.length, files[0]?.path ?? null))
  } else if (selection.filterText !== filterText || selection.ownerFilterKey !== ownerFilterKey || selection.chip !== chip) {
    setSelection(filteredSelection(selection, filterText, ownerFilterKey, chip, nodes, files, loaded?.revealPath ?? null))
  } else if (selection.grouped !== hasReviewGroups(nodes)) {
    setSelection(groupedSelection(selection, nodes, files))
  }
  const { collapsed, selectedPath, selectedRow } = selection
  const activeGroupKey = useMemo(() => activeConcernGroup(nodes, selectedRow ?? selectedPath), [nodes, selectedRow, selectedPath])
  // the file of a link is selected once: a later key or search does not select it again
  const revealApplied = loaded !== null && loaded.revealPath !== null && selection.revealed === loaded.revealPath
  const code = useRef<CommitReviewCodeHandle>(null)
  // what the overview, a card or a link asked of the diffs before they mounted (no diff of the subject read yet: the
  // overview alone on the canvas, a pull request's changes still on their way): kept, the last ask alone, and done as
  // they mount, so a place shown early is not lost
  const waitingForCode = useRef<{ readonly key: string | null; readonly action: (code: CommitReviewCodeHandle) => void } | null>(null)
  // ⌘F pressed before the diffs mounted (the tree shows the files, or they are still read): the subject it was pressed
  // at. The find opens as its diffs mount, beside the place asked of them: it is no place, so it replaces none
  const findWaiting = useRef<string | null>(null)
  const withCode = useCallback(
    (action: (code: CommitReviewCodeHandle) => void) => {
      if (code.current === null) {
        waitingForCode.current = { key: loadedKey, action }
      } else {
        waitingForCode.current = null
        action(code.current)
      }
    },
    [loadedKey],
  )
  const revealedPlace = loaded?.revealPlace ?? null
  useEffect(() => {
    if (revealApplied) {
      if (revealedPlace !== null) {
        withCode(view => showPlace(view, revealedPlace))
      }
      onRevealApplied()
    }
  }, [revealApplied, revealedPlace, onRevealApplied, withCode])
  const tree = useRef<HTMLDivElement>(null)

  // the diffs show the selected file of what the search kept
  const scrollToSelected = useEffectEvent(() => {
    if (selectedPath !== null) {
      code.current?.scrollToFile(selectedPath)
    }
  })
  useEffect(() => {
    scrollToSelected()
  }, [filterText])
  // the awaited file, once a filter showed it: the diffs go to it as the tree selected it
  const arrived = selection.arrived
  useEffect(() => {
    if (arrived !== null) {
      code.current?.scrollToFile(arrived)
    }
  }, [arrived])

  // the diffs mount with the models of the engine; each file reads its texts by blob id as it comes into view, and the
  // window shows once the rows of the first screen are painted (`onFirstScreen`)
  const diffs = loaded?.diffs ?? null
  const diffsComplete = loaded?.diffsComplete ?? false
  // with the plan, the story of its units (openspec/specs/branch/review-units/spec.md) in the order of its steps: each run of
  // changes of a file one section, each group a section per file, the mechanical ones collapsed, the uses a change left
  // outside after it
  const narrative = useMemo(() => {
    if (diffs === null || reviewFacts === null) {
      return null
    }
    const byPath = new Map(diffs.map(diff => [diff.path, diff]))
    const shown = files.map(file => {
      const diff = byPath.get(file.path)
      return { path: file.path, blocks: diff?.content.kind === "text" ? diff.content.blocks : null }
    })
    return presentNarrative(reviewFacts.units, shown, reviewFacts.api, reviewFacts.files, chapters)
  }, [diffs, files, reviewFacts, chapters])
  // the row of the tree each section selects, and the section each row shows: a file "also in" a concern its part there
  const rows = useMemo(() => (narrative === null ? NO_ROWS : sectionRows(narrative, nodes)), [narrative, nodes])
  // the file at the head of each use left outside, read once per review
  const revision = loaded?.revisions.new ?? null
  // the files the reader saw at this head, for the progress of the groups of large steps (openspec/specs/branch/review-tour/spec.md,
  // "In the Review Window"): a file shown on purpose, a card scrolled entirely through the diffs; a review of no large
  // step counts nothing
  const seenHead = chapters.some(chapter => chapter.groups.length > 0) ? (revision ?? loadedKey) : null
  const seenPaths = useSeenPaths(seenHead)
  // the sections of each file the story shows in parts: such a file is seen once each of its parts was
  const partsOfPath = useMemo(() => {
    const parts = new Map<string, string[]>()
    for (const section of narrative ?? []) {
      if (section.part !== null) {
        parts.set(section.path, [...(parts.get(section.path) ?? []), section.key])
      }
    }
    return parts
  }, [narrative])
  // cards the reader saw (passed through the view, shown on purpose): the file of each, of a file in parts once all
  // its parts were seen
  const cardsSeen = useCallback((cards: readonly { readonly path: string; readonly key: string }[]) => markCardsSeen(seenHead, cards, partsOfPath), [seenHead, partsOfPath])
  const [outside, setOutside] = useState<ReadonlyMap<string, DeclarationTarget | null>>(() => new Map())
  useEffect(() => {
    const repositoryPath = getDiffRepositoryPath()
    if (narrative === null || revision === null || repositoryPath === undefined) {
      return
    }
    let current = true
    for (const section of narrative) {
      if (section.ghost !== null && !outside.has(section.key)) {
        void readOutsideExcerpt(repositoryPath, revision, section.path, section.ghost.line, section.ghost.name).then(target => {
          if (current) {
            setOutside(previous => new Map(previous).set(section.key, target))
          }
        })
      }
    }
    return () => {
      current = false
    }
  }, [narrative, revision, outside])
  // a step of no card (another step's section shows all it changed, or nothing of it lands) has its opening above the
  // opening of the next step that has a card, else after the last card
  const withoutCard = useMemo(() => (narrative === null ? NO_STEPS_WITHOUT_CARD : stepsWithoutCard(chapters, narrative)), [chapters, narrative])
  const viewFiles = useMemo((): readonly DiffViewFile[] | null => {
    if (diffs === null) {
      return null
    }
    const byPath = new Map(diffs.map(diff => [diff.path, diff]))
    const counted = withEngineCounts(files, diffs)
    if (narrative === null) {
      return counted.map(file => ({ file, diff: byPath.get(file.path), pending: !diffsComplete && !byPath.has(file.path) }))
    }
    const fileOf = new Map(counted.map(file => [file.path, file]))
    const chapterOf = new Map(chapters.map(chapter => [chapter.id, chapter]))
    // the sections the bodies of the story name, each marked in its header
    const marks = storyMarks(reviewFacts!, narrative)
    return narrative.flatMap((section): DiffViewFile[] => {
      if (section.ghost !== null) {
        const target = outside.get(section.key)
        // a use the head lacks shows nothing; one not read yet its header
        return target === null ? [] : [ghostFile(section.key, section.path, section.ghost, target)]
      }
      return [
        {
          file: fileOf.get(section.path)!,
          diff: byPath.get(section.path),
          pending: !diffsComplete && !byPath.has(section.path),
          quietImports: true,
          ...(section.folded !== null && { folded: section.folded }),
          section: {
            key: section.key,
            units: section.units ?? NO_UNITS,
            targets: section.targets,
            heading: section.heading,
            group: section.group,
            step: stepOf(section, chapterOf),
            before: withoutCard.before.get(section.key),
            stepHeading: stepHeadingOf(section, chapterOf),
            story: marks.get(section.key) ?? [],
            unitIds: section.unitIds,
            part: section.part,
          },
        },
      ]
    })
  }, [diffs, diffsComplete, files, narrative, outside, chapters, reviewFacts, withoutCard])
  // the diffs mounted (`viewFiles`, with the first diff read): what was asked of them before, for this subject
  const codeMounted = viewFiles !== null
  useEffect(() => {
    if (!codeMounted || code.current === null) {
      return
    }
    const waiting = waitingForCode.current
    waitingForCode.current = null
    if (waiting !== null && waiting.key === loadedKey) {
      waiting.action(code.current)
    }
    const find = findWaiting.current
    findWaiting.current = null
    if (find !== null && find === loadedKey) {
      code.current.openFind()
    }
  }, [codeMounted, loadedKey])
  // the section whose layer of openings holds the opening of a step: its first, else the one a step of no card stands
  // above; of a step of no card after the last card (the epilogue), the last section
  const openingOfStep = useCallback(
    (step: string): { key: string; path: string } | undefined => {
      if (withoutCard.after.some(it => it.id === step)) {
        const last = viewFiles?.at(-1)
        return last === undefined ? undefined : { key: last.section?.key ?? last.file.path, path: last.file.path }
      }
      const file = viewFiles?.find(it => it.section?.step?.id === step || it.section?.before?.some(before => before.id === step) === true)
      return file?.section == null ? undefined : { key: file.section.key, path: file.file.path }
    },
    [viewFiles, withoutCard],
  )
  const showWindow = useCallback(() => {
    markSubWindowOpenPhase("painted")
    markWindowContentReady()
  }, [])

  const toggleDirectory = useCallback((key: string) => setSelection(previous => ({ ...previous, collapsed: toggleInSet(previous.collapsed, key) })), [])

  // the tree shows the selected file, at the row of it given (a file "also in" a concern) or its own: its directories open
  const select = useCallback(
    (path: string, row: string | null) => {
      setSelection(previous => ({ ...previous, selectedPath: path, selectedRow: row, collapsed: revealInTree(previous.collapsed, row ?? path, nodes) }))
    },
    [nodes],
  )

  // the section at the top selects the row of its file in its concern; a use left outside has no row there
  const selectShown = useCallback(
    (key: string, path: string) => {
      if (files.some(file => file.path === path)) {
        const row = rows.rowOf.get(key) ?? path
        select(path, row === path ? null : row)
      }
    },
    [files, rows, select],
  )

  // the opening of a step at the top selects the row of its step (of a layer of several, `step`); a step of no row (what
  // no evidence joined) its file
  const selectStep = useCallback(
    (key: string, path: string, step?: string) => {
      const chapter = step ?? narrative?.find(it => it.key === key)?.concern ?? null
      const row = chapter === null ? undefined : concernRowKey(chapter)
      // the groups of the steps are rows of the top of the tree
      if (row !== undefined && nodes.some(node => node.key === row)) {
        setSelection(previous => ({ ...previous, selectedPath: path, selectedRow: row }))
      } else {
        selectShown(key, path)
      }
    },
    [narrative, nodes, selectShown],
  )

  // the section at the top of the diffs, for the bar of its step above them: as they scroll, and as a key, a row or a step
  // of the overview scrolls them, which the diffs do not report. At the opening of a step the tree selects the step, at
  // a card its file; the overview keeps the selection
  const [topSection, setTopSection] = useState<string | null>(null)
  // the step whose opening is at the top: of a section that holds the openings of several, the one there
  const [topOpening, setTopOpening] = useState<string | null>(null)
  // the overview at the top, where the review opens: the bar says so
  const [overviewShown, setOverviewShown] = useState(true)
  // the card of the heading of a commit reviewed alone, across the places its overview mounts in
  const headingCard = useHeadingCardState()
  const showVisibleSection = useCallback(
    (key: string, path: string, part: LayoutPart = "card", step?: string) => {
      if (part === "opening") {
        selectStep(key, path, step)
      } else if (part === "card") {
        selectShown(key, path)
      }
      setTopSection(key)
      setTopOpening(part === "opening" ? (step ?? null) : null)
      setOverviewShown(part === "prologue")
    },
    [selectShown, selectStep],
  )

  // a row shows its first open section: of a file its own concern, of a file "also in" a concern its part there
  const selectInTree = useCallback(
    (file: CommitFile, key: string) => {
      select(file.path, key === file.path ? null : key)
      const section = rows.sectionOf.get(key)
      // a file shown on purpose, of a file in parts the part the row shows
      cardsSeen([{ path: file.path, key: section ?? file.path }])
      // the overview is no longer what the reader is at
      setOverviewShown(false)
      if (section === undefined) {
        withCode(view => view.scrollToFile(file.path))
      } else {
        setTopSection(section)
        setTopOpening(null)
        withCode(view => view.scrollToSection(section, "card"))
      }
    },
    [rows, select, cardsSeen, withCode],
  )

  // a place of a popover or of the card of the review plan: its file is selected as a row of the tree is, by its path
  // or its old path, and the diffs show its line; a file that the search hides is revealed
  const selectChangedFile = useCallback(
    (place: ReviewPlace) => {
      // a file shown on purpose, also one a search hid: the overview is no longer what the reader is at
      setOverviewShown(false)
      const file = files.find(it => it.path === place.path) ?? files.find(it => it.oldPath === place.path)
      if (file === undefined) {
        onRevealHidden(place)
        return
      }
      select(file.path, null)
      withCode(view => {
        // of a file in parts the part that shows the place, else the one its row shows
        const shown = showPlace(view, { ...place, path: file.path })
        cardsSeen([{ path: file.path, key: shown ?? rows.sectionOf.get(file.path) ?? file.path }])
      })
    },
    [files, select, onRevealHidden, cardsSeen, rows, withCode],
  )

  // a use the change left outside: its file at the head of the review opens over it
  const showOutside = useCallback(
    (place: ReviewPlace) => {
      const repositoryPath = getDiffRepositoryPath()
      if (repositoryPath === undefined || revision === null) {
        return
      }
      void readOutsidePlace(repositoryPath, revision, place).then(target => {
        if (target !== null) {
          withCode(view => view.openOutside(target))
        }
      })
    },
    [revision, withCode],
  )
  // a step of the overview, of the bar or the title of its opening: its opening at the top of the diffs, its row selected
  const showStep = useCallback(
    (step: string) => {
      const opening = openingOfStep(step)
      if (opening !== undefined) {
        showVisibleSection(opening.key, opening.path, "opening", step)
        withCode(view => view.scrollToSection(opening.key, "opening", step))
      }
    },
    [openingOfStep, showVisibleSection, withCode],
  )
  // the block of a story a mark of a section showed last: its story outlines it
  const [highlight, setHighlight] = useState<StoryHighlight | null>(null)
  const showStory = useCallback(
    (mark: StoryMark) => {
      if (mark.step === null) {
        code.current?.scrollToPrologue()
        setOverviewShown(true)
      } else {
        showStep(mark.step)
      }
      setHighlight(previous => ({ commit: mark.commit, block: mark.block, serial: (previous?.serial ?? 0) + 1 }))
    },
    [showStep],
  )
  const navigator = useMemo<ReviewNavigator>(() => ({ show: selectChangedFile, showOutside, showStep, showStory }), [selectChangedFile, showOutside, showStep, showStory])
  // the link that opened the review, once every diff is read and the sections are told (the story of the plan, else
  // the diffs): followed once
  const linkReady = diffsComplete && (reviewFacts === null || narrative !== null)
  const followedLink = useRef<ReviewLink | null>(null)
  useEffect(() => {
    if (initialLink !== null && linkReady && followedLink.current !== initialLink) {
      followedLink.current = initialLink
      followLink(navigator, initialLink)
    }
  }, [initialLink, linkReady, navigator])

  // a commit row, and the row of a step of the commits (the commit that starts it): selected, its step's opening at the
  // top of the diffs, as a step of the overview; Enter and Show Commit Alone the commit alone
  const selectCommit = useCallback(
    (node: FileTreeCommit) => {
      const opening = node.chapter === null ? undefined : openingOfStep(node.chapter)
      setSelection(previous => ({ ...previous, selectedPath: opening?.path ?? previous.selectedPath, selectedRow: node.key }))
      if (opening !== undefined && node.chapter !== null) {
        setTopSection(opening.key)
        setTopOpening(node.chapter)
        setOverviewShown(false)
        code.current?.scrollToSection(opening.key, "opening", node.chapter)
      }
    },
    [openingOfStep],
  )
  const onShowCommit = commitRows?.onShow
  const commitMenu = commitRows?.menu
  const treeCommitRows = useMemo<FileTreeCommitRows | null>(
    () =>
      onShowCommit === undefined || commitMenu === undefined
        ? null
        : {
            onSelect: selectCommit,
            onOpen: node => {
              if (node.commit.item !== undefined) {
                onShowCommit(node.commit.item.key)
              }
            },
            menu: node => (node.commit.item === undefined ? [] : commitMenu(node.commit.item)),
          },
    [onShowCommit, commitMenu, selectCommit],
  )
  // the opening of each step above the card of its first section
  const openings = useMemo(() => (reviewFacts === null ? undefined : presentOpenings(reviewFacts)), [reviewFacts])
  const groupOpenings = useMemo(() => (reviewFacts === null ? undefined : presentGroupOpenings(reviewFacts)), [reviewFacts])
  const topStep = useMemo(
    () => (narrative === null ? null : presentStepBar(chapters, narrative, { opening: topOpening, section: topSection })),
    [narrative, topSection, topOpening, chapters],
  )
  // the group of a large step the section at the top is of, with the part of it seen: where the reader is in the step
  const topGroup = useMemo(() => {
    const path = topOpening === null ? narrative?.find(it => it.key === topSection)?.path : undefined
    const group = path === undefined ? undefined : topStep?.chapter?.groups.find(it => it.paths.includes(path))
    return group === undefined ? null : { name: group.name, code: group.code, seen: formatSeen(group.paths.filter(it => seenPaths.has(it)).length, group.paths.length) }
  }, [narrative, topSection, topOpening, topStep, seenPaths])

  // the badges of the review plan
  const badges = useMemo<FileTreeBadges | null>(() => {
    if (reviewFacts === null) {
      return null
    }
    const dependencies = new Map<string, DependencyBadge>()
    for (const [key, changes] of dependencyRows(nodes, reviewFacts.dependencies)) {
      const badge = presentDependencyBadge(changes)
      if (badge !== null) {
        dependencies.set(key, badge)
      }
    }
    return { files: presentFileBadges(reviewFacts), dependencies, onSelectPlace: selectChangedFile }
  }, [reviewFacts, nodes, selectChangedFile])

  // the files of the chosen owner chip: the tree counts them on the steps; an owner without files here marks nothing
  const treeMarks = useMemo<FileTreeMarks | null>(() => {
    if (ownerMarks === null || ownerMarks.paths.size === 0) {
      return null
    }
    const counts = new Map([
      ...[...presentOwnerMarks(chapters, ownerMarks.paths)].map(([chapter, count]) => [concernRowKey(chapter), count] as const),
      // of a large step each of its groups too
      ...presentGroupCounts(chapters, ownerMarks.paths).map(({ chapter, group, shown, total }) => [groupRowKey(chapter, group), { shown, total }] as const),
    ])
    return { label: ownerMarks.label, counts }
  }, [ownerMarks, chapters])

  // the files the reader saw of each large step and each of its groups
  const treeProgress = useMemo<ReadonlyMap<string, FileTreeProgress> | null>(() => {
    const grouped = chapters.filter(chapter => chapter.groups.length > 0)
    if (grouped.length === 0) {
      return null
    }
    return new Map([
      ...[...presentOwnerMarks(grouped, seenPaths)].map(([chapter, { shown, total }]) => [concernRowKey(chapter), { seen: shown, total }] as const),
      ...presentGroupCounts(grouped, seenPaths).map(({ chapter, group, shown, total }) => [groupRowKey(chapter, group), { seen: shown, total }] as const),
    ])
  }, [chapters, seenPaths])

  // the order J and K step through: with the plan the document, its overview, then the sections of the story, a step
  // at its opening, then at its card; else the files of the tree
  const hasPrologue = prologue !== undefined && prologue !== null
  const visibleChapters = useMemo(
    () => new Set(chapters.filter(chapter => !ownerFiltered || (ownerPaths !== null && chapter.paths.some(path => ownerPaths.has(path)))).map(chapter => chapter.id)),
    [chapters, ownerFiltered, ownerPaths],
  )
  const stepOrder = useMemo<readonly NextSection[]>(() => {
    if (reviewFacts === null || viewFiles === null) {
      return files.map(file => ({ key: file.path, path: file.path, part: "card" }))
    }
    const stops: NextSection[] = hasPrologue && viewFiles.length > 0 ? [{ key: PROLOGUE_KEY, path: viewFiles[0]!.file.path, part: "prologue" }] : []
    for (const file of viewFiles) {
      const key = file.section?.key ?? file.file.path
      const step = file.section?.step
      if (step != null) {
        // the openings of the steps of no card told before it, each a stop, then its own
        for (const it of [...(file.section?.before ?? []), step]) {
          if (visibleChapters.has(it.id)) {
            stops.push({ key, path: file.file.path, part: "opening", step: it.id })
          }
        }
      }
      stops.push({ key, path: file.file.path, part: "card" })
    }
    // the openings of the steps of no card after the last card, of the last section
    const last = viewFiles.at(-1)
    if (last !== undefined) {
      for (const step of withoutCard.after) {
        if (visibleChapters.has(step.id)) {
          stops.push({ key: last.section?.key ?? last.file.path, path: last.file.path, part: "opening", step: step.id })
        }
      }
    }
    return stops
  }, [reviewFacts, viewFiles, files, hasPrologue, withoutCard, visibleChapters])
  // what J (1) and K (-1) show after the stop at the top (none: from before the first), only steps and cards of shown files
  const stepSection = useCallback(
    (from: { key: string; part: LayoutPart; step?: string } | null, direction: 1 | -1): NextSection | undefined => {
      const at =
        from === null
          ? -1
          : stepOrder.findIndex(it =>
              from.part === "prologue"
                ? it.part === "prologue"
                : it.key === from.key && it.part === from.part && (from.part !== "opening" || from.step === undefined || it.step === from.step),
            )
      for (let index = at + direction; index >= 0 && index < stepOrder.length; index += direction) {
        const stop = stepOrder[index]!
        return stop
      }
      return undefined
    },
    [stepOrder],
  )
  // the line at the end of a card names the section J shows after it
  const nextSection = useCallback((key: string) => stepSection({ key, part: "card" }, 1), [stepSection])
  // J, K and a click on the line at the end of a card: the stop at the top, then the tree, the bar of the step and the
  // footer follow it, which a scroll the review made does not tell them
  const showSection = useCallback(
    (next: NextSection) => {
      if (next.part === "prologue") {
        code.current?.scrollToPrologue()
      } else {
        code.current?.scrollToSection(next.key, next.part, next.step)
      }
      showVisibleSection(next.key, next.path, next.part, next.step)
      // J and K show a card on purpose: its file is seen, of a file in parts that part
      if (next.part === "card") {
        cardsSeen([{ path: next.path, key: next.key }])
      }
    },
    [showVisibleSection, cardsSeen],
  )
  const selectedIndex = files.findIndex(file => file.path === selectedPath)
  // the chosen owner chip is named in the footer; the file count is already there
  const ownerLabel = treeMarks === null ? null : (ownerMarks?.label ?? null)
  // the overview of the plan at the top is no file; else the step of the story at the top, then its file. The message
  // of a commit shown without the plan heads its files as before: the first file is the one shown
  const [planned, setPlanned] = useState(false)
  if (reviewFacts !== null && !planned) {
    // the window shows a plan: a read of it again (a sync) keeps the overview the overview
    setPlanned(true)
  }
  const overviewAtTop = overviewShown && prologue !== undefined && prologue !== null && planned
  const stepAtTop = topStep?.chapter?.told === true ? topStep.chapter.number : undefined
  // the part of its file the card at the top is, of a file the story shows in several sections: numbers, which stay the
  // same while the story is laid out again
  const topCard = topOpening === null ? narrative?.find(it => it.key === topSection) : undefined
  const partAtTop = topCard?.part != null && topCard.path === selectedPath ? topCard.part : null
  const partIndex = partAtTop?.index
  const partCount = partAtTop?.count
  const standIn = standsIn(selection)
  useEffect(() => {
    if (loadedKey !== null) {
      onPositionChange({
        path: selectedPath,
        index: selectedIndex,
        count: files.length,
        ...(ownerLabel !== null && { owner: ownerLabel }),
        ...(overviewAtTop && { overview: true }),
        ...(stepAtTop !== undefined && { step: stepAtTop }),
        ...(partIndex !== undefined && partCount !== undefined && { part: { index: partIndex, count: partCount } }),
        ...(standIn && { standIn: true }),
      })
    }
  }, [onPositionChange, loadedKey, selectedPath, selectedIndex, files.length, ownerLabel, overviewAtTop, stepAtTop, partIndex, partCount, standIn])

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.defaultPrevented) {
      return
    }
    if ((event.metaKey || event.ctrlKey) && !event.altKey) {
      if (event.code === "KeyF" && !event.shiftKey) {
        event.preventDefault()
        if (code.current === null) {
          findWaiting.current = subjectId
        } else {
          findWaiting.current = null
          code.current.openFind()
        }
      } else if (event.code === "KeyG") {
        event.preventDefault()
        code.current?.stepFind(event.shiftKey ? -1 : 1)
      }
      return
    }
    // the other keys are of the sidebar and the diffs, not of the frame that holds the focus while neither has it
    if (event.target === event.currentTarget) {
      return
    }
    if (loaded === null || event.altKey || event.metaKey || event.ctrlKey || event.shiftKey || isTypingTarget(event)) {
      return
    }
    // the physical keys, as E and F of the preview; each a letter no action of the window names (`REVIEW_KEYS`)
    const action = reviewKeyAction(event.code)
    switch (action) {
      case "nextSection":
      case "previousSection": {
        // with the plan the diffs are the document: J and K step through the overview, the opening of each step and
        // the sections of the story from the stop at the top, else through the files from the selected one; the tree
        // follows; under an owner chip through its files only, nothing moves past the last one
        const from = reviewFacts !== null ? (code.current?.visibleStop() ?? null) : selectedPath === null ? null : { key: selectedPath, part: "card" as const }
        const next = stepSection(from, action === "nextSection" ? 1 : -1)
        if (next !== undefined) {
          showSection(next)
        }
        break
      }
      case "sidebar":
        // the sidebar, as F shows the files of a preview
        setReviewSidebar(!sidebarShown)
        break
      case "nextChange":
      case "previousChange": {
        const shown = code.current?.moveChange(action === "nextChange" ? 1 : -1)
        if (shown !== undefined) {
          select(shown.path, null)
          // of a file in parts the part of the change
          cardsSeen([shown])
          // a change shown on purpose: the reader is at its file, also while the overview still shows above it
          setOverviewShown(false)
        }
        break
      }
      case "notes":
        // the old lines of the notes in view (Inline shows a quiet run once); with no such note there nothing changes
        code.current?.toggleNotes()
        break
      case "moveEnd":
        // the other end of a move, from the note of one of its ends in view; with no such note there nothing moves
        code.current?.jumpMove()
        break
      default:
        return
    }
    event.preventDefault()
  }

  return (
    <ReviewRoot subjectId={subjectId} onPositionChange={onPositionChange} ready={loaded !== null} onKeyDown={handleKeyDown}>
      <ResizablePanelGroup orientation="horizontal" defaultLayout={sidebarShown ? defaultLayout : undefined} onLayoutChanged={onLayoutChanged}>
        {sidebarShown && (
          <ResizablePanel id="sidebar" defaultSize={SIDEBAR_WIDTH} minSize="16rem" maxSize="50">
            <div className="flex h-full flex-col" data-testid="review-sidebar">
              {sidebarTop !== undefined && (
                // what the user works in here keeps the focus when the diffs of the next subject mount
                <div data-review-keeps-focus="" className="flex max-h-[40%] min-h-0 shrink-0 flex-col border-b border-border">
                  {sidebarTop(listsCommits(nodes))}
                </div>
              )}
              {search !== null && (
                // the diffs that mount for what is typed leave the focus in the field
                <div data-review-keeps-focus="" className="shrink-0 px-2 pt-2">
                  <SearchField
                    value={search.text}
                    onValueChange={search.onTextChange}
                    label="Filter files"
                    className="w-full"
                    onLeave={() => (code.current ?? tree.current?.closest<HTMLElement>("[data-review-root]"))?.focus()}
                    onArrowDown={() => tree.current?.querySelector<HTMLElement>("[role=treeitem][tabindex='0']")?.focus()}
                  />
                </div>
              )}
              <div
                data-review-keeps-focus=""
                className="flex min-h-10 shrink-0 flex-wrap items-center gap-1.5 px-3 py-2 text-xs text-muted-foreground"
                data-testid="review-files-header"
              >
                {filesHeader !== undefined
                  ? filesHeader(loaded === null ? null : { shown: loaded.files.length, total: loaded.totalCount })
                  : loaded !== null && (
                      <SectionLabel title="Files" count={loaded.files.length} of={loaded.files.length === loaded.totalCount ? undefined : loaded.totalCount} className="flex-1" />
                    )}
              </div>
              <div ref={tree} className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
                {loaded !== null && (
                  <FileTree
                    key={loaded.key}
                    mode="navigate"
                    nodes={nodes}
                    aria-label="Changed files"
                    collapsed={collapsed}
                    onToggleDirectory={toggleDirectory}
                    // the overview at the top selects no row: no file is shown
                    selectedKey={overviewAtTop ? null : (selectedRow ?? selectedPath)}
                    activeGroupKey={overviewAtTop ? null : activeGroupKey}
                    onSelectFile={selectInTree}
                    fileOwners={fileOwners}
                    badges={badges}
                    marks={treeMarks}
                    commitRows={treeCommitRows}
                    progress={treeProgress}
                  />
                )}
              </div>
            </div>
          </ResizablePanel>
        )}
        {sidebarShown && <ResizableHandle />}
        <ResizablePanel id="code" minSize="40">
          <div className="flex h-full flex-col">
            {loaded !== null && topStep !== null && (
              <StepBar
                view={topStep}
                overview={overviewShown && hasPrologue}
                group={overviewShown && hasPrologue ? null : topGroup}
                onShowStep={showStep}
                onShowSection={showSection}
              />
            )}
            {/* a search that hides every file keeps the overview: a click reveals the file */}
            <SelectReviewFileContext value={loaded === null && !filesRead ? null : navigator}>
              {/* the overview mounts in one place after another as the diffs load: its heading keeps its card there */}
              <HeadingCardContext value={headingCard}>
                <StoryHighlightContext value={highlight}>
                  {/* the canvas of the diffs from the start: the overview keeps its card and its place as they mount */}
                  {loaded === null ? (
                    <div className="dv-cards min-h-0 flex-1 overflow-y-auto">
                      <PlainPrologue prologue={prologue} />
                      {message}
                    </div>
                  ) : viewFiles === null ? (
                    <div className="dv-cards min-h-0 flex-1 overflow-y-auto">
                      <PlainPrologue prologue={prologue} />
                      <PendingFileHeaders files={files} fileOwners={fileOwners} />
                      <LoadingLine key={loaded.key} className="p-6">
                        Loading changes…
                      </LoadingLine>
                    </div>
                  ) : (
                    <div data-testid="commit-review-code" className="min-h-0 flex-1">
                      <CommitReviewCode
                        key={loaded.key}
                        revisions={loaded.revisions}
                        ref={code}
                        files={viewFiles}
                        diffViewMode={diffViewMode}
                        initialPath={selection.initialPath ?? selection.arrived}
                        onVisibleSectionChange={showVisibleSection}
                        onFirstScreen={showWindow}
                        fileOwners={fileOwners}
                        onShowStep={showStep}
                        prologue={prologue}
                        openings={openings}
                        groupOpenings={groupOpenings}
                        after={withoutCard.after}
                        nextSection={nextSection}
                        onShowSection={showSection}
                        ownerLabel={ownerLabel}
                        onCardsPassed={treeProgress === null ? undefined : cardsSeen}
                      />
                    </div>
                  )}
                </StoryHighlightContext>
              </HeadingCardContext>
            </SelectReviewFileContext>
          </div>
        </ResizablePanel>
      </ResizablePanelGroup>
    </ReviewRoot>
  )
}

/**
 * The overview while the diffs are read or cannot show: its card above the headers and the message, at the inset and
 * the space above the cards (the diffs hold it as their prologue once they show, diff-view.css)
 */
function PlainPrologue({ prologue }: { prologue: ReactNode }) {
  return prologue === undefined || prologue === null ? null : (
    <div className="px-3 pt-2.5">
      <div className="dv-prose dv-prologue">{prologue}</div>
    </div>
  )
}

/**
 * The bar above the diffs of a review with its plan (openspec/specs/branch/review-tour/spec.md, "In the Review Window"): one line,
 * what the section at the top is of (`StepBarView`): a step by its place in the story and its title, whole in its
 * hover, which a click shows at its opening; Other changes and a group of no step by their name alone, a click showing
 * the opening of Other changes, the first section of the group; "Overview · 4 steps" while the overview is at the top,
 * a click the first step. It follows the diffs as they scroll. It lies on the canvas of the diffs, so the column of the
 * review is one canvas of cards
 */
function StepBar({
  view,
  overview,
  group,
  onShowStep,
  onShowSection,
}: {
  view: StepBarView
  overview: boolean
  /** The group of a large step the section at the top is of, with the part of it seen: "view.gui.media · 12 / 59 seen" */
  group: { readonly name: string; readonly code: boolean; readonly seen: string } | null
  onShowStep: (step: string) => void
  onShowSection: (section: NextSection) => void
}) {
  const position = overview ? "Overview" : view.position
  return (
    <button
      type="button"
      data-testid="review-step-bar"
      data-step={view.chapter?.id}
      title={overview ? undefined : view.label}
      className="flex min-w-0 shrink-0 items-baseline gap-1.5 bg-canvas px-3 py-1.5 text-left text-xs outline-none hover:bg-muted focus-visible:bd-focus"
      onClick={() => {
        if (view.chapter !== null) {
          onShowStep(view.chapter.id)
        } else if (view.section !== null) {
          onShowSection({ ...view.section, part: "card" })
        }
      }}
    >
      {position !== null && (
        <>
          <span className="shrink-0 text-muted-foreground tabular-nums" data-testid="review-step-position">
            {position}
          </span>
          <span aria-hidden className="shrink-0 text-muted-foreground">
            ·
          </span>
        </>
      )}
      <span className={cn("min-w-0 truncate font-medium", !overview && view.code && "font-mono")} data-testid="review-step-title">
        {overview ? formatStepCount(view.steps) : <ReviewTitleText title={view.title} code={view.code} />}
      </span>
      {group !== null && (
        <span className="flex shrink-0 items-baseline gap-1.5 text-muted-foreground" data-testid="review-step-group">
          <span aria-hidden>›</span>
          <span className={cn("font-medium text-foreground", group.code && "font-mono")}>{group.name}</span>
          <span aria-hidden>·</span>
          <span className="tabular-nums">{group.seen}</span>
        </span>
      )}
    </button>
  )
}
