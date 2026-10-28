import { useRef, useState } from "react"
import type { KeyboardEvent } from "react"
import { TriangleAlertIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { DialogTitle } from "@/components/ui/dialog"
import { Kbd } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"
import { CommitSubject } from "@/components/commit/CommitSubject"
import { PrefixChangeHint } from "@/components/commit/PrefixChangeHint"
import { LockWaitNotice } from "@/components/shared/LockWaitNotice"
import { KeyHint, PreviewDialogBody, PreviewDialogFooter, PreviewDialogFrame, PreviewDialogHeader } from "@/components/shared/PreviewDialogFrame"
import { squashCommits } from "@/hooks/useSquashCommits"
import type { Commit } from "@/lib/bindings"
import { getCommitHash, getCommitSubject } from "@/lib/commit"
import { formatShortHash } from "@/lib/hash"
import { isMac } from "@/lib/platform"
import { buildSquashMessage, countPushedCommits } from "@/lib/squash-message"
import { useBranchSync } from "@/stores/branch-sync"
import { useIndicatorClaim, useSurfaceBusy } from "@/stores/indicators"

export interface SquashCommitsDialogProps {
  /** The commits to squash in list order, newest first; null while closed */
  commits: readonly Commit[] | null
  /** The branch of the commits; null for the unassigned commits */
  branchName: string | null
  onClose: () => void
  /** The commits were squashed; the dialog should close */
  onSquashed: () => void
}

/** The lists show the newest commit first */
function oldestFirst(commits: readonly Commit[]): Commit[] {
  return commits.toReversed()
}

/**
 * Edits the message of the commit that the selected commits are squashed into (the oldest one), prefilled with all
 * their messages. ⌘↵ squashes, Escape cancels.
 *
 * Keep it mounted while closed (see `PreviewDialogFrame`).
 */
/** The surface of the squash this dialog starts */
const SQUASH_SURFACE = "squash-dialog"

export function SquashCommitsDialog({ commits, branchName, onClose, onSquashed }: SquashCommitsDialogProps) {
  const open = commits !== null
  // the last commits stay while the closing animation runs; new commits start with a new message
  const [shownCommits, setShownCommits] = useState(commits)
  const [draft, setDraft] = useState(() => (commits === null ? "" : buildSquashMessage(oldestFirst(commits).map(commit => commit.message))))
  if (commits !== null && commits !== shownCommits) {
    setShownCommits(commits)
    setDraft(buildSquashMessage(oldestFirst(commits).map(commit => commit.message)))
  }
  // as the message reads; the target is the oldest
  const shown = shownCommits === null ? [] : oldestFirst(shownCommits)
  const target = shown[0]

  useIndicatorClaim(SQUASH_SURFACE)
  const isSquashing = useSurfaceBusy(SQUASH_SURFACE) !== null
  const remoteStatus = useBranchSync(state => (branchName === null ? null : state.branches[branchName]?.remoteStatus))
  const pushedCount = countPushedCommits(shown, remoteStatus)
  // a squash during a sync goes to the session at once: it is named on the table the dialog shows
  const canSquash = open && !isSquashing && draft.trim() !== ""

  const editorRef = useRef<HTMLTextAreaElement>(null)
  const bodyRef = useRef<HTMLDivElement>(null)

  const squash = async () => {
    if (!canSquash) {
      return
    }
    const squashed = await squashCommits(shown, draft, SQUASH_SURFACE)
    if (squashed) {
      onSquashed()
    }
  }

  const handleKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault()
      void squash()
    }
  }

  return (
    <PreviewDialogFrame
      data-testid="squash-dialog"
      open={open}
      onOpenChange={(nextOpen, eventDetails) => {
        if (nextOpen) {
          return
        }
        // a click outside keeps the edited message; nothing closes the dialog while the squash runs
        if (eventDetails.reason === "outside-press" || isSquashing) {
          eventDetails.cancel()
          return
        }
        onClose()
      }}
      initialFocus={editorRef}
      onKeyDown={handleKeyDown}
    >
      <PreviewDialogHeader>
        <DialogTitle className="leading-5">{`Squash ${shown.length} commits`}</DialogTitle>
        <div className="flex flex-col gap-2">
          {target !== undefined && (
            <p className="text-xs text-muted-foreground">
              {"Into the oldest commit, "}
              <span className="font-mono">{formatShortHash(getCommitHash(target))}</span>, which keeps its place and its author
            </p>
          )}
          <ul data-testid="squash-commits" className="flex max-h-32 flex-col gap-1 overflow-y-auto">
            {shown.map(commit => {
              const hash = getCommitHash(commit)
              return (
                <li key={hash} className="flex min-w-0 items-baseline gap-2">
                  <span className="shrink-0 font-mono text-xs text-muted-foreground">{formatShortHash(hash)}</span>
                  <span className="min-w-0 truncate">
                    <CommitSubject subject={getCommitSubject(commit)} clamp={false} />
                  </span>
                </li>
              )
            })}
          </ul>
        </div>
      </PreviewDialogHeader>

      <PreviewDialogBody ref={bodyRef} itemKey={target === undefined ? "" : getCommitHash(target)}>
        <Textarea
          ref={editorRef}
          aria-label="Commit message"
          data-testid="squash-message-editor"
          // the raw message, as git stores it: the first line is the subject, the grouping prefix included
          className="min-h-48 font-mono"
          value={draft}
          disabled={isSquashing}
          spellCheck
          onChange={event => setDraft(event.target.value)}
        />
      </PreviewDialogBody>

      <PreviewDialogFooter
        start={
          <div className="flex min-w-0 flex-col gap-0.5">
            <LockWaitNotice surface={SQUASH_SURFACE} />
            {target !== undefined && <PrefixChangeHint before={target.message} after={draft} action="Squashing" />}
            {pushedCount > 0 && (
              <span data-testid="squash-pushed-warning" className="flex items-center gap-1.5 text-warning">
                <TriangleAlertIcon className="size-3 shrink-0" />
                {pushedCount === 1
                  ? "1 squashed commit is already pushed; the next push replaces it"
                  : `${pushedCount} squashed commits are already pushed; the next push replaces them`}
              </span>
            )}
          </div>
        }
      >
        <KeyHint keys={<Kbd>Esc</Kbd>}>Cancel</KeyHint>
        <Button size="xs" disabled={!canSquash} onClick={() => void squash()}>
          {isSquashing && <Spinner data-icon="inline-start" />}
          Squash
          <Kbd>{isMac ? "⌘↵" : "Ctrl+Enter"}</Kbd>
        </Button>
      </PreviewDialogFooter>
    </PreviewDialogFrame>
  )
}
