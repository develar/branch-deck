import { Fragment, useRef, useState } from "react"
import type { KeyboardEvent, ReactNode } from "react"
import { TriangleAlertIcon } from "lucide-react"
import { cn } from "cn"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { DialogTitle } from "@/components/ui/dialog"
import { Kbd } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { BranchIssueBadge, BranchSummary } from "@/components/branch/BranchNameCell"
import { CommitMessageBody } from "@/components/commit/CommitMessage"
import { CommitMetadata } from "@/components/commit/CommitMetadata"
import { CommitSubject } from "@/components/commit/CommitSubject"
import { LinkedText } from "@/components/commit/LinkedText"
import {
  PreviewDialogBody,
  PreviewDialogFooter,
  PreviewDialogFrame,
  PreviewDialogHeader,
  PreviewNavigationHints,
} from "@/components/shared/PreviewDialogFrame"
import { getPushButtonState } from "@/hooks/usePushButton"
import { getStatusBadge } from "@/hooks/useStatusBadge"
import { getCommitHash, getCommitSubject, hasCommitMessageBody } from "@/lib/commit"
import { pushBranch } from "@/lib/git/push"
import { handlePreviewNavigationKey, hasKeyModifier } from "@/lib/preview-keys"
import { formatTimestamp } from "@/lib/time"
import { statusDotClassName } from "@/lib/ui-types"
import type { Branch, SyncedCommit } from "@/stores/branch-sync"
import { useBranch, useBranchSync, useIsBranchPushing } from "@/stores/branch-sync"
import { useIsRowProcessing } from "@/stores/inline-row-action"

export interface BranchPreviewDialogProps {
  /** The branches in the table order; ↑/↓ step through them */
  names: readonly string[]
  /** The shown branch, null while closed */
  name: string | null
  onNameChange: (name: string) => void
  onClose: () => void
  /** The pull request of the branch (title, review decision, reviewers, checks), shown above the commits */
  renderPullRequestSection?: (branch: Branch) => ReactNode
}

/**
 * Centered preview of one branch, as the commit preview shows a commit: the name and the meta line, then the commits
 * newest first with their whole messages, so it reads like the description of a pull request.
 * ↑/↓ show the previous or next branch of the table, P pushes, Space and Escape close it.
 *
 * Stays mounted while closed (see `PreviewDialogFrame`).
 */
export function BranchPreviewDialog({ names, name, onNameChange, onClose, renderPullRequestSection }: BranchPreviewDialogProps) {
  const open = name !== null
  // the last shown branch stays while the closing animation runs
  const [shownName, setShownName] = useState(name)
  if (open && name !== shownName) {
    setShownName(name)
  }
  const currentName = open ? name : shownName
  const branch = useBranch(currentName ?? "")
  const index = currentName === null ? -1 : names.indexOf(currentName)

  const isSyncing = useBranchSync(state => state.isSyncing)
  const isPushing = useIsBranchPushing(currentName ?? "")
  const processing = useIsRowProcessing(currentName ?? "")
  // as the push button of the row: only a branch that synced without an error has something to push
  const push = branch === undefined || branch.hasError || branch.commitCount === 0 ? null : getPushButtonState(branch, isSyncing, isPushing)
  const canPush = push !== null && !push.disabled && !processing

  const bodyRef = useRef<HTMLDivElement>(null)

  const handleKeyDown = (event: KeyboardEvent) => {
    if (hasKeyModifier(event) || handlePreviewNavigationKey(event, index, names.length, next => onNameChange(names[next]!), onClose)) {
      return
    }
    // the physical key, in any keyboard layout
    if (event.code === "KeyP" && canPush) {
      event.preventDefault()
      void pushBranch(branch!.name)
    }
  }

  return (
    <PreviewDialogFrame
      data-testid="branch-preview"
      open={open && branch !== undefined}
      onOpenChange={(nextOpen) => {
        if (!nextOpen) {
          onClose()
        }
      }}
      initialFocus={bodyRef}
      onKeyDown={handleKeyDown}
    >
      {branch !== undefined && (
        <>
          <PreviewDialogHeader>
            <div className="flex min-w-0 items-center gap-2">
              <DialogTitle className="shrink-0 leading-5">
                <LinkedText text={branch.name} className="text-sm font-medium" linkClassName="text-foreground hover:underline" />
              </DialogTitle>
              {/* an issue branch is named after its issue */}
              {branch.issueKey !== null && branch.issueKey !== branch.name && <BranchIssueBadge issueKey={branch.issueKey} />}
              {branch.summary && <BranchSummary summary={branch.summary} />}
            </div>
            <BranchPreviewMeta branch={branch} />
          </PreviewDialogHeader>

          <PreviewDialogBody ref={bodyRef} itemKey={branch.name} data-testid="branch-preview-body">
            <div className="flex flex-col gap-5">
              <BranchErrorNotice branch={branch} />
              {renderPullRequestSection?.(branch)}
              <ol data-testid="branch-preview-commits" className="flex flex-col divide-y divide-border">
                {branch.commits.map(commit => <BranchPreviewCommit key={getCommitHash(commit)} commit={commit} />)}
              </ol>
            </div>
          </PreviewDialogBody>

          <PreviewDialogFooter start={<span data-testid="branch-preview-position">{`${index + 1} / ${names.length}`}</span>}>
            <PreviewNavigationHints count={names.length} />
            {push !== null && (
              <Button variant="ghost" size="xs" disabled={!canPush} aria-busy={isPushing || undefined} onClick={() => void pushBranch(branch.name)}>
                {isPushing ? <Spinner data-icon="inline-start" /> : <Kbd>P</Kbd>}
                {push.text}
              </Button>
            )}
          </PreviewDialogFooter>
        </>
      )}
    </PreviewDialogFrame>
  )
}

/** Commit count, latest commit time, the status of the row (with its description on hover) and the last push time. */
function BranchPreviewMeta({ branch }: { branch: Branch }) {
  const status = getStatusBadge(branch)
  const lastPushTime = branch.remoteStatus?.lastPushTime ?? 0
  const items: ReactNode[] = [<span>{branch.commitCount === 1 ? "1 commit" : `${branch.commitCount} commits`}</span>]
  if (branch.latestCommitTime > 0) {
    items.push(<span>{formatTimestamp(branch.latestCommitTime)}</span>)
  }
  items.push(
    <Tooltip>
      <TooltipTrigger render={<span data-testid="branch-preview-status" className="flex items-center gap-1.5" />}>
        <span aria-hidden className={cn("size-1.5 shrink-0 rounded-full", statusDotClassName(status.color))} />
        <span className="lowercase">{status.text}</span>
      </TooltipTrigger>
      <TooltipContent className="max-w-sm whitespace-pre-line">{status.tooltip}</TooltipContent>
    </Tooltip>,
  )
  if (lastPushTime > 0) {
    items.push(<span data-testid="branch-preview-last-push">{`Pushed ${formatTimestamp(lastPushTime)}`}</span>)
  }

  return (
    <div className="flex min-w-0 flex-wrap items-center gap-x-2 text-xs text-muted-foreground">
      {/* the items never reorder, so the index is a stable key */}
      {items.map((item, itemIndex) => (
        <Fragment key={itemIndex}>
          {itemIndex > 0 && <span>•</span>}
          {item}
        </Fragment>
      ))}
    </div>
  )
}

/** A merge conflict or a sync error of the branch; the expanded row shows the details. */
function BranchErrorNotice({ branch }: { branch: Branch }) {
  const baselineBranch = useBranchSync(state => state.baselineBranch)
  const genericError = branch.errorDetails?.Generic
  const conflictCommit = branch.commits.find(commit => commit.error?.MergeConflict != null)
  if (genericError === undefined && conflictCommit === undefined && branch.status !== "MergeConflict") {
    return null
  }

  return (
    <Alert variant="destructive" data-testid="branch-preview-conflict" className="border-transparent bg-destructive/10">
      <TriangleAlertIcon />
      {genericError === undefined
        ? (
            <>
              <AlertTitle>Merge conflict</AlertTitle>
              <AlertDescription>
                {conflictCommit === undefined
                  ? "A commit of the branch does not apply cleanly. Expand the branch to see the conflicting files."
                  : `“${getCommitSubject(conflictCommit)}” does not apply cleanly onto ${baselineBranch ?? "the baseline"}. Expand the branch to see the conflicting files.`}
              </AlertDescription>
            </>
          )
        : (
            <>
              <AlertTitle>Sync failed</AlertTitle>
              <AlertDescription>{genericError}</AlertDescription>
            </>
          )}
    </Alert>
  )
}

function BranchPreviewCommit({ commit }: { commit: SyncedCommit }) {
  const subject = getCommitSubject(commit)
  return (
    <li data-testid="branch-preview-commit" className="flex flex-col gap-2 py-4 first:pt-0 last:pb-0">
      <div className="flex flex-col gap-1">
        <div className="flex items-start gap-2">
          <CommitSubject subject={subject} clamp={false} />
          {commit.error?.MergeConflict != null && <Badge variant="destructive">Merge conflict</Badge>}
        </div>
        <CommitMetadata hash={getCommitHash(commit)} newHash={commit.hash} authorTime={commit.authorTime} committerTime={commit.committerTime} />
      </div>
      {hasCommitMessageBody(subject, commit.message) && <CommitMessageBody message={commit.message} className="text-sm leading-6" />}
    </li>
  )
}
