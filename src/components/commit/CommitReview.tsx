import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import type { KeyboardEvent } from "react"
import { useDefaultLayout } from "react-resizable-panels"
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "@/components/ui/resizable"
import { Spinner } from "@/components/ui/spinner"
import type { CommitReviewCodeHandle } from "@/components/diff/CommitReviewCode"
import { LazyCommitReviewCode } from "@/components/diff/LazyDiffViews"
import { FileTree } from "@/components/files/FileTree"
import { Callout } from "@/components/shared/Callout"
import { DiffViewToggle } from "@/components/shared/DiffViewToggle"
import type { CommitFile, CommitFiles, CommitPatch } from "@/lib/bindings"
import { buildFileTree, flattenTreeFiles, getInitiallyCollapsed, revealInTree, toggleInSet } from "@/lib/file-tree"
import { getFilesOfCommit, loadFilesOfCommit, useFilesOfCommit } from "@/stores/commit-files"
import { loadCommitPatch, useCommitPatch } from "@/stores/commit-patch"
import { useConflictViewerSettings } from "@/stores/settings/conflictViewer"

export interface CommitReviewProps {
  commitId: string
  /** The file shown first; the first file of the tree by default */
  initialPath: string | null
  /** "File 3 / 12": the selected file, for the footer */
  onPositionChange: (position: string) => void
}

/**
 * The review of a commit: the directory tree of its files beside the diffs of all of them in one virtualized view.
 * The tree follows the diffs as they scroll, a file selected in the tree scrolls the diffs to it. J and K step through
 * the files, N and P through the blocks of changed lines.
 */
export function CommitReview({ commitId, initialPath, onPositionChange }: CommitReviewProps) {
  const commitFiles = useFilesOfCommit(commitId)
  const entry = useCommitPatch(commitId)
  const [filesFailed, setFilesFailed] = useState<string | null>(null)

  useEffect(() => {
    void loadFilesOfCommit(commitId).then(() => {
      if (getFilesOfCommit(commitId) === undefined) {
        setFilesFailed(commitId)
      }
    })
    loadCommitPatch(commitId)
  }, [commitId])

  if (filesFailed === commitId || entry?.kind === "error") {
    return (
      <div className="p-6">
        <Callout tone="destructive" title="Cannot read the changes of the commit">{entry?.kind === "error" ? entry.message : null}</Callout>
      </div>
    )
  }
  if (commitFiles === undefined || entry?.kind !== "loaded") {
    return (
      <p className="flex items-center gap-2 p-6 text-sm text-muted-foreground">
        <Spinner className="size-4" />
        Loading changes…
      </p>
    )
  }
  if (commitFiles.files.length === 0) {
    return <p className="p-6 text-sm text-muted-foreground">No changed files</p>
  }
  // keyed by the commit: the selection and the collapsed directories belong to one commit
  return <ReviewLayout key={commitId} commitFiles={commitFiles} patch={entry.patch} initialPath={initialPath} onPositionChange={onPositionChange} />
}

interface ReviewLayoutProps {
  commitFiles: CommitFiles
  patch: CommitPatch
  initialPath: string | null
  onPositionChange: (position: string) => void
}

// the tree takes a quarter until the divider is dragged
const initialLayout = { tree: 25, code: 75 }

function ReviewLayout({ commitFiles, patch, initialPath, onPositionChange }: ReviewLayoutProps) {
  const diffViewMode = useConflictViewerSettings(state => state.conflictDiffViewMode)
  const { defaultLayout, onLayoutChanged } = useDefaultLayout({ id: "commit-review", storage: localStorage, onlySaveAfterUserInteractions: true })
  const nodes = useMemo(() => buildFileTree(commitFiles.files), [commitFiles])
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
    onPositionChange(`File ${selectedIndex + 1} / ${files.length}`)
  }, [onPositionChange, selectedIndex, files.length])

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
    <div data-testid="commit-review" className="h-full min-h-0" onKeyDown={handleKeyDown}>
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
              />
            </div>
          </div>
        </ResizablePanel>
        <ResizableHandle />
        <ResizablePanel id="code" minSize="40">
          <div data-testid="commit-review-code" className="h-full">
            <LazyCommitReviewCode ref={code} patch={patch} files={files} diffViewMode={diffViewMode} initialPath={initialPath} onVisibleFileChange={select} />
          </div>
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  )
}
