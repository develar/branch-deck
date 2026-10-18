import { useCallback, useEffect, useEffectEvent, useMemo, useRef, useState } from "react"
import type { KeyboardEvent, ReactNode } from "react"
import { useDefaultLayout } from "react-resizable-panels"
import { Button } from "@/components/ui/button"
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "@/components/ui/resizable"
import { CommitReviewCode, PendingFileHeaders } from "@/components/diff/CommitReviewCode"
import type { CommitReviewCodeHandle } from "@/components/diff/CommitReviewCode"
import { FileTree } from "@/components/files/FileTree"
import { Callout } from "@/components/shared/Callout"
import { LoadingLine } from "@/components/shared/LoadingLine"
import { SearchField } from "@/components/shared/SearchField"
import { SectionLabel } from "@/components/shared/SectionLabel"
import type { CommitFile, CommitPatch } from "@/lib/bindings"
import type { DiffSubject } from "@/lib/commit-files"
import { diffSubjectId, parseDiffSubjectId } from "@/lib/commit-files"
import { getDiffWorkerPool } from "@/lib/diff/diff-engine"
import { prepareFirstDiffs } from "@/lib/diff/review-diffs"
import { FILE_FILTER_MIN_FILES, filterFilesByText, formatNoFileMatch } from "@/lib/file-filter"
import { buildFileTree, flattenTreeFiles, getInitiallyCollapsed, revealInTree, toggleInSet } from "@/lib/file-tree"
import { isTypingTarget } from "@/lib/preview-keys"
import type { ReviewPosition } from "@/lib/review-position"
import { markSubWindowContentReady, markSubWindowOpenPhase } from "@/lib/sub-window"
import { getFilesOfCommit, loadFilesOfSubject, useFilesOfCommit } from "@/stores/commit-files"
import { loadSubjectPatch, useCommitPatch } from "@/stores/commit-patch"
import { useConflictViewerSettings } from "@/stores/settings/diff-view-settings"

// the diff engine starts with the page of a review, beside the reads of its files and patch, not after them
getDiffWorkerPool()

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
  /** The header of the files in the sidebar: "18 files" by default, the owner chips of a branch review */
  filesHeader?: ReactNode
  /** Above the diffs: the commit a range shows */
  codeTop?: ReactNode
  /** What ⌥↑/⌥↓ show next: read once the subject is, so the step is instant */
  neighbours?: readonly DiffSubject[]
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
}: CommitReviewProps) {
  const subjectId = diffSubjectId(subject)
  const commitFiles = useFilesOfCommit(subjectId)
  const entry = useCommitPatch(subjectId)
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

  // by the id: the subject object may be new on every render
  useEffect(() => {
    const current = parseDiffSubjectId(subjectId)
    void loadFilesOfSubject(current).then(() => {
      if (getFilesOfCommit(subjectId) === undefined) {
        setFilesFailed(subjectId)
      }
    })
    loadSubjectPatch(current)
  }, [subjectId])

  const patchLoaded = entry?.kind === "loaded"
  useEffect(() => {
    if (commitFiles !== undefined) {
      markSubWindowOpenPhase("files")
    }
  }, [commitFiles])
  const neighbourIds = neighbours?.map(diffSubjectId).join("\n") ?? ""
  useEffect(() => {
    if (!patchLoaded) {
      return
    }
    markSubWindowOpenPhase("patch")
    for (const id of neighbourIds === "" ? [] : neighbourIds.split("\n")) {
      const neighbour = parseDiffSubjectId(id)
      void loadFilesOfSubject(neighbour)
      loadSubjectPatch(neighbour)
    }
  }, [patchLoaded, neighbourIds])

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
      key: pathFilter === null ? subjectId : `${subjectId}|${[...pathFilter].join("\n")}`,
      files: shownFiles,
      totalCount: filteredFiles?.length ?? shownFiles.length,
      patch: entry?.kind === "loaded" ? entry.patch : null,
      initialPath: initialPath !== null && shownFiles.some(file => file.path === initialPath) ? initialPath : null,
    }
  }
  // an error and an empty subject are the content: the window shows them
  const settledWithoutDiffs = message !== null && shownFiles !== undefined
  useEffect(() => {
    if (settledWithoutDiffs || filesFailed === subjectId) {
      markSubWindowContentReady()
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

/** The files of the shown subject once read, and its patch once read too: the tree does not wait for the patch */
interface LoadedReview {
  /** The subject and the filter: a new key starts the tree and the diffs over */
  key: string
  /** The files the search kept */
  files: readonly CommitFile[]
  /** The files before the search: "Files 2 of 18" */
  totalCount: number
  patch: CommitPatch | null
  initialPath: string | null
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
  filesHeader: ReactNode
  codeTop: ReactNode
  search: FileSearch | null
}

/** The tree state of one loaded subject (`LoadedReview.key`) and one search text */
interface TreeSelection {
  key: string | null
  filterText: string
  collapsed: ReadonlySet<string>
  selectedPath: string | null
}

function initialSelection(loaded: LoadedReview | null, filterText: string, nodes: ReturnType<typeof buildFileTree>, fileCount: number, firstPath: string | null): TreeSelection {
  if (loaded === null) {
    return { key: null, filterText, collapsed: new Set(), selectedPath: null }
  }
  // what a search found shows whole: its directories open
  const initial = filterText.trim() === "" ? getInitiallyCollapsed(nodes, fileCount) : new Set<string>()
  return {
    key: loaded.key,
    filterText,
    collapsed: loaded.initialPath === null ? initial : revealInTree(initial, loaded.initialPath),
    selectedPath: loaded.initialPath ?? firstPath,
  }
}

/** Another search text: the directories as a new tree has them, the selected file kept while the search keeps it */
function searchedSelection(previous: TreeSelection, filterText: string, nodes: ReturnType<typeof buildFileTree>, files: readonly CommitFile[]): TreeSelection {
  const selectedPath = files.some(file => file.path === previous.selectedPath) ? previous.selectedPath : (files[0]?.path ?? null)
  const collapsed = filterText.trim() === "" ? getInitiallyCollapsed(nodes, files.length) : new Set<string>()
  return { key: previous.key, filterText, collapsed: selectedPath === null ? collapsed : revealInTree(collapsed, selectedPath), selectedPath }
}

// the sidebar takes 30% until the divider is dragged; never narrower than the names need
const initialLayout = { sidebar: 30, code: 70 }

/**
 * The sidebar beside the diffs, in every state of the review: the parts of the sidebar (the commits, the header of the
 * files) stay mounted, only the tree and the diffs start over for another subject or filter.
 */
function ReviewLayout({ loaded, message, onPositionChange, fileOwners, sidebarTop, filesHeader, codeTop, search }: ReviewLayoutProps) {
  const diffViewMode = useConflictViewerSettings(state => state.conflictDiffViewMode)
  const { defaultLayout, onLayoutChanged } = useDefaultLayout({ id: "review-sidebar", storage: localStorage, onlySaveAfterUserInteractions: true })
  const loadedFiles = loaded?.files
  const nodes = useMemo(() => (loadedFiles === undefined ? [] : buildFileTree(loadedFiles)), [loadedFiles])
  const files = useMemo(() => flattenTreeFiles(nodes), [nodes])
  const filterText = search?.text ?? ""
  const [selection, setSelection] = useState<TreeSelection>(() => initialSelection(loaded, filterText, nodes, files.length, files[0]?.path ?? null))
  const loadedKey = loaded?.key ?? null
  if (selection.key !== loadedKey) {
    setSelection(initialSelection(loaded, filterText, nodes, files.length, files[0]?.path ?? null))
  } else if (selection.filterText !== filterText) {
    setSelection(searchedSelection(selection, filterText, nodes, files))
  }
  const { collapsed, selectedPath } = selection
  const code = useRef<CommitReviewCodeHandle>(null)
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

  // the diffs mount once their first screen is ready (`prepareFirstDiffs`), so their first frame is highlighted
  const patch = loaded?.patch ?? null
  const initialPath = loaded?.initialPath ?? null
  const [prepared, setPrepared] = useState<{ key: string; patch: CommitPatch } | null>(null)
  const isPrepared = loaded !== null && patch !== null && prepared?.key === loaded.key && prepared.patch === patch
  useEffect(() => {
    if (loadedKey === null || patch === null) {
      return
    }
    let current = true
    void prepareFirstDiffs(patch, files, initialPath).then(() => {
      if (current) {
        setPrepared({ key: loadedKey, patch })
      }
    })
    return () => {
      current = false
    }
  }, [loadedKey, patch, files, initialPath])
  // the diffs rendered their first frame in their mount: the window shows it
  useEffect(() => {
    if (isPrepared) {
      markSubWindowContentReady()
    }
  }, [isPrepared])

  const toggleDirectory = useCallback((key: string) => setSelection(previous => ({ ...previous, collapsed: toggleInSet(previous.collapsed, key) })), [])

  // the tree shows the selected file: its directories open
  const select = useCallback((path: string) => {
    setSelection(previous => ({ ...previous, selectedPath: path, collapsed: revealInTree(previous.collapsed, path) }))
  }, [])

  const selectInTree = useCallback(
    (file: CommitFile) => {
      select(file.path)
      code.current?.scrollToFile(file.path)
    },
    [select],
  )

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
        const next = files[selectedIndex + (event.code === "KeyJ" ? 1 : -1)]
        if (next !== undefined) {
          selectInTree(next)
        }
        break
      }
      case "KeyN":
      case "KeyP": {
        const path = code.current?.moveChange(event.code === "KeyN" ? 1 : -1)
        if (path !== undefined) {
          select(path)
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
      <ResizablePanelGroup orientation="horizontal" defaultLayout={defaultLayout ?? initialLayout} onLayoutChanged={onLayoutChanged}>
        <ResizablePanel id="sidebar" minSize="16rem" maxSize="50">
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
              {filesHeader ??
                (loaded !== null && (
                  <SectionLabel title="Files" count={loaded.files.length} of={loaded.files.length === loaded.totalCount ? undefined : loaded.totalCount} className="flex-1" />
                ))}
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
                  selectedKey={selectedPath}
                  onSelectFile={selectInTree}
                  fileOwners={fileOwners}
                />
              )}
            </div>
          </div>
        </ResizablePanel>
        <ResizableHandle />
        <ResizablePanel id="code" minSize="40">
          <div className="flex h-full flex-col">
            {codeTop}
            {loaded === null ? (
              <div className="min-h-0 flex-1 overflow-y-auto">{message}</div>
            ) : !isPrepared || patch === null ? (
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
                  ref={code}
                  patch={patch}
                  files={files}
                  diffViewMode={diffViewMode}
                  initialPath={loaded.initialPath}
                  onVisibleFileChange={select}
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
