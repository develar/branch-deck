import { useEffect, useRef, useState } from "react"
import type { KeyboardEvent } from "react"
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog"
import { Kbd, KbdGroup } from "@/components/ui/kbd"
import { CommitMessageBody } from "@/components/commit/CommitMessage"
import { CommitMetadata } from "@/components/commit/CommitMetadata"
import { CommitSubject } from "@/components/commit/CommitSubject"
import type { CommitUnion } from "@/lib/commit"
import { getCommitFileCount, getCommitHash, getCommitSubject, hasCommitMessageBody } from "@/lib/commit"

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
}

/**
 * Centered preview of one commit of a list, as Quick Look shows a file: the whole subject, the meta line and the body.
 * ↑/↓ show the previous or next commit of the list, Space and Escape close it.
 *
 * Stays mounted while closed: Base UI moves the focus into a dialog (and back) only when `open` changes.
 */
export function CommitPreviewDialog({ commits, index, onIndexChange, onClose, showAuthor, showFileCount, showNewHash }: CommitPreviewDialogProps) {
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

  // the body has the focus, so PageUp/PageDown scroll it; a newly shown commit starts at the top
  const bodyRef = useRef<HTMLDivElement>(null)
  useEffect(() => {
    bodyRef.current?.scrollTo({ top: 0 })
  }, [hash])

  const handleKeyDown = (event: KeyboardEvent) => {
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
    // Space on a button or a link presses it
    else if (event.key === " " && !(event.target instanceof Element && event.target.closest("button, a") !== null)) {
      event.preventDefault()
      onClose()
    }
  }

  return (
    <Dialog
      open={open && commit !== undefined}
      onOpenChange={(nextOpen) => {
        if (!nextOpen) {
          onClose()
        }
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
              {hasCommitMessageBody(subject, commit.message)
                ? <CommitMessageBody message={commit.message} className="text-sm leading-6" />
                : <p className="text-sm text-muted-foreground">No description</p>}
            </div>

            <div className="flex items-center justify-between gap-4 border-t border-border px-6 py-2 text-xs text-muted-foreground">
              <span data-testid="commit-preview-position">{`${currentIndex + 1} / ${commits.length}`}</span>
              <div className="flex items-center gap-3">
                {commits.length > 1 && (
                  <span className="flex items-center gap-1.5">
                    <KbdGroup>
                      <Kbd>↑</Kbd>
                      <Kbd>↓</Kbd>
                    </KbdGroup>
                    Previous/next
                  </span>
                )}
                <span className="flex items-center gap-1.5">
                  <Kbd>Space</Kbd>
                  Close
                </span>
              </div>
            </div>
          </>
        )}
      </DialogContent>
    </Dialog>
  )
}
