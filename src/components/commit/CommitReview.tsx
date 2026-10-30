import { useCallback, useEffect, useEffectEvent, useMemo, useRef, useState } from "react"
import type { KeyboardEvent, ReactNode } from "react"
import { useDefaultLayout } from "react-resizable-panels"
import { Button } from "@/components/ui/button"
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "@/components/ui/resizable"
import { CommitReviewCode, PendingFileHeaders } from "@/components/diff/CommitReviewCode"
import type { CommitReviewCodeHandle } from "@/components/diff/CommitReviewCode"
import type { DiffViewFile } from "@/components/diff/DiffView"
import { FileTree } from "@/components/files/FileTree"
import type { FileTreeBadges, FileTreeCommitRows, FileTreeMarks } from "@/components/files/FileTree"
import { Callout } from "@/components/shared/Callout"
import { LoadingLine } from "@/components/shared/LoadingLine"
import { SearchField } from "@/components/shared/SearchField"
import { SectionLabel } from "@/components/shared/SectionLabel"
import { SelectReviewFileContext } from "@/hooks/useSelectReviewFile"
import type { ReviewNavigator } from "@/hooks/useSelectReviewFile"
import type { CommitFile, DeclarationTarget, DiffRevisions, MechanicalKind } from "@/lib/bindings"
import type { DiffSubject } from "@/lib/commit-files"
import { diffSubjectId, parseDiffSubjectId } from "@/lib/commit-files"
import type { FileDiff } from "@/lib/diff/file-diff-model"
import { getHighlightClient } from "@/lib/diff/highlight/highlighter"
import type { SectionUnits } from "@/lib/diff/rows"
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
import type { ReviewPlace } from "@/lib/review-place"
import { readOutsideExcerpt, readOutsidePlace } from "@/lib/review-plan"
import type { ReviewPosition } from "@/lib/review-position"
import { markWindowContentReady, markSubWindowOpenPhase } from "@/lib/sub-window"
import { presentDependencyBadge, presentFileBadges } from "@/presenters/review-plan"
import type { DependencyBadge, ReviewFacts } from "@/presenters/review-plan"
import { formatStepPosition, placeCommits, presentBrief, presentChapters, presentOwnerMarks } from "@/presenters/review-steps"
import type { BriefStep, Chapter } from "@/presenters/review-steps"
import { presentNarrative } from "@/presenters/review-units"
import { getFilesOfCommit, loadFilesOfSubject, useFilesOfCommit } from "@/stores/commit-files"
import { loadFileDiffs, useFileDiffs, whenFileDiffsEnded } from "@/stores/file-diffs"
import { getDiffRepositoryPath } from "@/stores/repository"
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
 * The files of the chosen owner chip (spec/branch/branch-review.spec.md, "Code owners"): every file stays, the others are
 * muted, J/K and N/P move through these only; empty (the owner has no files here) mutes nothing and steers nothing
 */
export interface OwnerMarks {
  /** The name of the chip: "Design" */
  readonly label: string
  readonly paths: ReadonlySet<string>
}

/**
 * The commits of a range shown whole with its plan (spec/branch/review-tour.spec.md, "In the Review Window"): the tree
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
  /** The selected file, for the footer ("File 3 / 12") and for a review that starts again at it */
  onPositionChange: (position: ReviewPosition) => void
  /** The files of the chosen owner chip: marked in the whole story, the keys move through them; none without */
  ownerMarks?: OwnerMarks | null
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
  /** Above the diffs: the commit a range shows; it selects a changed file with `useSelectReviewFile` */
  codeTop?: ReactNode
  /** What ⌥↑/⌥↓ show next: read once the subject is, so the step is instant */
  neighbours?: readonly DiffSubject[]
  /**
   * The facts of the review plan of the subject: the badges of the rows, the mechanical files folded into one group at
   * the end of the tree; the plain tree without them
   */
  reviewFacts?: ReviewFacts | null
}

/**
 * The review of a commit or of the range of a pull request (`base..head`): a sidebar (the commits of a range, the
 * header of the files, their directory tree) beside the diffs of all of them in one virtualized view. The split is
 * there in every state, so what the sidebar holds keeps its focus while another commit or filter loads.
 * The tree follows the diffs as they scroll, a file selected in the tree scrolls the diffs to it. J and K step through
 * the files, N and P through the blocks of changed lines. From `FILE_FILTER_MIN_FILES` files on the sidebar has a
 * search of the files ("/"): the tree and the diffs show the files with every word in their path, also of the next
 * commit (⌥↑/⌥↓). An owner chip (`ownerMarks`) hides no file: it mutes the others and steers the keys.
 */
export function CommitReview({
  subject,
  initialPath,
  onPositionChange,
  ownerMarks = null,
  fileOwners,
  rangeNoun = "pull request",
  sidebarTop,
  commitRows = null,
  filesHeader,
  codeTop,
  neighbours,
  reviewFacts = null,
}: CommitReviewProps) {
  const subjectId = diffSubjectId(subject)
  const commitFiles = useFilesOfCommit(subjectId)
  const policy = useDiffWhitespacePolicy()
  const entry = useFileDiffs(subject, policy)
  const [filesFailed, setFilesFailed] = useState<string | null>(null)
  const [filterText, setFilterText] = useState("")
  const allFiles = commitFiles?.files
  const shownFiles = useMemo(() => (allFiles === undefined ? undefined : filterFilesByText(allFiles, filterText)), [allFiles, filterText])
  // the count of the last subject read: the search stays while the next one loads
  const [fileCount, setFileCount] = useState(0)
  if (allFiles !== undefined && allFiles.length !== fileCount) {
    setFileCount(allFiles.length)
  }
  const isSearchShown = filterText !== "" || fileCount >= FILE_FILTER_MIN_FILES

  // a file a link of a mark selects while the search hides it: the search is cleared, then it is selected once, and the
  // tree reports that (`onRevealApplied`); a subject or search other than those the click left forgets it before. An
  // owner chip hides nothing
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
      const hiddenBySearch = filterFilesByText([file], filterText).length === 0
      if (hiddenBySearch) {
        setFilterText("")
      }
      setReveal({ subjectId, place: { ...place, path: file.path }, filterText: hiddenBySearch ? "" : filterText })
    },
    [commitFiles, filterText, subjectId],
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
  } else if (shownFiles.length === 0) {
    message = <p className="p-6 text-sm text-muted-foreground">No changed files</p>
  } else {
    loaded = {
      // the selection and the collapsed directories belong to one commit or range; an owner chip keeps them
      key: subjectId,
      // the counts of the engine, none until its diffs are read: git's numstat counts what the policy ignores too
      files: withEngineCounts(shownFiles, entry?.kind === "loaded" ? entry.files : undefined),
      totalCount: allFiles?.length ?? shownFiles.length,
      diffs: entry?.kind === "loaded" ? entry.files : null,
      diffsComplete: entry?.kind === "loaded" && entry.complete,
      initialPath: revealPath ?? (initialPath !== null && shownFiles.some(file => file.path === initialPath) ? initialPath : null),
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
    <ReviewRoot subjectId={subjectId} onPositionChange={onPositionChange} ready={loaded !== null}>
      <ReviewLayout
        loaded={loaded}
        message={message}
        onPositionChange={onPositionChange}
        fileOwners={fileOwners}
        ownerMarks={ownerMarks}
        sidebarTop={sidebarTop}
        commitRows={commitRows}
        filesHeader={filesHeader}
        codeTop={codeTop}
        search={isSearchShown ? { text: filterText, onTextChange: setFilterText } : null}
        onRevealHidden={revealHidden}
        filesRead={commitFiles !== undefined && commitFiles.files.length > 0}
        onRevealApplied={forgetReveal}
      />
    </ReviewRoot>
  )
}

/**
 * The focusable frame of every state of the review: while another commit loads, has no files or cannot be read, the
 * focus stays here (the diffs that had it are gone), so the keys of the surface (⌥↑/⌥↓, Escape) still reach it
 */
function ReviewRoot({
  subjectId,
  ready,
  onPositionChange,
  children,
}: {
  subjectId: string
  ready: boolean
  onPositionChange: (position: ReviewPosition) => void
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
    <div ref={root} tabIndex={-1} data-review-root="" data-testid="commit-review" className="h-full min-h-0 outline-none">
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
  loaded: LoadedReview | null
  /** Where the diffs are while there are none: loading, the error, no files */
  message: ReactNode
  onPositionChange: (position: ReviewPosition) => void
  fileOwners: ReadonlyMap<string, string> | undefined
  ownerMarks: OwnerMarks | null
  sidebarTop: CommitReviewProps["sidebarTop"]
  commitRows: CommitRows | null
  filesHeader: CommitReviewProps["filesHeader"]
  codeTop: ReactNode
  search: FileSearch | null
  /** A link of a mark shows a place of a changed file that the search hides */
  onRevealHidden: (place: ReviewPlace) => void
  /** The subject has changed files: the card selects one also while the search hides every file */
  filesRead: boolean
  /** The tree selected `LoadedReview.revealPath`: it is not selected again */
  onRevealApplied: () => void
}

/** The tree state of one loaded subject (`LoadedReview.key`) and one search text */
interface TreeSelection {
  key: string | null
  filterText: string
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
  /** The file of a link this selection selected (`LoadedReview.revealPath`) */
  revealed: string | null
}

function initialSelection(loaded: LoadedReview | null, filterText: string, nodes: readonly FileTreeNode[], fileCount: number, firstPath: string | null): TreeSelection {
  if (loaded === null) {
    return { key: null, filterText, collapsed: new Set(), selectedPath: null, selectedRow: null, grouped: false, initialPath: null, defaultPath: null, revealed: null }
  }
  // what a search found shows whole: its directories open
  const initial = filterText.trim() === "" ? getInitiallyCollapsed(nodes, fileCount) : new Set<string>()
  return {
    key: loaded.key,
    filterText,
    collapsed: loaded.initialPath === null ? initial : revealInTree(initial, loaded.initialPath, nodes),
    selectedPath: loaded.initialPath ?? firstPath,
    selectedRow: null,
    grouped: hasReviewGroups(nodes),
    initialPath: loaded.initialPath,
    defaultPath: loaded.initialPath === null ? firstPath : null,
    revealed: loaded.revealPath,
  }
}

/**
 * Another search text: the directories as a new tree has them, the selected file kept while the search keeps it; the
 * file a link revealed (`revealPath`) selected
 */
function searchedSelection(previous: TreeSelection, filterText: string, nodes: readonly FileTreeNode[], files: readonly CommitFile[], revealPath: string | null): TreeSelection {
  const kept = revealPath ?? previous.selectedPath
  const selectedPath = files.some(file => file.path === kept) ? kept : (files[0]?.path ?? null)
  const collapsed = filterText.trim() === "" ? getInitiallyCollapsed(nodes, files.length) : new Set<string>()
  return {
    ...previous,
    filterText,
    collapsed: selectedPath === null ? collapsed : revealInTree(collapsed, selectedPath, nodes),
    selectedPath,
    selectedRow: null,
    grouped: hasReviewGroups(nodes),
    revealed: revealPath,
  }
}

/**
 * The plan came, or went: the Mechanical group and the generated files of each concern start collapsed unless they
 * hold the selected file. A file selected only as the first of the tree without the plan, which may be a generated
 * one, gives way to the first of the new tree
 */
function groupedSelection(previous: TreeSelection, nodes: readonly FileTreeNode[], firstPath: string | null): TreeSelection {
  const grouped = hasReviewGroups(nodes)
  if (!grouped) {
    return { ...previous, grouped, selectedRow: null }
  }
  const moved = previous.selectedPath !== null && previous.selectedPath === previous.defaultPath
  const selectedPath = moved ? firstPath : previous.selectedPath
  const collapsed = new Set([...previous.collapsed, ...foldedGroups(nodes)])
  return {
    ...previous,
    grouped,
    selectedPath,
    selectedRow: null,
    defaultPath: moved ? firstPath : previous.defaultPath,
    collapsed: selectedPath === null ? collapsed : revealInTree(collapsed, selectedPath, nodes),
  }
}

const NO_MECHANICAL: ReadonlyMap<string, MechanicalKind> = new Map()
const NO_ROWS = { rowOf: new Map<string, string>(), sectionOf: new Map<string, string>() }

/** A section of the story that shows its file whole */
const NO_UNITS: SectionUnits = { marks: new Map(), elsewhere: new Map() }

/**
 * A use a change left outside (`ghost`), as a section of the story: its file at the head (`target`, unread while
 * undefined) around its line, the lines before and after it as context, a heading before the line; no change, so N
 * and P pass it (an empty block played down as the imports are)
 */
function ghostFile(key: string, path: string, ghost: { line: number; heading: string }, target: DeclarationTarget | undefined): DiffViewFile {
  const blobId = target?.outside?.blobId ?? null
  const lineCount = target?.outside?.lineCount ?? 0
  const at = ghost.line - 1
  const file: CommitFile = { path, status: "modified", oldPath: null, additions: 0, deletions: 0, binary: false, oldId: blobId, newId: blobId, submodule: false }
  const section = { key, units: { marks: new Map([[0, ghost.heading]]), elsewhere: new Map() }, targets: new Map(), heading: "Left outside", concern: null }
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
    },
  }
  return { file, diff, quietImports: true, section }
}

/** The diffs show `place` of a shown file: its line with its name selected, or its header without one */
function showPlace(code: CommitReviewCodeHandle | null, place: ReviewPlace): void {
  const line = place.line === null ? null : { path: place.path, side: place.side, line: place.line - 1, column: 0, endColumn: 0, ...(place.name !== null && { name: place.name }) }
  if (line === null || code?.revealDeclaration(line, null) !== true) {
    code?.scrollToFile(place.path)
  }
}

/**
 * The width of the sidebar until its divider is dragged: what the names of a tree need, not a share of the window, so
 * a maximized window gives the diffs the rest (IntelliJ's commit tree is about as wide)
 */
const SIDEBAR_WIDTH = "19rem"

/**
 * The sidebar beside the diffs, in every state of the review: the parts of the sidebar (the commits, the header of the
 * files) stay mounted, only the tree and the diffs start over for another subject.
 */
function ReviewLayout({
  loaded,
  message,
  onPositionChange,
  fileOwners,
  ownerMarks,
  sidebarTop,
  commitRows,
  filesHeader,
  codeTop,
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
  // the steps of the story of the plan as the tree, each with its files (spec/branch/review-tour.spec.md), what no evidence
  // joined under Other changes, the mechanical ones in one group at the end, as Rust told, classed and ordered them
  const mechanical = useMemo(() => (reviewFacts === null ? NO_MECHANICAL : mechanicalKindsOf(reviewFacts)), [reviewFacts])
  const chapters = useMemo(() => (reviewFacts === null ? [] : presentChapters(reviewFacts)), [reviewFacts])
  // the commits of a range shown whole, each in the step it tells or follows up
  const commitItems = commitRows?.items
  const commitPlaces = useMemo(
    () => (reviewFacts === null || commitItems === undefined ? undefined : placeCommits(reviewFacts.steps.steps, commitItems)),
    [reviewFacts, commitItems],
  )
  const filterText = search?.text ?? ""
  // what a search found: the commits of the groups it keeps
  const searched = filterText.trim() !== ""
  const nodes = useMemo(() => {
    if (loadedFiles === undefined) {
      return []
    }
    return reviewFacts === null ? buildFileTree(loadedFiles) : buildReviewTree(loadedFiles, mechanical, chapters, commitPlaces, searched)
  }, [loadedFiles, reviewFacts, mechanical, chapters, commitPlaces, searched])
  const files = useMemo(() => flattenTreeFiles(nodes), [nodes])
  const [selection, setSelection] = useState<TreeSelection>(() => initialSelection(loaded, filterText, nodes, files.length, files[0]?.path ?? null))
  const loadedKey = loaded?.key ?? null
  if (selection.key !== loadedKey) {
    setSelection(initialSelection(loaded, filterText, nodes, files.length, files[0]?.path ?? null))
  } else if (selection.filterText !== filterText) {
    setSelection(searchedSelection(selection, filterText, nodes, files, loaded?.revealPath ?? null))
  } else if (selection.grouped !== hasReviewGroups(nodes)) {
    setSelection(groupedSelection(selection, nodes, files[0]?.path ?? null))
  }
  const { collapsed, selectedPath, selectedRow } = selection
  // the file of a link is selected once: a later key or search does not select it again
  const revealApplied = loaded !== null && loaded.revealPath !== null && selection.revealed === loaded.revealPath
  const code = useRef<CommitReviewCodeHandle>(null)
  const revealedPlace = loaded?.revealPlace ?? null
  useEffect(() => {
    if (revealApplied) {
      if (revealedPlace !== null) {
        showPlace(code.current, revealedPlace)
      }
      onRevealApplied()
    }
  }, [revealApplied, revealedPlace, onRevealApplied])
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

  // the diffs mount with the models of the engine; each file reads its texts by blob id as it comes into view, and the
  // window shows once the rows of the first screen are painted (`onFirstScreen`)
  const diffs = loaded?.diffs ?? null
  const diffsComplete = loaded?.diffsComplete ?? false
  // with the plan, the story of its units (spec/branch/review-units.spec.md) in the order of its steps: each run of
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
  const viewFiles = useMemo(() => {
    if (diffs === null) {
      return null
    }
    const byPath = new Map(diffs.map(diff => [diff.path, diff]))
    const counted = withEngineCounts(files, diffs)
    if (narrative === null) {
      return counted.map(file => ({ file, diff: byPath.get(file.path), pending: !diffsComplete && !byPath.has(file.path) }))
    }
    const fileOf = new Map(counted.map(file => [file.path, file]))
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
          section: { key: section.key, units: section.units ?? NO_UNITS, targets: section.targets, heading: section.heading, concern: section.concernHeading },
        },
      ]
    })
  }, [diffs, diffsComplete, files, narrative, outside])
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

  // the section at the top of the diffs, for the bar of its step above them: as they scroll, and as a key, a row or a step
  // of the brief scrolls them, which the diffs do not report
  const [topSection, setTopSection] = useState<string | null>(null)
  const showVisibleSection = useCallback(
    (key: string, path: string) => {
      selectShown(key, path)
      setTopSection(key)
    },
    [selectShown],
  )

  // a row shows its first open section: of a file its own concern, of a file "also in" a concern its part there
  const selectInTree = useCallback(
    (file: CommitFile, key: string) => {
      select(file.path, key === file.path ? null : key)
      const section = rows.sectionOf.get(key)
      if (section === undefined) {
        code.current?.scrollToFile(file.path)
      } else {
        setTopSection(section)
        code.current?.scrollToSection(section)
      }
    },
    [rows, select],
  )

  // a place of a popover or of the card of the review plan: its file is selected as a row of the tree is, by its path
  // or its old path, and the diffs show its line; a file that the search hides is revealed
  const selectChangedFile = useCallback(
    (place: ReviewPlace) => {
      const file = files.find(it => it.path === place.path) ?? files.find(it => it.oldPath === place.path)
      if (file === undefined) {
        onRevealHidden(place)
        return
      }
      select(file.path, null)
      showPlace(code.current, { ...place, path: file.path })
    },
    [files, select, onRevealHidden],
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
          code.current?.openOutside(target)
        }
      })
    },
    [revision],
  )
  // a step of the brief: its first section at the top of the diffs, its row selected
  const showStep = useCallback(
    (step: string) => {
      const section = narrative?.find(it => it.concern === step && it.ghost === null)
      if (section !== undefined) {
        showVisibleSection(section.key, section.path)
        code.current?.scrollToSection(section.key)
      }
    },
    [narrative, showVisibleSection],
  )
  const navigator = useMemo<ReviewNavigator>(() => ({ show: selectChangedFile, showOutside, showStep }), [selectChangedFile, showOutside, showStep])

  // a commit row: selected, its step's first section at the top of the diffs, as the step of the brief; Enter and Show
  // Commit Alone the commit alone
  const selectCommit = useCallback(
    (node: FileTreeCommit) => {
      const section = node.chapter === null ? undefined : narrative?.find(it => it.concern === node.chapter && it.ghost === null)
      setSelection(previous => ({ ...previous, selectedPath: section?.path ?? previous.selectedPath, selectedRow: node.key }))
      if (section !== undefined) {
        setTopSection(section.key)
        code.current?.scrollToSection(section.key)
      }
    },
    [narrative],
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
  const brief = useMemo(() => (reviewFacts === null ? null : presentBrief(reviewFacts)), [reviewFacts])
  const topStep = useMemo(() => {
    const chapter = narrative?.find(it => it.key === topSection)?.concern ?? narrative?.find(it => it.concern !== null)?.concern
    const index = chapters.findIndex(it => it.id === chapter)
    return index < 0 || brief === null ? null : { chapter: chapters[index]!, step: brief.steps[index] ?? null, count: chapters.length }
  }, [narrative, topSection, chapters, brief])

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

  // the files of the chosen owner chip: the tree mutes the other rows and counts them on the steps, the diffs mute the
  // headers of the others, the keys move through these only; an owner without files here marks nothing
  const marked = ownerMarks === null || ownerMarks.paths.size === 0 ? null : ownerMarks.paths
  const treeMarks = useMemo<FileTreeMarks | null>(() => {
    if (ownerMarks === null || marked === null) {
      return null
    }
    const counts = new Map([...presentOwnerMarks(chapters, marked)].map(([chapter, count]) => [concernRowKey(chapter), count]))
    return { label: ownerMarks.label, paths: marked, counts }
  }, [ownerMarks, marked, chapters])
  const accept = marked === null ? undefined : (path: string) => marked.has(path)

  const selectedIndex = files.findIndex(file => file.path === selectedPath)
  useEffect(() => {
    if (loadedKey !== null) {
      onPositionChange({ path: selectedPath, index: selectedIndex, count: files.length })
    }
  }, [onPositionChange, loadedKey, selectedPath, selectedIndex, files.length])

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (loaded === null || event.defaultPrevented || event.altKey || event.metaKey || event.ctrlKey || event.shiftKey || isTypingTarget(event)) {
      return
    }
    // the physical keys, as E and F of the preview
    switch (event.code) {
      case "KeyJ":
      case "KeyK": {
        // with the plan the diffs are the story: J and K step through its sections, the tree follows; under an owner
        // chip through the sections of its files only, nothing moves past the last one
        const direction = event.code === "KeyJ" ? 1 : -1
        if (reviewFacts !== null) {
          const next = code.current?.moveSection(direction, accept)
          if (next !== undefined) {
            showVisibleSection(next.key, next.path)
          }
          break
        }
        let index = selectedIndex + direction
        while (accept !== undefined && files[index] !== undefined && !accept(files[index]!.path)) {
          index += direction
        }
        const next = files[index]
        if (next !== undefined) {
          selectInTree(next, next.path)
        }
        break
      }
      case "KeyF":
        // the sidebar, as F shows the files of a preview
        setReviewSidebar(!sidebarShown)
        break
      case "KeyN":
      case "KeyP": {
        const path = code.current?.moveChange(event.code === "KeyN" ? 1 : -1, accept)
        if (path !== undefined) {
          select(path, null)
        }
        break
      }
      default:
        return
    }
    event.preventDefault()
  }

  return (
    <div className="h-full min-h-0" onKeyDown={handleKeyDown}>
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
                    selectedKey={selectedRow ?? selectedPath}
                    onSelectFile={selectInTree}
                    fileOwners={fileOwners}
                    badges={badges}
                    marks={treeMarks}
                    commitRows={treeCommitRows}
                  />
                )}
              </div>
            </div>
          </ResizablePanel>
        )}
        {sidebarShown && <ResizableHandle />}
        <ResizablePanel id="code" minSize="40">
          <div className="flex h-full flex-col">
            {/* a search that hides every file keeps the card: a click reveals the file */}
            <SelectReviewFileContext value={loaded === null && !filesRead ? null : navigator}>{codeTop}</SelectReviewFileContext>
            {loaded !== null && topStep !== null && <StepBar chapter={topStep.chapter} step={topStep.step} count={topStep.count} onShow={showStep} />}
            {loaded === null ? (
              <div className="min-h-0 flex-1 overflow-y-auto">{message}</div>
            ) : viewFiles === null ? (
              <div className="min-h-0 flex-1 overflow-y-auto">
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
                  initialPath={selection.initialPath}
                  onVisibleSectionChange={showVisibleSection}
                  onFirstScreen={showWindow}
                  fileOwners={fileOwners}
                  marked={marked}
                />
              </div>
            )}
          </div>
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  )
}

/**
 * The bar above the diffs of a review with its plan (spec/branch/review-tour.spec.md, "In the Review Window"): the step of
 * the section at the top, its place in the story, its title and the first paragraph of its body, which a click opens at
 * its first section; it follows the diffs as they scroll
 */
function StepBar({ chapter, step, count, onShow }: { chapter: Chapter; step: BriefStep | null; count: number; onShow: (step: string) => void }) {
  return (
    <button
      type="button"
      data-testid="review-step-bar"
      data-step={chapter.id}
      title={step?.why ?? chapter.label}
      className="flex min-w-0 shrink-0 items-baseline gap-2 border-b border-border px-3 py-1.5 text-left text-xs outline-none hover:bg-muted focus-visible:ring-3 focus-visible:ring-ring/30"
      onClick={() => onShow(chapter.id)}
    >
      <span className="shrink-0 text-muted-foreground tabular-nums" data-testid="review-step-position">
        {formatStepPosition(chapter.number, count)}
      </span>
      <span className="shrink-0 truncate font-medium" data-testid="review-step-title">
        {chapter.label}
      </span>
      {step?.why != null && <span className="min-w-0 truncate text-muted-foreground">{step.why}</span>}
      {step?.unproven != null && <span className="ml-auto shrink-0 text-warning">{step.unproven}</span>}
    </button>
  )
}
