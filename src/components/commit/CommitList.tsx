import { Fragment, lazy, memo, Suspense, useCallback, useEffect, useRef, useState } from "react"
import type { ComponentProps, DragEvent, KeyboardEvent, MouseEvent, ReactNode } from "react"
import { cn } from "cn"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { ContextMenu, ContextMenuContent, ContextMenuTrigger } from "@/components/ui/context-menu"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { LandedCommitMarker } from "@/components/branch/LandedChip"
import { CommitHeader } from "@/components/commit/CommitHeader"
import { CommitMessageButton } from "@/components/commit/CommitMessage"
import { ContextMenuItems } from "@/components/shared/ContextMenuItems"
import { beginCommitDrag } from "@/hooks/useCommitDrag"
import { useCommitFileStats, useCommitFiles } from "@/hooks/useCommitFiles"
import { commitReviewBlocker, getCommitReviewMenuItems } from "@/hooks/useCommitReviewActions"
import { usePreviewHotkeys } from "@/hooks/usePreviewHotkeys"
import { useRewordCommit } from "@/hooks/useRewordCommit"
import type { RowSelection } from "@/hooks/useRowSelection"
import { describeFixupTarget, getFixupTarget } from "@/lib/autosquash"
import type { CommitUnion } from "@/lib/commit"
import { getCommitFileCount, getCommitHash, getCommitSubject, getSubjectLine, hasCommitMessageBody, parseSubject, stripLeadingIssueKey, toReviewCommit } from "@/lib/commit"
import { openCommitReviewWindow } from "@/lib/commit-review-window"
import type { MenuItemGroups } from "@/lib/menu"
import { useAutosquashPlan } from "@/stores/branch-sync"

// the preview pulls in the review: its frame, the file tree and the resizable panels
const CommitPreviewDialog = lazy(() => import("@/components/commit/CommitPreviewDialog").then(it => ({ default: it.CommitPreviewDialog })))
const MergeConflictViewer = lazy(() => import("@/components/conflict/MergeConflictViewer").then(it => ({ default: it.MergeConflictViewer })))

export type CommitListVariant = "compact" | "detailed" | "status"

export interface CommitListProps<T extends CommitUnion> {
  commits: readonly T[]
  variant?: CommitListVariant
  /** For the status variant: passed to the merge conflict viewer */
  branchName?: string
  /** The name of the list in the title bar of its review window: the branch, or "Unassigned" */
  listLabel?: string
  /** Show the file count a commit carries (`fileDiffs` of a missing commit) until its files are loaded */
  showFileCount?: boolean
  showAuthor?: boolean
  /** Marks the tip commit as the amend target: the first, the last of a list `oldestFirst` */
  highlightTipCommit?: boolean
  /** The commits are listed oldest first (a table with stack lanes): the tip is the last */
  oldestFirst?: boolean
  /** The issue key of the branch, which its row shows: a subject starting with it shows the rest */
  omitIssueKey?: string | null
  /** The commits are on the current branch: the preview can edit their messages */
  editable?: boolean

  /**
   * Makes the rows selectable (click, Cmd/Ctrl+click, Shift+click, Cmd/Ctrl+A, Escape).
   * The state is owned by the caller: `useRowSelection({ items: commits, getId: getCommitHash })`.
   */
  selection?: RowSelection<T>
  /**
   * Context menu for selectable rows. Called when the menu opens, with the selected commits if the row is selected,
   * otherwise with the row's commit only. Returning no items suppresses the menu.
   */
  contextMenuItems?: (commits: T[]) => MenuItemGroups
  /** Called before the selection shortcuts; call `preventDefault()` to suppress them */
  onKeyDown?: (event: KeyboardEvent<HTMLTableElement>) => void
  /**
   * Makes selectable rows draggable; called when a drag starts with the selected commits if the row is selected,
   * otherwise with the row's commit only
   */
  onDragCommits?: (commits: T[]) => void
  /** Called when a drag started in this list ends, dropped or not */
  onDragEnd?: () => void

  /** Rendered above the table (e.g. an inline form) */
  portalTarget?: ReactNode
  /** Rendered at the end of each commit cell; keep it stable (`useCallback`) so rows stay memoized */
  renderAfterCommit?: (commit: T, index: number) => ReactNode
  /** A header row above a commit that starts a section of the list (the branch of the missing commits), or null */
  renderSectionHeader?: (commit: T, index: number) => ReactNode
}

/**
 * Commit table used by the branch tables, the unassigned commits card and the conflict views.
 * Space (over a row, or on the last clicked row of the focused table) and the row's message button open the commit preview;
 * D its review in its window (`usePreviewHotkeys`, and the first item of the row's menu).
 * The files of the commits are loaded with one call when the list is shown; the meta lines show the counts once known.
 */
export function CommitList<T extends CommitUnion>({
  commits,
  variant = "compact",
  branchName,
  listLabel,
  showFileCount = false,
  showAuthor = false,
  highlightTipCommit = false,
  oldestFirst = false,
  omitIssueKey = null,
  editable = false,
  selection,
  contextMenuItems,
  onKeyDown,
  onDragCommits,
  onDragEnd,
  portalTarget,
  renderAfterCommit,
  renderSectionHeader,
}: CommitListProps<T>) {
  const selectable = selection !== undefined
  const isLoadingFiles = useCommitFiles(commits.map(getCommitHash))

  // stable callbacks for the memoized rows, reading the latest props
  const latestRef = useRef<{
    selection: typeof selection
    contextMenuItems: typeof contextMenuItems
    onDragCommits: typeof onDragCommits
    onDragEnd: typeof onDragEnd
    review: (commit: CommitUnion) => void
  }>({ selection, contextMenuItems, onDragCommits, onDragEnd, review: () => {} })

  const getMenuGroups = useCallback((commit: CommitUnion): MenuItemGroups => {
    const { selection, contextMenuItems } = latestRef.current
    if (contextMenuItems === undefined || selection === undefined) {
      return []
    }
    const actionCommits = getActionCommits(selection, commit)
    // the review of the commit first, as in the menu of a branch
    return [getCommitReviewMenuItems(actionCommits, latestRef.current.review), ...contextMenuItems(actionCommits)]
  }, [])

  const handleDragStart = useCallback((event: DragEvent, commit: CommitUnion) => {
    const { selection, onDragCommits } = latestRef.current
    if (selection === undefined || onDragCommits === undefined) {
      return
    }
    const commits = getActionCommits(selection, commit)
    beginCommitDrag(event, commits.length)
    onDragCommits(commits)
  }, [])
  const handleDragEnd = useCallback(() => latestRef.current.onDragEnd?.(), [])

  // a row context menu opened with the mouse leaves the focus on the table; its keys (e.g. Escape) belong to the menu
  const isMenuOpenRef = useRef(false)
  const handleMenuOpenChange = useCallback((open: boolean) => {
    isMenuOpenRef.current = open
  }, [])

  // the preview follows the commit, not its index, across a sync; after an edit, it waits for the reworded commit
  // (`nextHash`)
  const [preview, setPreview] = useState<{ hash: string; nextHash?: string } | null>(null)
  let previewIndex = preview === null ? -1 : commits.findIndex(commit => getCommitHash(commit) === preview.hash)
  if (preview !== null && previewIndex === -1) {
    const nextHash = preview.nextHash
    previewIndex = nextHash === undefined ? -1 : commits.findIndex(commit => getCommitHash(commit) === nextHash)
    // a commit gone after a sync (also a reworded one that moved to another list) closes the preview for good
    setPreview(previewIndex === -1 ? null : { hash: nextHash! })
  }
  // loaded on the first use, then kept mounted (the dialog moves the focus only when it opens or closes)
  const [isPreviewLoaded, setIsPreviewLoaded] = useState(false)
  const openPreview = useCallback((hash: string) => {
    setPreview({ hash })
    setIsPreviewLoaded(true)
  }, [])
  const { rewordCommit } = useRewordCommit()
  const saveMessage = (commit: CommitUnion, message: string) =>
    rewordCommit(commit, message, newCommitId => {
      setPreview({ hash: getCommitHash(commit), nextHash: newCommitId })
    })
  const tableRef = useRef<HTMLTableElement>(null)
  // the commit the preview showed last (↑/↓ stepped): its close focuses the list, and the keys of a row (Space, D) act on
  // that commit while the pointer is on no row, until a click of a row
  const lastPreviewedRef = useRef<string | null>(null)
  const closePreview = () => {
    lastPreviewedRef.current = preview?.hash ?? null
    setPreview(null)
  }
  // D reviews the commit in its window, also from a list in a sub-window (the missing commits); a commit without files
  // gets its preview, which says so. Resolves to whether the window opened
  const review = async (hash: string, path: string | null = null): Promise<boolean> => {
    const index = commits.findIndex(it => getCommitHash(it) === hash)
    if (index === -1) {
      return false
    }
    if (path === null && commitReviewBlocker([commits[index]!]) !== null) {
      openPreview(hash)
      return false
    }
    return openCommitReviewWindow(commits.map(toReviewCommit), listLabel ?? branchName ?? null, index, path)
  }
  const reviewRow = (commit: CommitUnion) => void review(getCommitHash(commit))
  usePreviewHotkeys(tableRef, () => lastPreviewedRef.current ?? latestRef.current.selection?.getAnchorId() ?? null, {
    preview: openPreview,
    review: hash => void review(hash),
  })
  useEffect(() => {
    latestRef.current = { selection, contextMenuItems, onDragCommits, onDragEnd, review: reviewRow }
  })

  const selectRow = selection?.handleRowClick
  const handleRowClick = useCallback(
    (event: MouseEvent, id: string, index: number) => {
      lastPreviewedRef.current = null
      selectRow?.(event, id, index)
    },
    [selectRow],
  )
  const handleKeyDown = (event: KeyboardEvent<HTMLTableElement>) => {
    // React bubbles events from portals (row context menus, popovers) through the component tree; only handle keys pressed in the table.
    // The preview takes the focus a frame after it opens; until then, its keys (Escape, arrows) must not reach the selection.
    if (isMenuOpenRef.current || previewIndex !== -1 || !event.currentTarget.contains(event.target as Node)) {
      return
    }
    onKeyDown?.(event)
    if (!event.defaultPrevented && selection !== undefined) {
      selection.handleKeyDown(event)
    }
  }

  return (
    <div>
      {portalTarget}
      <table
        ref={tableRef}
        className="w-full rounded-lg outline-none focus-visible:ring-3 focus-visible:ring-ring/30"
        tabIndex={selectable ? 0 : -1}
        // the meta lines get their file counts
        aria-busy={isLoadingFiles || undefined}
        onKeyDown={handleKeyDown}
      >
        <tbody className="divide-y divide-border">
          {commits.map((commit, index) => {
            const hash = getCommitHash(commit)
            const sectionHeader = renderSectionHeader?.(commit, index)
            return (
              <Fragment key={hash}>
                {sectionHeader != null && (
                  <tr>
                    <td className="bd-padding-list-item pb-1">{sectionHeader}</td>
                  </tr>
                )}
                <CommitRow
                  commit={commit}
                  hash={hash}
                  index={index}
                  variant={variant}
                  branchName={branchName}
                  showFileCount={showFileCount}
                  showAuthor={showAuthor}
                  omitIssueKey={omitIssueKey}
                  isAmendTarget={highlightTipCommit && index === (oldestFirst ? commits.length - 1 : 0)}
                  selectable={selectable}
                  selected={selection?.isSelected(hash) === true}
                  onRowClick={handleRowClick}
                  getMenuGroups={contextMenuItems !== undefined && selectable ? getMenuGroups : undefined}
                  onDragStart={onDragCommits !== undefined && selectable ? handleDragStart : undefined}
                  onDragEnd={handleDragEnd}
                  onMenuOpenChange={handleMenuOpenChange}
                  onOpenPreview={openPreview}
                  renderAfterCommit={renderAfterCommit as CommitRowProps["renderAfterCommit"]}
                />
              </Fragment>
            )
          })}
        </tbody>
      </table>
      {isPreviewLoaded && (
        <Suspense fallback={null}>
          <CommitPreviewDialog
            commits={commits}
            index={previewIndex}
            onIndexChange={index => setPreview({ hash: getCommitHash(commits[index]!) })}
            onClose={closePreview}
            finalFocus={() => tableRef.current ?? true}
            onSaveMessage={editable ? saveMessage : undefined}
            onReview={(index, path) => review(getCommitHash(commits[index]!), path)}
            showAuthor={showAuthor}
            showFileCount={showFileCount}
            showNewHash={variant === "status"}
          />
        </Suspense>
      )}
    </div>
  )
}

/** The commits a row action applies to: the selection if the row is selected, otherwise the row's commit only */
function getActionCommits<T extends CommitUnion>(selection: RowSelection<T>, commit: CommitUnion): T[] {
  const isRowSelected = selection.isSelected(getCommitHash(commit))
  return isRowSelected && selection.selectedCount > 0 ? selection.selectedItems : [commit as T]
}

interface CommitRowProps {
  commit: CommitUnion
  hash: string
  index: number
  variant: CommitListVariant
  branchName: string | undefined
  showFileCount: boolean
  showAuthor: boolean
  omitIssueKey: string | null
  isAmendTarget: boolean
  selectable: boolean
  selected: boolean
  onRowClick: ((event: MouseEvent, id: string, index: number) => void) | undefined
  /** Set if the row has a context menu */
  getMenuGroups: ((commit: CommitUnion) => MenuItemGroups) | undefined
  /** Set if the row is draggable */
  onDragStart: ((event: DragEvent, commit: CommitUnion) => void) | undefined
  onDragEnd: () => void
  onMenuOpenChange: (open: boolean) => void
  onOpenPreview: (hash: string) => void
  renderAfterCommit: ((commit: CommitUnion, index: number) => ReactNode) | undefined
}

const CommitRow = memo(function CommitRow(props: CommitRowProps) {
  const { commit, hash, index, selectable, selected, onRowClick, getMenuGroups, onDragStart, onDragEnd, onMenuOpenChange, renderAfterCommit } = props

  const rowProps = {
    "data-row-id": hash,
    "data-selected": String(selectable && selected),
    // only selectable rows react to the pointer: hover on a row that does nothing on click is a false affordance
    className: cn(selectable && "cursor-pointer select-none hover:bg-muted", selectable && selected && "bg-primary/15 hover:bg-primary/20"),
    onClick: selectable && onRowClick ? (event: MouseEvent) => onRowClick(event, hash, index) : undefined,
    ...(onDragStart === undefined ? {} : { draggable: true, onDragStart: (event: DragEvent) => onDragStart(event, commit), onDragEnd }),
  }

  // a missing commit's file diffs, below its row content
  const after = renderAfterCommit?.(commit, index)
  const cell = (
    <td className="bd-padding-list-item">
      {after == null ? (
        <CommitCellContent {...props} />
      ) : (
        <div className="flex flex-col gap-3">
          <CommitCellContent {...props} />
          {after}
        </div>
      )}
    </td>
  )

  if (getMenuGroups === undefined) {
    return <tr {...rowProps}>{cell}</tr>
  }
  return (
    <CommitRowWithContextMenu commit={commit} rowProps={rowProps} getMenuGroups={getMenuGroups} onOpenChange={onMenuOpenChange}>
      {cell}
    </CommitRowWithContextMenu>
  )
})

interface CommitRowWithContextMenuProps {
  commit: CommitUnion
  rowProps: ComponentProps<"tr"> & Record<`data-${string}`, string>
  getMenuGroups: (commit: CommitUnion) => MenuItemGroups
  onOpenChange: (open: boolean) => void
  children: ReactNode
}

function CommitRowWithContextMenu({ commit, rowProps, getMenuGroups, onOpenChange, children }: CommitRowWithContextMenuProps) {
  // computed when the menu opens, from the selection at that moment
  const [groups, setGroups] = useState<MenuItemGroups>([])

  return (
    <ContextMenu
      onOpenChange={(open, eventDetails) => {
        if (open) {
          const nextGroups = getMenuGroups(commit)
          if (nextGroups.every(group => group.length === 0)) {
            eventDetails.cancel()
            return
          }
          setGroups(nextGroups)
        }
        onOpenChange(open)
      }}
    >
      <ContextMenuTrigger render={<tr {...rowProps} />}>{children}</ContextMenuTrigger>
      {/* an inline form opened from the menu takes the focus */}
      <ContextMenuContent finalFocus={false}>
        <ContextMenuItems groups={groups} />
      </ContextMenuContent>
    </ContextMenu>
  )
}

function CommitCellContent({ commit, hash, variant, branchName, showFileCount, showAuthor, omitIssueKey, isAmendTarget, onOpenPreview }: CommitRowProps) {
  const status = variant === "status" && "status" in commit ? commit.status : undefined
  const error = variant === "status" && "error" in commit ? commit.error : undefined
  // a fixup shows its markers apart, with where it goes
  const autosquash = parseSubject(getSubjectLine(commit.message)).autosquash
  const fullSubject = getCommitSubject(commit)
  const shownSubject = autosquash === "" ? fullSubject : fullSubject.slice(parseSubject(fullSubject).autosquash.length)
  // a subject that is only the key keeps it
  const subject = (omitIssueKey === null ? "" : stripLeadingIssueKey(shownSubject, omitIssueKey)) || shownSubject
  const hasBody = hasCommitMessageBody(fullSubject, commit.message)

  const statusHash = variant === "status" && "hash" in commit ? commit.hash : undefined
  const fileStats = useCommitFileStats(hash)

  return (
    // gap-3: the header and the error or conflict below it (the header alone has no sibling)
    <div className="flex flex-col gap-3">
      <CommitHeader
        subject={subject}
        hash={hash}
        newHash={statusHash}
        author={showAuthor ? commit.author : undefined}
        authorTime={commit.authorTime}
        committerTime={commit.committerTime}
        fileCount={showFileCount ? getCommitFileCount(commit) : undefined}
        fileStats={fileStats}
        subjectPrefix={autosquash !== "" && <FixupMarker marker={autosquash.trim()} commitId={hash} />}
        subjectAddon={
          <>
            {hasBody && <CommitMessageButton onOpen={() => onOpenPreview(hash)} />}
            <LandedCommitMarker commitId={hash} />
            {status === "Pending" && (
              <Badge variant="secondary">
                <Spinner data-icon="inline-start" />
                Pending
              </Badge>
            )}
            {status === "Error" && <Badge variant="destructive">{error?.MergeConflict ? "Merge conflict" : "Error"}</Badge>}
            {status === "Blocked" && <Badge variant="warning">Blocked</Badge>}
            {isAmendTarget && (
              <Tooltip>
                <TooltipTrigger render={<Badge variant="primary" />}>Amend target</TooltipTrigger>
                <TooltipContent>Changes will be amended to this commit</TooltipContent>
              </Tooltip>
            )}
          </>
        }
      />

      {error?.MergeConflict ? (
        <Suspense fallback={null}>
          <MergeConflictViewer conflict={error.MergeConflict} branchName={branchName} />
        </Suspense>
      ) : (
        error?.Generic && (
          <Alert variant="destructive" className="py-2">
            <AlertDescription>
              <p className="text-xs">{error.Generic}</p>
            </AlertDescription>
          </Alert>
        )
      )}
    </div>
  )
}

/** The autosquash markers of a fixup commit (`fixup!`), with its target in the tooltip; muted like a subject prefix */
function FixupMarker({ marker, commitId }: { marker: string; commitId: string }) {
  const plan = useAutosquashPlan()
  const target = getFixupTarget(plan, commitId)
  const markerProps = {
    "data-testid": "fixup-marker",
    "data-target": target === undefined ? undefined : target === null ? "none" : target.target,
    className: cn("shrink-0 font-mono text-xs", target === null ? "text-warning" : "text-muted-foreground"),
  }
  const description = describeFixupTarget(target)
  if (description === null) {
    return <span {...markerProps}>{marker}</span>
  }
  return (
    <Tooltip>
      <TooltipTrigger render={<span {...markerProps} />}>{marker}</TooltipTrigger>
      <TooltipContent>{description}</TooltipContent>
    </Tooltip>
  )
}
