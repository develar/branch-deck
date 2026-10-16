import { useRef, useState } from "react"
import type { KeyboardEvent } from "react"
import { Button } from "@/components/ui/button"
import { DialogTitle } from "@/components/ui/dialog"
import { Kbd } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"
import { CommitFilesSection } from "@/components/commit/CommitFilesSection"
import { CommitMessageBody } from "@/components/commit/CommitMessage"
import { CommitMetadata } from "@/components/commit/CommitMetadata"
import { CommitReview } from "@/components/commit/CommitReview"
import { CommitSubject } from "@/components/commit/CommitSubject"
import { PrefixChangeHint } from "@/components/commit/PrefixChangeHint"
import {
  KeyHint,
  PreviewDialogBody,
  PreviewDialogFooter,
  PreviewDialogFrame,
  PreviewDialogHeader,
  PreviewNavigationHints,
} from "@/components/shared/PreviewDialogFrame"
import { useCommitFileStats } from "@/hooks/useCommitFiles"
import type { CommitUnion } from "@/lib/commit"
import { getCommitFileCount, getCommitHash, getCommitSubject, hasCommitMessageBody } from "@/lib/commit"
import { isMac } from "@/lib/platform"
import { handlePreviewItemSwitchKey, handlePreviewNavigationKey, hasKeyModifier } from "@/lib/preview-keys"
import { useBranchSync } from "@/stores/branch-sync"
import { toggleCommitFilesSection, useIsCommitFilesSectionOpen } from "@/stores/commit-files"

export interface CommitPreviewDialogProps {
  commits: readonly CommitUnion[]
  /** Index of the shown commit in `commits`, -1 while closed */
  index: number
  onIndexChange: (index: number) => void
  onClose: () => void
  showAuthor: boolean
  showFileCount: boolean
  /** Show `hash → newHash` of a synced commit, as the status rows do */
  showNewHash: boolean
  /** Set if the messages can be edited; resolves to false if saving failed (the edit stays open) */
  onSaveMessage?: (commit: CommitUnion, message: string) => Promise<boolean>
}

/**
 * Centered preview of one commit of a list, as Quick Look shows a file: the whole subject, the meta line, the body and
 * the changed files. ↑/↓ show the previous or next commit of the list (⌥↑/⌥↓ also from the file tree), Space and Escape
 * close it, E edits the message, F shows or hides the files, D reviews the commit: the window-sized tree and diffs of all
 * its files, until Escape.
 *
 * Stays mounted while closed (see `PreviewDialogFrame`).
 */
export function CommitPreviewDialog({ commits, index, onIndexChange, onClose, showAuthor, showFileCount, showNewHash, onSaveMessage }: CommitPreviewDialogProps) {
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
  const [isSaving, setIsSaving] = useState(false)
  const isSyncing = useBranchSync(state => state.isSyncing)
  const canEdit = onSaveMessage !== undefined && commit !== undefined && !isSyncing
  const fileStats = useCommitFileStats(hash)
  const isFilesOpen = useIsCommitFilesSectionOpen()
  // the review stays while another commit is shown (⌥↑/⌥↓); the file it opened at belongs to the first one
  const [review, setReview] = useState<{ hash: string, path: string | null } | null>(null)
  const isReviewing = open && review !== null && !isEditing
  const [reviewPosition, setReviewPosition] = useState("")
  const canReview = fileStats === undefined || fileStats.count > 0

  // the body has the focus, so PageUp/PageDown scroll it
  const bodyRef = useRef<HTMLDivElement>(null)

  const startEdit = () => {
    if (canEdit) {
      setDraft(commit.message.trimEnd())
      setEditedHash(hash)
    }
  }

  const endEdit = () => {
    setEditedHash(null)
    bodyRef.current?.focus()
  }

  const save = async () => {
    if (commit === undefined || onSaveMessage === undefined || isSaving || isSyncing) {
      return
    }
    setIsSaving(true)
    // `.finally`, not try/finally: React Compiler cannot compile a try without a catch
    const saved = await onSaveMessage(commit, draft).finally(() => setIsSaving(false))
    if (saved) {
      endEdit()
    }
  }

  const close = () => {
    setEditedHash(null)
    setReview(null)
    onClose()
  }

  const startReview = (path: string | null = null) => {
    if (canReview || path !== null) {
      setReview({ hash, path })
    }
  }

  const endReview = () => {
    setReview(null)
    bodyRef.current?.focus()
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
    // the review's keys are its own: the arrows, PageUp, PageDown and Space scroll the diffs
    if (isReviewing) {
      return
    }
    if (hasKeyModifier(event) || handlePreviewNavigationKey(event, currentIndex, commits.length, onIndexChange, close)) {
      return
    }
    // the physical key: E is "у" in the Russian layout
    if (event.code === "KeyE" && canEdit) {
      event.preventDefault()
      startEdit()
    }
    else if (event.code === "KeyF") {
      event.preventDefault()
      toggleCommitFilesSection()
    }
    else if (event.code === "KeyD") {
      event.preventDefault()
      startReview()
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
        // in the review, Escape goes back to the preview of the same commit
        if (isReviewing && eventDetails.reason === "escape-key") {
          eventDetails.cancel()
          endReview()
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
      width={isReviewing ? "full" : "lg"}
      onKeyDown={handleKeyDown}
    >
      {commit !== undefined && (
        <>
          <PreviewDialogHeader>
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

          <PreviewDialogBody ref={bodyRef} itemKey={hash} fill={isReviewing} data-testid="commit-preview-body">
            {isReviewing
              ? <CommitReview subject={{ kind: "commit", id: hash }} initialPath={review.hash === hash ? review.path : null} onPositionChange={setReviewPosition} />
              : isEditing
                ? (
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
                  )
                : (
                    <div className="flex flex-col gap-5">
                      {hasCommitMessageBody(subject, commit.message)
                        ? <CommitMessageBody message={commit.message} size="reading" />
                        : <p className="text-sm text-muted-foreground">No description</p>}
                      {isFilesOpen && <CommitFilesSection commitId={hash} onReviewFile={startReview} />}
                    </div>
                  )}
          </PreviewDialogBody>

          <PreviewDialogFooter
            start={isEditing
              ? <PrefixChangeHint before={commit.message} after={draft} action="Saving" />
              : isReviewing
                ? <span data-testid="commit-review-position">{commits.length > 1 ? `Commit ${currentIndex + 1} / ${commits.length} · ${reviewPosition}` : reviewPosition}</span>
                : <span data-testid="commit-preview-position">{`${currentIndex + 1} / ${commits.length}`}</span>}
          >
            {isReviewing
              ? <ReviewKeyHints count={commits.length} />
              : isEditing
                ? (
                    <>
                      <KeyHint keys={<Kbd>Esc</Kbd>}>Cancel</KeyHint>
                      <Button size="xs" disabled={isSaving || isSyncing} onClick={() => void save()}>
                        {isSaving && <Spinner data-icon="inline-start" />}
                        Save
                        <Kbd>{isMac ? "⌘↵" : "Ctrl+Enter"}</Kbd>
                      </Button>
                    </>
                  )
                : (
                    <>
                      <PreviewNavigationHints count={commits.length} />
                      <Button variant="ghost" size="xs" disabled={!canReview} data-testid="commit-review-toggle" onClick={() => startReview()}>
                        <Kbd>D</Kbd>
                        Review
                      </Button>
                      <Button variant="ghost" size="xs" aria-pressed={isFilesOpen} data-testid="commit-files-toggle" onClick={toggleCommitFilesSection}>
                        <Kbd>F</Kbd>
                        Files
                      </Button>
                      {onSaveMessage !== undefined && (
                        <Button variant="ghost" size="xs" disabled={!canEdit} onClick={startEdit}>
                          <Kbd>E</Kbd>
                          Edit
                        </Button>
                      )}
                    </>
                  )}
          </PreviewDialogFooter>
        </>
      )}
    </PreviewDialogFrame>
  )
}

/** The keys of the review: the files, the changes, the commits (`count` > 1) and the way back */
export function ReviewKeyHints({ count }: { count: number }) {
  const option = isMac ? "⌥" : "Alt+"
  return (
    <>
      <KeyHint keys={(
        <>
          <Kbd>J</Kbd>
          <Kbd>K</Kbd>
        </>
      )}
      >
        Files
      </KeyHint>
      <KeyHint keys={(
        <>
          <Kbd>N</Kbd>
          <Kbd>P</Kbd>
        </>
      )}
      >
        Changes
      </KeyHint>
      {count > 1 && (
        <KeyHint keys={(
          <>
            <Kbd>{`${option}↑`}</Kbd>
            <Kbd>{`${option}↓`}</Kbd>
          </>
        )}
        >
          Commits
        </KeyHint>
      )}
      <KeyHint keys={<Kbd>Esc</Kbd>}>Back</KeyHint>
    </>
  )
}
