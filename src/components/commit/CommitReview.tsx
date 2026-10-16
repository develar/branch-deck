import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import type { KeyboardEvent, ReactNode } from "react"
import { useDefaultLayout } from "react-resizable-panels"
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "@/components/ui/resizable"
import { Spinner } from "@/components/ui/spinner"
import type { CommitReviewCodeHandle } from "@/components/diff/CommitReviewCode"
import { LazyCommitReviewCode } from "@/components/diff/LazyDiffViews"
import { FileTree } from "@/components/files/FileTree"
import { Callout } from "@/components/shared/Callout"
import { DiffViewToggle } from "@/components/shared/DiffViewToggle"
import type { CommitFile, CommitPatch } from "@/lib/bindings"
import type { DiffSubject } from "@/lib/commit-files"
import { diffSubjectId, parseDiffSubjectId } from "@/lib/commit-files"
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
  /** The code owner of a file by its path, beside its name */
  fileOwners?: ReadonlyMap<string, string>
  /** What the range is, in the error: "Cannot read the changes of the pull request" */
  rangeNoun?: string
}

/**
 * The review of a commit or of the range of a pull request (`base..head`): the directory tree of its files beside the
 * diffs of all of them in one virtualized view.
 * The tree follows the diffs as they scroll, a file selected in the tree scrolls the diffs to it. J and K step through
 * the files, N and P through the blocks of changed lines.
 */
export function CommitReview({ subject, initialPath, onPositionChange, pathFilter = null, filteredEmptyText, fileOwners, rangeNoun = "pull request" }: CommitReviewProps) {
  const subjectId = diffSubjectId(subject)
  const commitFiles = useFilesOfCommit(subjectId)
  const entry = useCommitPatch(subjectId)
  const [filesFailed, setFilesFailed] = useState<string | null>(null)
  const shownFiles = useMemo(() => commitFiles === undefined ? undefined : filterFiles(commitFiles.files, pathFilter), [commitFiles, pathFilter])

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

  let content: ReactNode
  if (filesFailed === subjectId || entry?.kind === "error") {
    content = (
      <div className="p-6">
        <Callout tone="destructive" title={`Cannot read the changes of the ${subject.kind === "commit" ? "commit" : rangeNoun}`}>{entry?.kind === "error" ? entry.message : null}</Callout>
      </div>
    )
  }
  else if (shownFiles === undefined || entry?.kind !== "loaded") {
    content = (
      <p className="flex items-center gap-2 p-6 text-sm text-muted-foreground">
        <Spinner className="size-4" />
        Loading changes…
      </p>
    )
  }
  else if (shownFiles.length === 0) {
    content = <p className="p-6 text-sm text-muted-foreground">{pathFilter !== null && filteredEmptyText !== undefined && commitFiles !== undefined && commitFiles.files.length > 0 ? filteredEmptyText : "No changed files"}</p>
  }
  else {
    // keyed by the subject: the selection and the collapsed directories belong to one commit or range, and to one filter
    content = (
      <ReviewLayout
        key={pathFilter === null ? subjectId : `${subjectId}|${[...pathFilter].join("\n")}`}
        commitFiles={shownFiles}
        patch={entry.patch}
        initialPath={initialPath !== null && shownFiles.some(file => file.path === initialPath) ? initialPath : null}
        onPositionChange={onPositionChange}
        fileOwners={fileOwners}
      />
    )
  }
  return (
    <ReviewRoot subjectId={subjectId} onPositionChange={onPositionChange} ready={shownFiles !== undefined && shownFiles.length > 0 && entry?.kind === "loaded"}>
      {content}
    </ReviewRoot>
  )
}

/**
 * The focusable frame of every state of the review: while another commit loads, has no files or cannot be read, the
 * focus stays here (the diffs that had it are gone), so the keys of the surface (⌥↑/⌥↓, Escape) still reach it
 */
function ReviewRoot({ subjectId, ready, onPositionChange, children }: { subjectId: string, ready: boolean, onPositionChange: (position: ReviewPosition) => void, children: ReactNode }) {
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

interface ReviewLayoutProps {
  commitFiles: readonly CommitFile[]
  patch: CommitPatch
  initialPath: string | null
  onPositionChange: (position: ReviewPosition) => void
  fileOwners: ReadonlyMap<string, string> | undefined
}

// the tree takes a quarter until the divider is dragged
const initialLayout = { tree: 25, code: 75 }

function ReviewLayout({ commitFiles, patch, initialPath, onPositionChange, fileOwners }: ReviewLayoutProps) {
  const diffViewMode = useConflictViewerSettings(state => state.conflictDiffViewMode)
  const { defaultLayout, onLayoutChanged } = useDefaultLayout({ id: "commit-review", storage: localStorage, onlySaveAfterUserInteractions: true })
  const nodes = useMemo(() => buildFileTree(commitFiles), [commitFiles])
  const files = useMemo(() => flattenTreeFiles(nodes), [nodes])
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(() => {
    const initial = getInitiallyCollapsed(nodes, files.length)
    return initialPath === null ? initial : revealInTree(initial, initialPath)
  })
  const [selectedPath, setSelectedPath] = useState<string | null>(initialPath ?? files[0]?.path ?? null)
  const code = useRef<CommitReviewCodeHandle>(null)

  const toggleDirectory = useCallback((key: string) => setCollapsed(previous => toggleInSet(previous, key)), [])

  // the tree shows the selected file: its directories open
  const select = useCallback((path: string) => {
    setSelectedPath(path)
    setCollapsed(previous => revealInTree(previous, path))
  }, [])

  const selectInTree = useCallback((file: CommitFile) => {
    select(file.path)
    code.current?.scrollToFile(file.path)
  }, [select])

  const selectedIndex = files.findIndex(file => file.path === selectedPath)
  useEffect(() => {
    onPositionChange({ path: selectedPath, index: selectedIndex, count: files.length })
  }, [onPositionChange, selectedPath, selectedIndex, files.length])

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.defaultPrevented || event.altKey || event.metaKey || event.ctrlKey || event.shiftKey) {
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
        <ResizablePanel id="tree" minSize="15" maxSize="50">
          <div className="flex h-full flex-col">
            <div className="flex items-center justify-between gap-2 px-3 py-2">
              <span className="text-xs text-muted-foreground">{`${files.length} ${files.length === 1 ? "file" : "files"}`}</span>
              <DiffViewToggle />
            </div>
            <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
              <FileTree
                mode="navigate"
                nodes={nodes}
                aria-label="Changed files"
                collapsed={collapsed}
                onToggleDirectory={toggleDirectory}
                selectedKey={selectedPath}
                onSelectFile={selectInTree}
                fileOwners={fileOwners}
              />
            </div>
          </div>
        </ResizablePanel>
        <ResizableHandle />
        <ResizablePanel id="code" minSize="40">
          <div data-testid="commit-review-code" className="h-full">
            <LazyCommitReviewCode ref={code} patch={patch} files={files} diffViewMode={diffViewMode} initialPath={initialPath} onVisibleFileChange={select} fileOwners={fileOwners} />
          </div>
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  )
}
