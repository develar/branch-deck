import { useCallback, useEffect, useEffectEvent, useImperativeHandle, useMemo, useRef, useState } from "react"
import type { Ref } from "react"
import { CodeView, WorkerPoolContext } from "@pierre/diffs/react"
import type { CodeViewHandle } from "@pierre/diffs/react"
import type { CodeView as CodeViewInstance, CodeViewDiffItem, CodeViewItem } from "@pierre/diffs"
import { ChevronRightIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { LineCounts } from "@/components/commit/CommitMetadata"
import { CopyButton } from "@/components/shared/CopyButton"
import type { CommitFile, CommitPatch } from "@/lib/bindings"
import { getFileStatusLetter, getFileStatusTextClass } from "@/lib/commit-files"
import { getChangeTargets } from "@/lib/diff/change-targets"
import type { ChangeTarget } from "@/lib/diff/change-targets"
import { getCommitFileDiffs, loadCommitFileContents } from "@/lib/diff/commit-patch"
import type { CommitFileDiff } from "@/lib/diff/commit-patch"
import { BASE_DIFF_OPTIONS, getDiffWorkerPool } from "@/lib/diff/diff-engine"
import { formatTreeFileLabel } from "@/lib/file-tree"
import type { ConflictDiffViewMode } from "@/stores/settings/conflictViewer"

/** The height of a file header: `h-9` of `ReviewFileHeader`, the metric the virtualizer lays out with */
const HEADER_HEIGHT = 36

/** A file with more changed lines starts collapsed: its header is enough to know it is there */
export const REVIEW_COLLAPSE_CHANGED_LINES = 1000

/** Lock files and generated files start collapsed */
const GENERATED_FILE = /(^|\/)(pnpm-lock\.yaml|package-lock\.json|yarn\.lock|Cargo\.lock|Gemfile\.lock|poetry\.lock|go\.sum)$|\.(min\.js|min\.css|snap|map)$/

const lineCountFormat = new Intl.NumberFormat("en")

export interface CommitReviewCodeHandle {
  /** Shows the file at the top */
  scrollToFile: (path: string) => void
  /**
   * Shows the next (1) or previous (-1) block of changed lines, from the last one shown in the file at the top; into
   * the next file at the end of one. Collapsed files are skipped. Returns the path of the file it shows
   */
  moveChange: (direction: 1 | -1) => string | undefined
  focus: () => void
}

export interface CommitReviewCodeProps {
  ref?: Ref<CommitReviewCodeHandle>
  patch: CommitPatch
  /** In the order of the tree */
  files: readonly CommitFile[]
  diffViewMode: ConflictDiffViewMode
  /** The file shown first, at the top */
  initialPath: string | null
  /** The file whose header is at the top, as the diffs scroll */
  onVisibleFileChange: (path: string) => void
  /** The code owner of a file by its path, in its header */
  fileOwners?: ReadonlyMap<string, string>
}

/** Why a file shows no lines, under its header */
function getNotice(entry: CommitFileDiff): string | null {
  if (entry.file.binary) {
    return "Binary file"
  }
  if (entry.omittedLines !== null) {
    return `Too large to show: ${lineCountFormat.format(entry.omittedLines)} changed lines`
  }
  if (entry.fileDiff === undefined || entry.fileDiff.hunks.length === 0) {
    return "No changed lines"
  }
  return null
}

function startsCollapsed(entry: CommitFileDiff): boolean {
  return getNotice(entry) !== null || entry.file.additions + entry.file.deletions > REVIEW_COLLAPSE_CHANGED_LINES || GENERATED_FILE.test(entry.file.path)
}

/**
 * The diffs of all files of a commit in one virtualized view (Pierre's `CodeView`): a sticky header per file, the
 * unchanged lines expandable, lock and generated files and files over 1,000 changed lines collapsed.
 */
export function CommitReviewCode({ ref, patch, files, diffViewMode, initialPath, onVisibleFileChange, fileOwners }: CommitReviewCodeProps) {
  const entries = useMemo(() => getCommitFileDiffs(patch, files).filter(entry => entry.fileDiff !== undefined), [patch, files])
  const byPath = useMemo(() => new Map(entries.map(entry => [entry.file.path, entry])), [entries])
  const initiallyCollapsed = useMemo(() => new Set(entries.filter(startsCollapsed).map(entry => entry.file.path)), [entries])
  // how often each file was collapsed or expanded: its state, and the version CodeView renders it again for
  const [toggleCounts, setToggleCounts] = useState<ReadonlyMap<string, number>>(() => new Map())
  const isCollapsed = useCallback((path: string) => initiallyCollapsed.has(path) !== ((toggleCounts.get(path) ?? 0) % 2 === 1), [initiallyCollapsed, toggleCounts])
  const codeView = useRef<CodeViewHandle<undefined, undefined>>(null)
  const container = useRef<HTMLDivElement>(null)
  const visiblePath = useRef<string | undefined>(initialPath ?? entries[0]?.file.path)
  // the block of changed lines N and P stepped to last, in the file at the top
  const changeCursor = useRef<{ path: string, index: number } | null>(null)
  // after a scroll to a file or a change, the file at the top is not reported until the user scrolls: the last files
  // of a commit cannot reach the top, and the file above them would take the selection back
  const followScroll = useRef(true)
  const targetsByPath = useMemo(() => new Map(entries.map(entry => [entry.file.path, entry.fileDiff === undefined ? [] : getChangeTargets(entry.fileDiff)])), [entries])

  const items = useMemo<CodeViewItem<undefined>[]>(() => entries.map((entry): CodeViewDiffItem<undefined> => ({
    id: entry.file.path,
    type: "diff",
    fileDiff: entry.fileDiff!,
    collapsed: isCollapsed(entry.file.path),
    version: toggleCounts.get(entry.file.path) ?? 0,
  })), [entries, isCollapsed, toggleCounts])

  const toggleFile = useCallback((path: string) => {
    setToggleCounts(previous => new Map(previous).set(path, (previous.get(path) ?? 0) + 1))
  }, [])

  const options = useMemo(() => ({
    ...BASE_DIFF_OPTIONS,
    diffStyle: diffViewMode,
    hunkSeparators: "line-info",
    loadDiffFiles: loadCommitFileContents,
    stickyHeaders: true,
    itemMetrics: { diffHeaderHeight: HEADER_HEIGHT },
  } as const), [diffViewMode])

  // only when the review opens: a later file is the tree's or the keys'
  const scrollToInitialFile = useEffectEvent(() => {
    if (initialPath !== null) {
      followScroll.current = false
      codeView.current?.scrollTo({ type: "item", id: initialPath, align: "start", behavior: "instant" })
    }
  })

  // the diffs take the focus, so the arrows, PageUp and PageDown scroll them; the review opens at its file
  useEffect(() => {
    const element = container.current
    const follow = () => {
      followScroll.current = true
    }
    if (element !== null) {
      element.tabIndex = 0
      element.setAttribute("aria-label", "Changes of the commit")
      // from the review, from what holds it or from the body, and from anything of its dialog (the dialog focused its
      // close button before the review was there), but not from the commit column beside it, which keeps its keys
      const active = document.activeElement
      const root = element.closest("[data-review-root]")
      const dialog = element.closest("[role=dialog]")
      if (active === null || active === document.body || root === null || root.contains(active) || active.contains(root)
        || (dialog !== null && dialog.contains(active) && active.closest("[role=listbox]") === null)) {
        element.focus()
      }
      // what the user scrolls with: the wheel, a touch, the scroll bar, the keys
      for (const type of ["wheel", "touchstart", "pointerdown", "keydown"] as const) {
        element.addEventListener(type, follow, { passive: true })
      }
    }
    scrollToInitialFile()
    return () => {
      for (const type of ["wheel", "touchstart", "pointerdown", "keydown"] as const) {
        element?.removeEventListener(type, follow)
      }
    }
  }, [])

  const scrollToTarget = (path: string, target: ChangeTarget) => {
    followScroll.current = false
    codeView.current?.scrollTo({ type: "line", id: path, lineNumber: target.lineNumber, side: target.side, align: "start", offset: HEADER_HEIGHT + 8, behavior: "instant" })
  }

  useImperativeHandle(ref, () => ({
    scrollToFile: (path) => {
      changeCursor.current = { path, index: -1 }
      visiblePath.current = path
      followScroll.current = false
      codeView.current?.scrollTo({ type: "item", id: path, align: "start", behavior: "instant" })
    },
    moveChange: (direction) => {
      const order = entries.map(entry => entry.file.path)
      const start = changeCursor.current ?? { path: visiblePath.current ?? order[0] ?? "", index: -1 }
      let fileIndex = order.indexOf(start.path)
      let index = start.index + direction
      // from the top of a file going back: the last block of the file before it
      if (direction === -1 && start.index === -1) {
        fileIndex--
        index = Number.MAX_SAFE_INTEGER
      }
      while (fileIndex >= 0 && fileIndex < order.length) {
        const path = order[fileIndex]!
        const targets = isCollapsed(path) ? [] : (targetsByPath.get(path) ?? [])
        const clamped = index === Number.MAX_SAFE_INTEGER ? targets.length - 1 : index
        const target = targets[clamped]
        if (target !== undefined && clamped >= 0) {
          changeCursor.current = { path, index: clamped }
          visiblePath.current = path
          scrollToTarget(path, target)
          return path
        }
        fileIndex += direction
        index = direction === 1 ? 0 : Number.MAX_SAFE_INTEGER
      }
      return undefined
    },
    focus: () => container.current?.focus(),
  }))

  // the file at the top: the last one whose top is above the scroll position
  const handleScroll = (scrollTop: number, viewer: CodeViewInstance<undefined, undefined>) => {
    if (!followScroll.current) {
      return
    }
    let low = 0
    let high = entries.length - 1
    let found = 0
    while (low <= high) {
      const middle = (low + high) >> 1
      const top = viewer.getTopForItem(entries[middle]!.file.path) ?? 0
      if (top <= scrollTop + HEADER_HEIGHT / 2) {
        found = middle
        low = middle + 1
      }
      else {
        high = middle - 1
      }
    }
    const path = entries[found]?.file.path
    if (path !== undefined && path !== visiblePath.current) {
      visiblePath.current = path
      // the user scrolled into another file: N starts from its top
      if (changeCursor.current?.path !== path) {
        changeCursor.current = null
      }
      onVisibleFileChange(path)
    }
  }

  return (
    <WorkerPoolContext.Provider value={getDiffWorkerPool()}>
      <CodeView<undefined, undefined>
        ref={codeView}
        containerRef={container}
        className="h-full overflow-auto outline-none"
        items={items}
        options={options}
        onScroll={handleScroll}
        renderCustomHeader={(item) => {
          const entry = byPath.get(item.id)
          return entry === undefined ? null : <ReviewFileHeader entry={entry} owner={fileOwners?.get(entry.file.path)} collapsed={isCollapsed(item.id)} onToggle={toggleFile} />
        }}
      />
    </WorkerPoolContext.Provider>
  )
}

interface ReviewFileHeaderProps {
  entry: CommitFileDiff
  owner: string | undefined
  collapsed: boolean
  onToggle: (path: string) => void
}

/** The sticky header of a file: what the tree row says, with the path in full */
function ReviewFileHeader({ entry, owner, collapsed, onToggle }: ReviewFileHeaderProps) {
  const { file } = entry
  const notice = getNotice(entry)
  const label = formatTreeFileLabel(file)
  const directory = file.path.slice(0, file.path.length - label.name.length)
  return (
    <div data-testid="review-file-header" data-path={file.path} className="group/header flex h-9 items-center gap-2 border-b border-border bg-card px-2 text-sm">
      <Button
        variant="ghost"
        size="icon-xs"
        aria-label={collapsed ? `Expand ${label.name}` : `Collapse ${label.name}`}
        aria-expanded={!collapsed}
        disabled={notice !== null}
        onClick={() => onToggle(file.path)}
      >
        <ChevronRightIcon className={cn("transition-transform duration-150", !collapsed && "rotate-90")} />
      </Button>
      <span aria-hidden className={cn("w-3 shrink-0 text-center font-mono text-xs no-underline", getFileStatusTextClass(file.status))}>
        {getFileStatusLetter(file.status)}
      </span>
      <span className="flex min-w-0 items-baseline truncate" title={file.oldPath === null ? file.path : `${file.oldPath} → ${file.path}`}>
        <span className="truncate text-muted-foreground">{directory}</span>
        <span className="shrink-0 font-medium">{label.name}</span>
        {label.from !== null && <span className="ml-1.5 shrink-0 text-xs text-muted-foreground">{`← ${label.from}`}</span>}
      </span>
      <CopyButton
        text={() => file.path}
        tooltip="Copy path"
        label={`Copy path of ${label.name}`}
        className="duration-200 ease-out group-hover/header:opacity-100 group-hover/header:delay-300"
      />
      <span className="ml-auto flex shrink-0 items-center gap-3 text-xs">
        {notice !== null && <span className="text-muted-foreground">{notice}</span>}
        {owner !== undefined && <span data-testid="review-file-owner" className="text-muted-foreground" title={`Owned by ${owner}`}>{owner}</span>}
        {(file.additions > 0 || file.deletions > 0) && <LineCounts additions={file.additions} deletions={file.deletions} colored />}
      </span>
    </div>
  )
}
