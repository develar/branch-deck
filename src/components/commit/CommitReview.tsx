import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import type { KeyboardEvent, ReactNode } from "react"
import { useDefaultLayout } from "react-resizable-panels"
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "@/components/ui/resizable"
import { Spinner } from "@/components/ui/spinner"
import type { CommitReviewCodeHandle } from "@/components/diff/CommitReviewCode"
import { LazyCommitReviewCode } from "@/components/diff/LazyDiffViews"
import { FileTree } from "@/components/files/FileTree"
import { Callout } from "@/components/shared/Callout"
import type { CommitFile, CommitPatch } from "@/lib/bindings"
import type { DiffSubject } from "@/lib/commit-files"
import { diffSubjectId, formatFileCount, parseDiffSubjectId } from "@/lib/commit-files"
import { buildFileTree, flattenTreeFiles, getInitiallyCollapsed, revealInTree, toggleInSet } from "@/lib/file-tree"
import type { ReviewPosition } from "@/lib/review-position"
import { getFilesOfCommit, loadFilesOfSubject, useFilesOfCommit } from "@/stores/commit-files"
import { loadSubjectPatch, useCommitPatch } from "@/stores/commit-patch"
import { useConflictViewerSettings } from "@/stores/settings/conflictViewer"

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
}

/**
 * The review of a commit or of the range of a pull request (`base..head`): a sidebar (the commits of a range, the
 * header of the files, their directory tree) beside the diffs of all of them in one virtualized view. The split is
 * there in every state, so what the sidebar holds keeps its focus while another commit or filter loads.
 * The tree follows the diffs as they scroll, a file selected in the tree scrolls the diffs to it. J and K step through
 * the files, N and P through the blocks of changed lines.
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
}: CommitReviewProps) {
  const subjectId = diffSubjectId(subject)
  const commitFiles = useFilesOfCommit(subjectId)
  const entry = useCommitPatch(subjectId)
  const [filesFailed, setFilesFailed] = useState<string | null>(null)
  const shownFiles = useMemo(() => (commitFiles === undefined ? undefined : filterFiles(commitFiles.files, pathFilter)), [commitFiles, pathFilter])

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
  } else if (shownFiles === undefined || entry?.kind !== "loaded") {
    message = (
      <p className="flex items-center gap-2 p-6 text-sm text-muted-foreground">
        <Spinner className="size-4" />
        Loading changes…
      </p>
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
      patch: entry.patch,
      initialPath: initialPath !== null && shownFiles.some(file => file.path === initialPath) ? initialPath : null,
    }
  }
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

/** The files and the diffs of the shown subject, once read */
interface LoadedReview {
  /** The subject and the filter: a new key starts the tree and the diffs over */
  key: string
  files: readonly CommitFile[]
  patch: CommitPatch
  initialPath: string | null
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
}

/** The tree state of one loaded subject (`LoadedReview.key`) */
interface TreeSelection {
  key: string | null
  collapsed: ReadonlySet<string>
  selectedPath: string | null
}

function initialSelection(loaded: LoadedReview | null, nodes: ReturnType<typeof buildFileTree>, fileCount: number, firstPath: string | null): TreeSelection {
  if (loaded === null) {
    return { key: null, collapsed: new Set(), selectedPath: null }
  }
  const initial = getInitiallyCollapsed(nodes, fileCount)
  return {
    key: loaded.key,
    collapsed: loaded.initialPath === null ? initial : revealInTree(initial, loaded.initialPath),
    selectedPath: loaded.initialPath ?? firstPath,
  }
}

// the sidebar takes 30% until the divider is dragged; never narrower than the names need
const initialLayout = { sidebar: 30, code: 70 }

/**
 * The sidebar beside the diffs, in every state of the review: the parts of the sidebar (the commits, the header of the
 * files) stay mounted, only the tree and the diffs start over for another subject or filter.
 */
function ReviewLayout({ loaded, message, onPositionChange, fileOwners, sidebarTop, filesHeader, codeTop }: ReviewLayoutProps) {
  const diffViewMode = useConflictViewerSettings(state => state.conflictDiffViewMode)
  const { defaultLayout, onLayoutChanged } = useDefaultLayout({ id: "review-sidebar", storage: localStorage, onlySaveAfterUserInteractions: true })
  const loadedFiles = loaded?.files
  const nodes = useMemo(() => (loadedFiles === undefined ? [] : buildFileTree(loadedFiles)), [loadedFiles])
  const files = useMemo(() => flattenTreeFiles(nodes), [nodes])
  const [selection, setSelection] = useState<TreeSelection>(() => initialSelection(loaded, nodes, files.length, files[0]?.path ?? null))
  const loadedKey = loaded?.key ?? null
  if (selection.key !== loadedKey) {
    setSelection(initialSelection(loaded, nodes, files.length, files[0]?.path ?? null))
  }
  const { collapsed, selectedPath } = selection
  const code = useRef<CommitReviewCodeHandle>(null)

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
    if (loaded === null || event.defaultPrevented || event.altKey || event.metaKey || event.ctrlKey || event.shiftKey) {
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
            <div
              data-review-keeps-focus=""
              className="flex min-h-10 shrink-0 flex-wrap items-center gap-1.5 px-3 py-2 text-xs text-muted-foreground"
              data-testid="review-files-header"
            >
              {filesHeader ?? (loaded !== null && formatFileCount(loaded.files.length))}
            </div>
            <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
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
            ) : (
              <div data-testid="commit-review-code" className="min-h-0 flex-1">
                <LazyCommitReviewCode
                  key={loaded.key}
                  ref={code}
                  patch={loaded.patch}
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
