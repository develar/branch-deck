import { memo, useCallback, useEffect, useRef, useState } from "react"
import type { KeyboardEvent, ReactNode, RefObject } from "react"
import { ChevronRightIcon, FolderIcon, FolderOpenIcon } from "lucide-react"
import { cn } from "cn"
import { LineCounts } from "@/components/commit/CommitMetadata"
import { CopyButton } from "@/components/shared/CopyButton"
import type { CommitFile } from "@/lib/bindings"
import { formatFileStatus, getFileStatusLetter, getFileStatusTextClass } from "@/lib/commit-files"
import { formatTreeFileLabel, getVisibleRows } from "@/lib/file-tree"
import type { FileTreeDirectory, FileTreeFile, FileTreeNode } from "@/lib/file-tree"

interface FileTreeBaseProps {
  "nodes": readonly FileTreeNode[]
  "aria-label": string
  "collapsed": ReadonlySet<string>
  /** Must keep its identity: the rows are memoized */
  "onToggleDirectory": (key: string) => void
}

/** A file row expands its diff under it (the commit preview) */
export interface InlineFileTreeProps extends FileTreeBaseProps {
  mode: "inline"
  /** The files whose diff is shown under their row */
  expandedFiles: ReadonlySet<string>
  /** Must keep its identity: the rows are memoized */
  onToggleFile: (key: string) => void
  /** The diff of an expanded file, under its row */
  renderFileContent: (file: CommitFile) => ReactNode
  /** Enter on a file row; without it Enter toggles the diff, as Space does */
  onOpenFile?: (file: CommitFile) => void
}

/** A file row selects the file, and the selection follows the focus (the review, beside the diffs) */
export interface NavigationFileTreeProps extends FileTreeBaseProps {
  mode: "navigate"
  selectedKey: string | null
  /** Must keep its identity: the rows are memoized */
  onSelectFile: (file: CommitFile) => void
}

export type FileTreeProps = InlineFileTreeProps | NavigationFileTreeProps

/** The indent of a tree level */
const INDENT_REM = 1

/**
 * The changed files grouped by directory: a directory row collapses its subtree, a file row expands its diff under it
 * or selects the file. One row is in the tab order (roving focus): ↑/↓, Home and End move between the rows, → opens
 * and ← closes or goes to the parent, Space and Enter toggle. The keys the tree handles are `preventDefault`ed, so the
 * dialog around it leaves them alone; the others (letters, keys with a modifier) reach it.
 */
export function FileTree(props: FileTreeProps) {
  const { "nodes": nodes, "aria-label": label, collapsed, onToggleDirectory, mode } = props
  // a file of a navigation tree has no diff to open
  const expandedFiles = props.mode === "inline" ? props.expandedFiles : null
  const selectedKey = props.mode === "navigate" ? props.selectedKey : null
  const onToggleFile = props.mode === "inline" ? props.onToggleFile : undefined
  const onSelectFile = props.mode === "navigate" ? props.onSelectFile : undefined
  const onOpenFile = props.mode === "inline" ? props.onOpenFile : undefined
  const rows = getVisibleRows(nodes, collapsed)
  const [focusedKey, setFocusedKey] = useState<string | null>(null)
  // the row in the tab order: the focused one while it shows, then the selected one, otherwise the first
  const tabKey = [focusedKey, selectedKey].find(key => key !== null && rows.some(row => row.key === key)) ?? rows[0]?.key ?? null
  const rowElements = useRef(new Map<string, HTMLElement>())

  // a file selected elsewhere (the diffs scrolled, J and K) scrolls into view
  useEffect(() => {
    if (selectedKey !== null) {
      rowElements.current.get(selectedKey)?.scrollIntoView({ block: "nearest" })
    }
  }, [selectedKey])

  const focusRow = (node: FileTreeNode) => {
    setFocusedKey(node.key)
    rowElements.current.get(node.key)?.focus()
    // the selection follows the focus
    if (node.kind === "file") {
      onSelectFile?.(node.file)
    }
  }

  const toggle = useCallback((node: FileTreeNode) => {
    setFocusedKey(node.key)
    if (node.kind === "directory") {
      onToggleDirectory(node.key)
    }
    else if (onSelectFile !== undefined) {
      onSelectFile(node.file)
    }
    else {
      onToggleFile?.(node.key)
    }
  }, [onToggleDirectory, onToggleFile, onSelectFile])

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const target = event.target as HTMLElement
    const key = target.dataset.treeKey
    // only the keys of a row, not of the diff under it; a key with a modifier is the dialog's
    if (key === undefined || event.altKey || event.metaKey || event.ctrlKey || event.shiftKey) {
      return
    }
    const index = rows.findIndex(row => row.key === key)
    const node = rows[index]
    if (node === undefined) {
      return
    }
    const isOpen = isRowOpen(node, collapsed, expandedFiles)
    let handled = true
    switch (event.key) {
      case "ArrowDown":
      case "ArrowUp": {
        // also at the first and the last row: the arrows never leave the tree
        const next = rows[index + (event.key === "ArrowDown" ? 1 : -1)]
        if (next !== undefined) {
          focusRow(next)
        }
        break
      }
      case "Home":
        focusRow(rows[0]!)
        break
      case "End":
        focusRow(rows.at(-1)!)
        break
      case "ArrowRight":
        if (isOpen === false) {
          toggle(node)
        }
        else if (node.kind === "directory" && node.children[0] !== undefined) {
          focusRow(node.children[0])
        }
        break
      case "ArrowLeft": {
        const parent = rows.find(row => row.key === node.parentKey)
        if (isOpen === true) {
          toggle(node)
        }
        else if (parent !== undefined) {
          focusRow(parent)
        }
        break
      }
      case "Enter":
        if (node.kind === "file" && onOpenFile !== undefined) {
          onOpenFile(node.file)
        }
        else {
          toggle(node)
        }
        break
      case " ":
        toggle(node)
        break
      default:
        handled = false
    }
    if (handled) {
      event.preventDefault()
    }
  }

  return (
    <div role="tree" aria-label={label} className="flex flex-col" onKeyDown={handleKeyDown}>
      {rows.map(node => (
        <FileTreeRow
          key={node.key}
          node={node}
          isOpen={isRowOpen(node, collapsed, expandedFiles)}
          isSelected={mode === "navigate" ? node.key === selectedKey : undefined}
          inTabOrder={node.key === tabKey}
          rowElements={rowElements}
          onToggle={toggle}
          onFocusRow={setFocusedKey}
          content={props.mode === "inline" && node.kind === "file" && props.expandedFiles.has(node.key) ? props.renderFileContent(node.file) : null}
        />
      ))}
    </div>
  )
}

/** A directory is open unless collapsed; a file is open while its diff shows, `undefined` where it has none to show */
function isRowOpen(node: FileTreeNode, collapsed: ReadonlySet<string>, expandedFiles: ReadonlySet<string> | null): boolean | undefined {
  if (node.kind === "directory") {
    return !collapsed.has(node.key)
  }
  return expandedFiles === null ? undefined : expandedFiles.has(node.key)
}

interface FileTreeRowProps {
  node: FileTreeNode
  isOpen: boolean | undefined
  /** In a navigation tree, whether the row is the selected file; `undefined` in an inline tree */
  isSelected: boolean | undefined
  inTabOrder: boolean
  rowElements: RefObject<Map<string, HTMLElement>>
  onToggle: (node: FileTreeNode) => void
  onFocusRow: (key: string) => void
  /** The diff under an expanded file */
  content: ReactNode
}

const FileTreeRow = memo(function FileTreeRow({ node, isOpen, isSelected, inTabOrder, rowElements, onToggle, onFocusRow, content }: FileTreeRowProps) {
  const indent = `${node.depth * INDENT_REM}rem`
  const description = node.kind === "directory"
    ? `${node.fileCount === 1 ? "1 file" : `${node.fileCount} files`}, +${node.additions} −${node.deletions}`
    : `${formatFileStatus(node.file.status)}, +${node.file.additions} −${node.file.deletions}`
  return (
    <div
      ref={(element) => {
        if (element === null) {
          rowElements.current.delete(node.key)
        }
        else {
          rowElements.current.set(node.key, element)
        }
      }}
      role="treeitem"
      aria-level={node.depth + 1}
      aria-expanded={isOpen}
      aria-selected={node.kind === "file" ? isSelected : undefined}
      aria-label={node.name}
      aria-description={description}
      tabIndex={inTabOrder ? 0 : -1}
      data-tree-key={node.key}
      className="group/tree-item flex flex-col outline-none"
      onFocus={(event) => {
        if (event.target === event.currentTarget) {
          onFocusRow(node.key)
        }
      }}
    >
      <div
        className={cn(
          "group/row flex h-6 cursor-default items-center gap-1.5 rounded-md pr-1 text-sm transition-colors select-none hover:bg-muted group-focus-visible/tree-item:ring-2 group-focus-visible/tree-item:ring-ring/50 group-focus-visible/tree-item:ring-inset",
          isSelected && "bg-primary/15 hover:bg-primary/15",
        )}
        style={{ paddingLeft: `calc(${indent} + 0.25rem)` }}
        onClick={() => onToggle(node)}
      >
        {isOpen === undefined
          // a file of a navigation tree opens nothing: the space keeps the names aligned
          ? <span className="size-3.5 shrink-0" />
          : <ChevronRightIcon className={cn("size-3.5 shrink-0 text-muted-foreground transition-transform duration-150", isOpen && "rotate-90")} />}
        {node.kind === "directory" ? <DirectoryRowContent node={node} isOpen={isOpen === true} /> : <FileRowContent node={node} />}
      </div>
      {content !== null && (
        <div className="my-1 overflow-hidden rounded-lg border border-border" style={{ marginLeft: indent }}>
          {content}
        </div>
      )}
    </div>
  )
})

function DirectoryRowContent({ node, isOpen }: { node: FileTreeDirectory, isOpen: boolean }) {
  const Icon = isOpen ? FolderOpenIcon : FolderIcon
  return (
    <>
      <Icon className="size-4 shrink-0 text-muted-foreground" />
      <span className="min-w-0 flex-1 truncate" title={node.key.slice(0, -1)}>{node.name}</span>
      <span className="shrink-0 text-xs text-muted-foreground" data-testid="directory-line-counts">
        <LineCounts additions={node.additions} deletions={node.deletions} colored={false} />
      </span>
    </>
  )
}

function FileRowContent({ node }: { node: FileTreeFile }) {
  const { file } = node
  const label = formatTreeFileLabel(file)
  return (
    <>
      <span className="flex min-w-0 flex-1 items-baseline gap-1.5" title={file.oldPath === null ? file.path : `${file.oldPath} → ${file.path}`}>
        <span className={cn("truncate", getFileStatusTextClass(file.status))}>{label.name}</span>
        {label.from !== null && <span className="min-w-0 truncate text-xs text-muted-foreground">{`← ${label.from}`}</span>}
      </span>
      <CopyButton
        text={() => file.path}
        tooltip="Copy path"
        label={`Copy path of ${label.name}`}
        // hidden without delay, shown on row hover after a delay
        className="duration-200 ease-out group-hover/row:opacity-100 group-hover/row:delay-300"
      />
      {(file.additions > 0 || file.deletions > 0) && (
        <span className="shrink-0 text-xs">
          <LineCounts data-testid="file-line-counts" additions={file.additions} deletions={file.deletions} colored />
        </span>
      )}
      <span aria-hidden className={cn("w-3 shrink-0 text-center font-mono text-xs", getFileStatusTextClass(file.status), "no-underline")}>
        {getFileStatusLetter(file.status)}
      </span>
    </>
  )
}
