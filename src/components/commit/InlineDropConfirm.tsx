import { Trash2Icon } from "lucide-react"
import { CommitSubject } from "@/components/commit/CommitSubject"
import { Callout } from "@/components/shared/Callout"
import { InlineForm } from "@/components/shared/InlineForm"
import { LockWaitNotice } from "@/components/shared/LockWaitNotice"
import { dropCommits } from "@/hooks/useDropCommits"
import type { Commit } from "@/lib/bindings"
import { formatCommitCount, getCommitHash, getCommitSubject } from "@/lib/commit"
import { formatShortHash } from "@/lib/hash"
import { useIndicatorClaim, useSurfaceBusy } from "@/stores/indicators"

/** The surface of the drop this form starts */
const DROP_SURFACE = "drop-commits"

export interface InlineDropConfirmProps {
  /** The commits to drop, in list order */
  commits: readonly Commit[]
  onCancel: () => void
  /** The commits are gone; the form closes */
  onDropped: () => void
}

/**
 * The confirmation of Drop Commit below the lead row of the selection: the commits, what leaves with them, Drop Commits
 * (the focus on it, Enter confirms), Cancel or Escape
 */
export function InlineDropConfirm({ commits, onCancel, onDropped }: InlineDropConfirmProps) {
  useIndicatorClaim(DROP_SURFACE)
  const isDropping = useSurfaceBusy(DROP_SURFACE) !== null
  const count = formatCommitCount(commits.length)
  return (
    <InlineForm
      title={commits.length === 1 ? "Drop commit" : `Drop ${count}`}
      dataTestId="drop-commits-form"
      canSubmit={!isDropping}
      loading={isDropping}
      submitText={commits.length === 1 ? "Drop Commit" : "Drop Commits"}
      submitIcon={<Trash2Icon data-icon="inline-start" />}
      submitVariant="destructive"
      focusSubmit
      onCancel={onCancel}
      onSubmit={() => {
        void dropCommits(commits, DROP_SURFACE).then(dropped => {
          if (dropped) {
            onDropped()
          }
        })
      }}
    >
      <ul data-testid="drop-commits" className="flex max-h-32 flex-col gap-1 overflow-y-auto">
        {commits.map(commit => {
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
      <Callout tone="warning" title={`This removes ${count} and ${commits.length === 1 ? "its" : "their"} changes from HEAD`}>
        <p>
          The files they changed go back to how they are without them, unless the commits cancel each other (a commit and its revert); uncommitted changes to those files stop the
          drop. Undo brings them back. A pushed branch needs a push again, and its open pull request keeps the commits until then.
        </p>
      </Callout>
      <LockWaitNotice surface={DROP_SURFACE} />
    </InlineForm>
  )
}
