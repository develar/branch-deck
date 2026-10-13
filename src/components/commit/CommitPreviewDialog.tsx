import { useEffect, useRef, useState } from "react"
import type { KeyboardEvent, ReactNode } from "react"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog"
import { Kbd, KbdGroup } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"
import { CommitMessageBody } from "@/components/commit/CommitMessage"
import { CommitMetadata } from "@/components/commit/CommitMetadata"
import { CommitSubject } from "@/components/commit/CommitSubject"
import type { CommitUnion } from "@/lib/commit"
import { getCommitFileCount, getCommitHash, getCommitSubject, getGroupingPrefix, hasCommitMessageBody } from "@/lib/commit"
import { useBranchSync } from "@/stores/branch-sync"

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
 * Centered preview of one commit of a list, as Quick Look shows a file: the whole subject, the meta line and the body.
 * ↑/↓ show the previous or next commit of the list, Space and Escape close it, E edits the message.
 *
 * Stays mounted while closed: Base UI moves the focus into a dialog (and back) only when `open` changes.
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

  // the body has the focus, so PageUp/PageDown scroll it; a newly shown commit starts at the top
  const bodyRef = useRef<HTMLDivElement>(null)
  useEffect(() => {
    bodyRef.current?.scrollTo({ top: 0 })
  }, [hash])

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
    try {
      if (await onSaveMessage(commit, draft)) {
        endEdit()
      }
    }
    finally {
      setIsSaving(false)
    }
  }

  const close = () => {
    setEditedHash(null)
    onClose()
  }

  const handleKeyDown = (event: KeyboardEvent) => {
    if (isEditing) {
      if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
        event.preventDefault()
        void save()
      }
      return
    }
    if (event.metaKey || event.ctrlKey || event.altKey || event.shiftKey) {
      return
    }
    if (event.key === "ArrowUp" || event.key === "ArrowDown") {
      event.preventDefault()
      const next = currentIndex + (event.key === "ArrowUp" ? -1 : 1)
      if (next >= 0 && next < commits.length) {
        onIndexChange(next)
      }
    }
    // the physical key: E is "у" in the Russian layout
    else if (event.code === "KeyE" && canEdit) {
      event.preventDefault()
      startEdit()
    }
    // Space on a button or a link presses it
    else if (event.key === " " && !(event.target instanceof Element && event.target.closest("button, a") !== null)) {
      event.preventDefault()
      close()
    }
  }

  return (
    <Dialog
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
    >
      <DialogContent
        data-testid="commit-preview"
        initialFocus={bodyRef}
        className="max-h-[min(80vh,48rem)] grid-rows-[auto_minmax(0,1fr)_auto] gap-0 p-0 sm:max-w-[min(56rem,calc(100%-4rem))]"
        onKeyDown={handleKeyDown}
      >
        {commit !== undefined && (
          <>
            {/* pr-14: room for the close button */}
            <div className="flex flex-col gap-1 border-b border-border py-4 pr-14 pl-6">
              <DialogTitle className="leading-5">
                <CommitSubject subject={subject} clamp={false} />
              </DialogTitle>
              <CommitMetadata
                hash={hash}
                newHash={showNewHash && "hash" in commit ? commit.hash : undefined}
                author={showAuthor ? commit.author : undefined}
                authorTime={commit.authorTime}
                committerTime={commit.committerTime}
                fileCount={showFileCount ? getCommitFileCount(commit) : undefined}
              />
            </div>

            <div ref={bodyRef} tabIndex={-1} data-testid="commit-preview-body" className="overflow-y-auto overscroll-contain px-6 py-5 outline-none">
              {isEditing
                ? (
                    <Textarea
                      aria-label="Commit message"
                      data-testid="commit-message-editor"
                      // the raw message, as git stores it: the first line is the subject, the grouping prefix included
                      className="min-h-48 font-mono text-[13px] leading-5"
                      value={draft}
                      disabled={isSaving}
                      spellCheck
                      autoFocus
                      onChange={event => setDraft(event.target.value)}
                    />
                  )
                : hasCommitMessageBody(subject, commit.message)
                  ? <CommitMessageBody message={commit.message} className="text-sm leading-6" />
                  : <p className="text-sm text-muted-foreground">No description</p>}
            </div>

            <div className="flex items-center justify-between gap-4 border-t border-border px-6 py-2 text-xs text-muted-foreground">
              {isEditing
                ? <PrefixChangeHint before={commit.message} after={draft} />
                : <span data-testid="commit-preview-position">{`${currentIndex + 1} / ${commits.length}`}</span>}
              <div className="flex items-center gap-3">
                {isEditing
                  ? (
                      <>
                        <KeyHint keys={<Kbd>Esc</Kbd>}>Cancel</KeyHint>
                        <Button size="xs" disabled={isSaving || isSyncing} onClick={() => void save()}>
                          {isSaving && <Spinner data-icon="inline-start" />}
                          Save
                          <Kbd>⌘↵</Kbd>
                        </Button>
                      </>
                    )
                  : (
                      <>
                        {commits.length > 1 && (
                          <KeyHint keys={(
                            <>
                              <Kbd>↑</Kbd>
                              <Kbd>↓</Kbd>
                            </>
                          )}
                          >
                            Previous/next
                          </KeyHint>
                        )}
                        <KeyHint keys={<Kbd>Space</Kbd>}>Close</KeyHint>
                        {onSaveMessage !== undefined && (
                          <Button variant="ghost" size="xs" disabled={!canEdit} onClick={startEdit}>
                            <Kbd>E</Kbd>
                            Edit
                          </Button>
                        )}
                      </>
                    )}
              </div>
            </div>
          </>
        )}
      </DialogContent>
    </Dialog>
  )
}

function KeyHint({ keys, children }: { keys: ReactNode, children: ReactNode }) {
  return (
    <span className="flex items-center gap-1.5">
      <KbdGroup>{keys}</KbdGroup>
      {children}
    </span>
  )
}

/** Warns that a changed grouping prefix moves the commit to another branch (or out of every branch). */
function PrefixChangeHint({ before, after }: { before: string, after: string }) {
  const prefix = getGroupingPrefix(after)
  if (prefix === getGroupingPrefix(before)) {
    return <span>The first line is the subject</span>
  }
  return (
    <span data-testid="commit-prefix-change" className="text-warning">
      {prefix === null ? "Saving moves the commit to the unassigned commits" : `Saving moves the commit to the branch ${prefix}`}
    </span>
  )
}
