import { useRef, useState } from "react"
import type { KeyboardEvent, ReactElement, ReactNode } from "react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { DialogTitle } from "@/components/ui/dialog"
import { Kbd } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { CommitFilesSection } from "@/components/commit/CommitFilesSection"
import { CommitMessageBody } from "@/components/commit/CommitMessage"
import { CommitMetadata } from "@/components/commit/CommitMetadata"
import { CommitSubject } from "@/components/commit/CommitSubject"
import { PrefixChangeHint } from "@/components/commit/PrefixChangeHint"
import { DropdownMenuItems } from "@/components/shared/DropdownMenuItems"
import { LockWaitNotice } from "@/components/shared/LockWaitNotice"
import { PRIMARY_KBD_CLASS_NAME } from "@/components/shared/MenuItemButton"
import { MoreActionsMenu } from "@/components/shared/MoreActionsMenu"
import { KeyHint, PreviewDialogBody, PreviewDialogFooter, PreviewDialogFrame, PreviewDialogHeader, PreviewNavigationHints } from "@/components/shared/PreviewDialogFrame"
import { useCommitFileStats } from "@/hooks/useCommitFiles"
import { REWORD_SURFACE } from "@/hooks/useRewordCommit"
import type { CommitUnion } from "@/lib/commit"
import { getCommitFileCount, getCommitHash, getCommitSubject, hasCommitMessageBody } from "@/lib/commit"
import { formatShortHash } from "@/lib/hash"
import type { MenuItemGroups } from "@/lib/menu"
import { toPreviewMenu } from "@/lib/menu"
import { isMac } from "@/lib/platform"
import { handlePreviewItemSwitchKey, handlePreviewNavigationKey, hasKeyModifier } from "@/lib/preview-keys"
import { useBranchSync } from "@/stores/branch-sync"
import { toggleCommitFilesSection, useIsCommitFilesSectionOpen } from "@/stores/commit-files"
import { useIndicatorClaim, useSurfaceBusy } from "@/stores/operations"

export interface CommitPreviewDialogProps {
  commits: readonly CommitUnion[]
  /** Index of the shown commit in `commits`, -1 while closed */
  index: number
  onIndexChange: (index: number) => void
  onClose: () => void
  showAuthor: boolean
  showFileCount: boolean
  /** Name the synced hash of a commit on the hover of its hash, as the status rows do */
  showNewHash: boolean
  /** Set if the messages can be edited; resolves to false if saving failed (the edit stays open) */
  onSaveMessage?: (commit: CommitUnion, message: string) => Promise<boolean>
  /** The preview opened for "Reword…" of the row's menu: the message is in edit at once */
  editRequested?: boolean
  /** The menu of the row of a commit ("⋯" of the footer, `toPreviewMenu`); none for a list without one */
  menuGroups?: (commit: CommitUnion) => MenuItemGroups
  /**
   * D, the footer button and Enter on a file of the tree: the review of `commits[index]` in its window, at `path` (the
   * first file when null); resolves to whether the window opened, and the preview closes then
   */
  onReview: (index: number, path: string | null) => Promise<boolean>
  /** Focused when the preview closes (`PreviewDialogFrame`): the list, whose keys go on from the commit shown last */
  finalFocus?: () => HTMLElement | boolean | null
}

/**
 * Centered preview of one commit of a list, as Quick Look shows a file: the whole subject, the meta line, the body and
 * the changed files. ↑/↓ show the previous or next commit of the list (⌥↑/⌥↓ also from the file tree), Space and Escape
 * close it, E edits the message, F shows or hides the files, D reviews the commit in its window (Enter on a file of the
 * tree at that file) and closes the preview once the window opened.
 *
 * Stays mounted while closed (see `PreviewDialogFrame`).
 */
export function CommitPreviewDialog({
  commits,
  index,
  onIndexChange,
  onClose,
  showAuthor,
  showFileCount,
  showNewHash,
  onSaveMessage,
  editRequested = false,
  menuGroups,
  onReview,
  finalFocus,
}: CommitPreviewDialogProps) {
  const open = index !== -1
  // the last shown commit stays while the closing animation runs
  const [shownIndex, setShownIndex] = useState(index)
  if (open && index !== shownIndex) {
    setShownIndex(index)
  }
  const currentIndex = Math.min(open ? index : shownIndex, commits.length - 1)
  const commit = commits[currentIndex]
  const hash = commit === undefined ? "" : getCommitHash(commit)
  const subject = commit === undefined ? "" : getCommitSubject(commit)

  // the edit belongs to one commit: another commit (also the reworded one after the save) ends it
  const [editedHash, setEditedHash] = useState<string | null>(null)
  const isEditing = open && editedHash === hash
  const [draft, setDraft] = useState("")
  // the reword runs from the editor: its Save shows the progress until the sync shows the new commit
  useIndicatorClaim(REWORD_SURFACE)
  const isSaving = useSurfaceBusy(REWORD_SURFACE) !== null
  const isSyncing = useBranchSync(state => state.isSyncing)
  const canEdit = onSaveMessage !== undefined && commit !== undefined && !isSyncing
  const fileStats = useCommitFileStats(hash)
  const isFilesOpen = useIsCommitFilesSectionOpen()
  const canReview = fileStats === undefined || fileStats.count > 0
  // the window opens: the preview stays until it did (a failure keeps it, and is told)
  const [isOpeningReview, setIsOpeningReview] = useState(false)

  // the body has the focus, so PageUp/PageDown scroll it
  const bodyRef = useRef<HTMLDivElement>(null)

  const startEdit = () => {
    if (canEdit) {
      setDraft(commit.message.trimEnd())
      setEditedHash(hash)
    }
  }

  // "Reword…" of the row opened the preview: the edit starts at once, once per request (a Cancel ends it)
  const [handledEditRequest, setHandledEditRequest] = useState(false)
  if (open && editRequested && !handledEditRequest && commit !== undefined) {
    setHandledEditRequest(true)
    if (canEdit) {
      setDraft(commit.message.trimEnd())
      setEditedHash(hash)
    }
  } else if (!editRequested && handledEditRequest) {
    setHandledEditRequest(false)
  }

  const endEdit = () => {
    setEditedHash(null)
    bodyRef.current?.focus()
  }

  const save = async () => {
    if (commit === undefined || onSaveMessage === undefined || isSaving || isSyncing) {
      return
    }
    const saved = await onSaveMessage(commit, draft)
    if (saved) {
      endEdit()
    }
  }

  const close = () => {
    setEditedHash(null)
    onClose()
  }

  const review = (path: string | null = null) => {
    if ((!canReview && path === null) || commit === undefined || isOpeningReview) {
      return
    }
    setIsOpeningReview(true)
    void onReview(currentIndex, path)
      .then(opened => {
        if (opened) {
          close()
        }
      })
      .finally(() => setIsOpeningReview(false))
  }

  const handleKeyDown = (event: KeyboardEvent) => {
    // a key the content handled: an arrow or Space in the file tree
    if (event.defaultPrevented) {
      return
    }
    if (isEditing) {
      if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
        event.preventDefault()
        void save()
      }
      return
    }
    if (handlePreviewItemSwitchKey(event, currentIndex, commits.length, onIndexChange)) {
      return
    }
    if (hasKeyModifier(event) || handlePreviewNavigationKey(event, currentIndex, commits.length, onIndexChange, close)) {
      return
    }
    // the physical key: E is "у" in the Russian layout
    if (event.code === "KeyE" && canEdit) {
      event.preventDefault()
      startEdit()
    } else if (event.code === "KeyF") {
      event.preventDefault()
      toggleCommitFilesSection()
    } else if (event.code === "KeyD") {
      event.preventDefault()
      review()
    }
  }

  return (
    <PreviewDialogFrame
      data-testid="commit-preview"
      open={open && commit !== undefined}
      onOpenChange={(nextOpen, eventDetails) => {
        if (nextOpen) {
          return
        }
        // while editing, Escape cancels the edit and a click outside keeps the draft
        if (isEditing && (eventDetails.reason === "escape-key" || eventDetails.reason === "outside-press")) {
          eventDetails.cancel()
          if (eventDetails.reason === "escape-key" && !isSaving) {
            endEdit()
          }
          return
        }
        close()
      }}
      initialFocus={bodyRef}
      finalFocus={finalFocus}
      onKeyDown={handleKeyDown}
    >
      {commit !== undefined && (
        <>
          <PreviewDialogHeader
            end={
              // the menu of the row, before the close button; the footer keeps the keyed actions
              menuGroups !== undefined && commit !== undefined ? (
                <MoreActionsMenu name={formatShortHash(hash)} menu={<DropdownMenuItems groups={toPreviewMenu(menuGroups(commit), close, PREVIEW_KEYS)} />} />
              ) : undefined
            }
          >
            <DialogTitle className="leading-6">
              <CommitSubject subject={subject} clamp={false} className="text-base font-medium" />
            </DialogTitle>
            <CommitMetadata
              hash={hash}
              newHash={showNewHash && "hash" in commit ? commit.hash : undefined}
              author={showAuthor ? commit.author : undefined}
              authorTime={commit.authorTime}
              committerTime={commit.committerTime}
              fileCount={showFileCount ? getCommitFileCount(commit) : undefined}
              fileStats={fileStats}
              coloredLineCounts
            />
          </PreviewDialogHeader>

          <PreviewDialogBody ref={bodyRef} itemKey={hash} data-testid="commit-preview-body">
            {isEditing ? (
              <Textarea
                aria-label="Commit message"
                data-testid="commit-message-editor"
                // the raw message, as git stores it: the first line is the subject, the grouping prefix included
                className="min-h-48 font-mono"
                value={draft}
                disabled={isSaving}
                spellCheck
                autoFocus
                onChange={event => setDraft(event.target.value)}
              />
            ) : (
              <div className="flex flex-col gap-5">
                {hasCommitMessageBody(subject, commit.message) ? (
                  <CommitMessageBody message={commit.message} size="reading" />
                ) : (
                  <p className="text-sm text-muted-foreground">No description</p>
                )}
                <CommitFilesSection commitId={hash} fileCount={fileStats?.count} onReviewFile={review} />
              </div>
            )}
          </PreviewDialogBody>

          <PreviewDialogFooter
            start={
              isEditing ? (
                <div className="flex min-w-0 flex-col gap-0.5">
                  <LockWaitNotice surface={REWORD_SURFACE} />
                  <PrefixChangeHint before={commit.message} after={draft} action="Saving" refusesStackProblems />
                </div>
              ) : (
                <span className="tabular-nums" data-testid="commit-preview-position">{`${currentIndex + 1} / ${commits.length}`}</span>
              )
            }
          >
            {isEditing ? (
              <>
                <KeyHint keys={<Kbd>Esc</Kbd>}>Cancel</KeyHint>
                <Button size="xs" disabled={isSaving || isSyncing} onClick={() => void save()}>
                  {isSaving && <Spinner data-icon="inline-start" />}
                  Save
                  <Kbd className={PRIMARY_KBD_CLASS_NAME}>{isMac ? "⌘↵" : "Ctrl+Enter"}</Kbd>
                </Button>
              </>
            ) : (
              <>
                <PreviewNavigationHints count={commits.length} />
                <ReasonTooltip reason={canReview ? null : "The commit changes no files"}>
                  <Button
                    variant="ghost"
                    size="xs"
                    disabled={!canReview || isOpeningReview}
                    focusableWhenDisabled
                    className={cn(!canReview && "opacity-50")}
                    data-testid="commit-review-toggle"
                    onClick={() => review()}
                  />
                  <Kbd>D</Kbd>
                  Review Changes
                </ReasonTooltip>
                <Button variant="ghost" size="xs" aria-pressed={isFilesOpen} data-testid="commit-files-toggle" onClick={toggleCommitFilesSection}>
                  <Kbd>F</Kbd>
                  Files
                </Button>
                {onSaveMessage !== undefined && (
                  <ReasonTooltip reason={canEdit ? null : "Wait for the sync to finish"}>
                    <Button variant="ghost" size="xs" disabled={!canEdit} focusableWhenDisabled className={cn(!canEdit && "opacity-50")} onClick={startEdit} />
                    <Kbd>E</Kbd>
                    Edit
                  </ReasonTooltip>
                )}
              </>
            )}
          </PreviewDialogFooter>
        </>
      )}
    </PreviewDialogFrame>
  )
}

/** The keys the preview handles itself: only their hints are shown in its "⋯" */
const PREVIEW_KEYS = new Set(["d", "e", "f"])

/**
 * A footer button (the first child, its element) with its content (the rest); disabled, the hover says why. Base UI
 * keeps the button focusable, so the dimming is the button's
 */
function ReasonTooltip({ reason, children }: { reason: string | null; children: [ReactElement, ...ReactNode[]] }) {
  const [button, ...content] = children
  return (
    <Tooltip disabled={reason === null}>
      <TooltipTrigger render={button}>{content}</TooltipTrigger>
      <TooltipContent>{reason}</TooltipContent>
    </Tooltip>
  )
}
