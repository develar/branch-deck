import { useCallback, useEffect, useEffectEvent, useMemo, useRef, useState } from "react"
import type { KeyboardEvent, ReactNode } from "react"
import { useDefaultLayout } from "react-resizable-panels"
import { Button } from "@/components/ui/button"
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "@/components/ui/resizable"
import { CommitReviewCode, PendingFileHeaders } from "@/components/diff/CommitReviewCode"
import type { CommitReviewCodeHandle } from "@/components/diff/CommitReviewCode"
import type { DiffViewFile } from "@/components/diff/DiffView"
import { FileTree } from "@/components/files/FileTree"
import type { FileTreeBadges } from "@/components/files/FileTree"
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
  dependencyRows,
  flattenTreeFiles,
  foldedGroups,
  getInitiallyCollapsed,
  hasReviewGroups,
  mechanicalKindsOf,
  revealInTree,
  sectionRows,
  toggleInSet,
} from "@/lib/file-tree"
import type { FileTreeNode } from "@/lib/file-tree"
import { isTypingTarget } from "@/lib/preview-keys"
import type { ReviewPlace } from "@/lib/review-place"
import { readOutsideExcerpt, readOutsidePlace } from "@/lib/review-plan"
import type { ReviewPosition } from "@/lib/review-position"
import { markWindowContentReady, markSubWindowOpenPhase } from "@/lib/sub-window"
import { presentDependencyBadge, presentFileBadges } from "@/presenters/review-plan"
import type { DependencyBadge, ReviewFacts } from "@/presenters/review-plan"
import { presentNarrative } from "@/presenters/review-units"
import { getFilesOfCommit, loadFilesOfSubject, useFilesOfCommit } from "@/stores/commit-files"
import { loadFileDiffs, useFileDiffs, whenFileDiffsEnded } from "@/stores/file-diffs"
import { getDiffRepositoryPath } from "@/stores/repository"
import { setReviewSidebar, useAppSettings } from "@/stores/settings/appSettings"
import { useDiffViewMode, useDiffWhitespacePolicy } from "@/stores/settings/diff-view-settings"

// the highlight worker starts with the page of a review, beside the reads of its files and diffs, not after them
getHighlightClient()

/** The files a review shows (the filters of its sidebar applied) and all of them */
export interface FilesCount {
  readonly shown: number
  readonly total: number
}

export interface CommitReviewProps {
  /** A commit, or the range of a pull request */
  subject: DiffSubject
  /** The file shown first; the first file of the tree by default */
  initialPath: string | null
  /** The selected file, for the footer ("File 3 / 12") and for a review that starts again at it */
  onPositionChange: (position: ReviewPosition) => void
  /** Only these files (by the path or the old path of a rename); all files without it */
  pathFilter?: ReadonlySet<string> | null
  /** What an empty filter result says, e.g. "No files of Air Plugin in this range" */
  filteredEmptyText?: string
  /** The code owner of a file by its path, in the header of its diff */
  fileOwners?: ReadonlyMap<string, string>
  /** What the range is, in the error: "Cannot read the changes of the pull request" */
  rangeNoun?: string
  /** Above the files in the sidebar: the commits of a range (`RangeReview`) */
  sidebarTop?: ReactNode
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
  /** Drops `pathFilter`: a file the filter hides is shown when a link of a badge or of `codeTop` selects it */
  onShowAllPaths?: () => void
}

/**
 * The review of a commit or of the range of a pull request (`base..head`): a sidebar (the commits of a range, the
 * header of the files, their directory tree) beside the diffs of all of them in one virtualized view. The split is
 * there in every state, so what the sidebar holds keeps its focus while another commit or filter loads.
 * The tree follows the diffs as they scroll, a file selected in the tree scrolls the diffs to it. J and K step through
 * the files, N and P through the blocks of changed lines. From `FILE_FILTER_MIN_FILES` files on the sidebar has a
 * search of the files ("/"): the tree and the diffs show the files with every word in their path, also of the next
 * commit (⌥↑/⌥↓), on top of `pathFilter`.
 */
export function CommitReview({
  subject,
  initialPath,
  onPositionChange,
  pathFilter = null,
  filteredEmptyText,
  fileOwners,
  rangeNoun = "pull request",
  sidebarTop,
  filesHeader,
  codeTop,
  neighbours,
  reviewFacts = null,
  onShowAllPaths,
}: CommitReviewProps) {
  const subjectId = diffSubjectId(subject)
  const commitFiles = useFilesOfCommit(subjectId)
  const policy = useDiffWhitespacePolicy()
  const entry = useFileDiffs(subject, policy)
  const [filesFailed, setFilesFailed] = useState<string | null>(null)
  const [filterText, setFilterText] = useState("")
  const filteredFiles = useMemo(() => (commitFiles === undefined ? undefined : filterFiles(commitFiles.files, pathFilter)), [commitFiles, pathFilter])
  const shownFiles = useMemo(() => (filteredFiles === undefined ? undefined : filterFilesByText(filteredFiles, filterText)), [filteredFiles, filterText])
  // the count of the last subject read: the search stays while the next one loads
  const [fileCount, setFileCount] = useState(0)
  if (filteredFiles !== undefined && filteredFiles.length !== fileCount) {
    setFileCount(filteredFiles.length)
  }
  const isSearchShown = filterText !== "" || fileCount >= FILE_FILTER_MIN_FILES

  // a file a link of a badge selects while a filter hides it: only the filter that hides it is dropped, then it is
  // selected once, and the tree reports that (`onRevealApplied`); a subject, search or owner filter other than those
  // the click left forgets it before
  const pathFilterKey = pathFilter === null ? null : [...pathFilter].join("\n")
  const [reveal, setReveal] = useState<PendingReveal | null>(null)
  if (reveal !== null && (reveal.subjectId !== subjectId || reveal.filterText !== filterText || reveal.pathFilterKey !== pathFilterKey)) {
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
      const hiddenByOwner = filterFiles([file], pathFilter).length === 0
      if (hiddenByOwner) {
        if (onShowAllPaths === undefined) {
          return
        }
        onShowAllPaths()
      }
      const hiddenBySearch = filterFilesByText([file], filterText).length === 0
      if (hiddenBySearch) {
        setFilterText("")
      }
      // the parent drops its filter in the same batch: All has no path filter
      setReveal({
        subjectId,
        place: { ...place, path: file.path },
        filterText: hiddenBySearch ? "" : filterText,
        pathFilterKey: hiddenByOwner ? null : pathFilterKey,
      })
    },
    [commitFiles, pathFilter, pathFilterKey, filterText, onShowAllPaths, subjectId],
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
  } else if (shownFiles.length === 0 && filteredFiles !== undefined && filteredFiles.length > 0) {
    message = (
      <div data-testid="review-files-no-match" className="flex items-center gap-3 p-6 text-sm text-muted-foreground">
        <span className="min-w-0 truncate">{formatNoFileMatch(filterText)}</span>
        <Button variant="link" size="xs" className="h-auto px-0" onClick={() => setFilterText("")}>
          Clear Filter
        </Button>
      </div>
    )
  } else if (shownFiles.length === 0) {
    message = (
      <p className="p-6 text-sm text-muted-foreground">
        {pathFilter !== null && filteredEmptyText !== undefined && commitFiles !== undefined && commitFiles.files.length > 0 ? filteredEmptyText : "No changed files"}
      </p>
    )
  } else {
    loaded = {
      // the selection and the collapsed directories belong to one commit or range, and to one filter
      key: pathFilterKey === null ? subjectId : `${subjectId}|${pathFilterKey}`,
      // the counts of the engine, none until its diffs are read: git's numstat counts what the policy ignores too
      files: withEngineCounts(shownFiles, entry?.kind === "loaded" ? entry.files : undefined),
      totalCount: filteredFiles?.length ?? shownFiles.length,
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
        sidebarTop={sidebarTop}
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

function filterFiles(files: readonly CommitFile[], pathFilter: ReadonlySet<string> | null): readonly CommitFile[] {
  if (pathFilter === null) {
    return files
  }
  return files.filter(file => pathFilter.has(file.path) || (file.oldPath !== null && pathFilter.has(file.oldPath)))
}

/** A place a link of a badge showed while a filter hid its file, at the last path of the file, and the filters that show it */
interface PendingReveal {
  subjectId: string
  place: ReviewPlace
  /** The search after the click: empty when the search hid the file */
  filterText: string
  /** The owner filter after the click: none when it hid the file, as the parent then drops it */
  pathFilterKey: string | null
}

/** The files of the shown subject once read, and their diffs once read too: the tree does not wait for the diffs */
interface LoadedReview {
  /** The subject and the filter: a new key starts the tree and the diffs over */
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
  /** The file a link of a badge selected while a filter hid it, now shown: the tree selects it */
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
  sidebarTop: ReactNode
  filesHeader: CommitReviewProps["filesHeader"]
  codeTop: ReactNode
  search: FileSearch | null
  /** A link of a badge shows a place of a changed file that a filter hides */
  onRevealHidden: (place: ReviewPlace) => void
  /** The subject has changed files: the card selects one also while a filter hides every file */
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
 * files) stay mounted, only the tree and the diffs start over for another subject or filter.
 */
function ReviewLayout({ loaded, message, onPositionChange, fileOwners, sidebarTop, filesHeader, codeTop, search, onRevealHidden, filesRead, onRevealApplied }: ReviewLayoutProps) {
  const diffViewMode = useDiffViewMode()
  const sidebarShown = useAppSettings(state => state.reviewSidebar)
  const { defaultLayout, onLayoutChanged } = useDefaultLayout({ id: "review-sidebar", storage: localStorage, onlySaveAfterUserInteractions: true })
  const loadedFiles = loaded?.files
  const reviewFacts = loaded?.reviewFacts ?? null
  // the concerns of the plan, what no evidence joined as the tree, the mechanical ones in one group at the end, as Rust
  // joined, classed and ordered them
  const mechanical = useMemo(() => (reviewFacts === null ? NO_MECHANICAL : mechanicalKindsOf(reviewFacts)), [reviewFacts])
  const nodes = useMemo(() => {
    if (loadedFiles === undefined) {
      return []
    }
    return reviewFacts === null ? buildFileTree(loadedFiles) : buildReviewTree(loadedFiles, mechanical, reviewFacts.concerns)
  }, [loadedFiles, reviewFacts, mechanical])
  const files = useMemo(() => flattenTreeFiles(nodes), [nodes])
  const filterText = search?.text ?? ""
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
  // with the plan, the story of its units (spec/branch/review-units.spec.md) in the order of its concerns: each run of
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
    return presentNarrative(reviewFacts.units, shown, reviewFacts.api, reviewFacts.files, reviewFacts.concerns)
  }, [diffs, files, reviewFacts])
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

  // a row shows its first open section: of a file its own concern, of a file "also in" a concern its part there
  const selectInTree = useCallback(
    (file: CommitFile, key: string) => {
      select(file.path, key === file.path ? null : key)
      const section = rows.sectionOf.get(key)
      if (section === undefined) {
        code.current?.scrollToFile(file.path)
      } else {
        code.current?.scrollToSection(section)
      }
    },
    [rows, select],
  )

  // a place of a popover or of the card of the review plan: its file is selected as a row of the tree is, by its path
  // or its old path, and the diffs show its line; a file that a filter hides is revealed
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
  const navigator = useMemo<ReviewNavigator>(() => ({ show: selectChangedFile, showOutside }), [selectChangedFile, showOutside])

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
        // with the plan the diffs are the story: J and K step through its sections, the tree follows
        if (reviewFacts !== null) {
          const next = code.current?.moveSection(event.code === "KeyJ" ? 1 : -1)
          if (next !== undefined) {
            selectShown(next.key, next.path)
          }
          break
        }
        const next = files[selectedIndex + (event.code === "KeyJ" ? 1 : -1)]
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
        const path = code.current?.moveChange(event.code === "KeyN" ? 1 : -1)
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
                  {sidebarTop}
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
                  />
                )}
              </div>
            </div>
          </ResizablePanel>
        )}
        {sidebarShown && <ResizableHandle />}
        <ResizablePanel id="code" minSize="40">
          <div className="flex h-full flex-col">
            {/* a filter that hides every file keeps the card: a click reveals the file */}
            <SelectReviewFileContext value={loaded === null && !filesRead ? null : navigator}>{codeTop}</SelectReviewFileContext>
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
                  onVisibleSectionChange={selectShown}
                  onFirstScreen={showWindow}
                  fileOwners={fileOwners}
                />
              </div>
            )}
          </div>
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  )
}
