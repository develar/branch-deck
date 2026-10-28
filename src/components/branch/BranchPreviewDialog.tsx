import { Fragment, useRef, useState } from "react"
import type { KeyboardEvent, ReactNode } from "react"
import { cn } from "cn"
import { FileDiffIcon, TriangleAlertIcon } from "lucide-react"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { DialogTitle } from "@/components/ui/dialog"
import { Kbd } from "@/components/ui/kbd"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { BranchIssueBadge, BranchSummary } from "@/components/branch/BranchNameCell"
import { BranchPreviewActions, BranchPreviewMenu } from "@/components/branch/BranchPreviewActions"
import { LandedWithChip } from "@/components/branch/LandedChip"
import { OrphanedStackPullRequestNotice } from "@/components/branch/OrphanedStackPullRequest"
import { PatronusChip } from "@/components/branch/PatronusChip"
import { PullRequestChip } from "@/components/branch/PullRequestChip"
import { StackBaseLabel } from "@/components/branch/StackBaseLabel"
import { StackProblemAlert } from "@/components/branch/StackProblemAlert"
import { CommitHeading } from "@/components/commit/CommitHeading"
import { CommitMessageBody } from "@/components/commit/CommitMessage"
import { LinkedText } from "@/components/commit/LinkedText"
import { ConflictFixPanel } from "@/components/conflict/ConflictFixPanel"
import { PreviewDialogBody, PreviewDialogFooter, PreviewDialogFrame, PreviewDialogHeader, PreviewNavigationHints } from "@/components/shared/PreviewDialogFrame"
import { useBranchNextStep } from "@/hooks/useBranchNextStep"
import { ClosePreviewContext } from "@/hooks/useClosePreview"
import { useCommitFileStats, useCommitFiles } from "@/hooks/useCommitFiles"
import { useConflictBase } from "@/hooks/useConflictBase"
import { useLandedIn } from "@/hooks/useLandedIn"
import { useMissingCommitOwners } from "@/hooks/useMissingCommitOwners"
import { useRowOperation } from "@/hooks/useRowOperation"
import { requestSafePush, useLandsByPush } from "@/hooks/useSafePush"
import { getStatusBadge } from "@/hooks/useStatusBadge"
import type { MissingCommit } from "@/lib/bindings"
import { getBranchSummary } from "@/lib/branch"
import { reviewBlockerOf } from "@/lib/branch-review"
import { openBranchReviewWindow, reviewBranchCommit } from "@/lib/branch-review-window"
import { getCommitHash, getCommitSubject, hasCommitMessageBody } from "@/lib/commit"
import { formatShortHash } from "@/lib/hash"
import { formatMissingCommitsTitle } from "@/lib/merge-conflict"
import { formatLandedWith } from "@/lib/patronus"
import { handlePreviewNavigationKey, hasKeyModifier } from "@/lib/preview-keys"
import { openPullRequest } from "@/lib/pull-request"
import { pushFromButton } from "@/lib/push-run"
import { stackProblemOf } from "@/lib/stack"
import { formatTimestamp } from "@/lib/time"
import { statusDotClassName } from "@/lib/ui-types"
import { useRowPart } from "@/model/store"
import { isActionEnabled } from "@/presenters/actions"
import type { Branch, SyncedCommit } from "@/stores/branch-sync"
import { useBranch, useIsBranchPushing } from "@/stores/branch-sync"
import { useBranchRobot, useLandedBy, usePatronusAvailable } from "@/stores/patronus"
import { usePullRequest } from "@/stores/pull-requests"

export interface BranchPreviewDialogProps {
  /** The branches in the table order; ↑/↓ step through them */
  names: readonly string[]
  /** The shown branch, null while closed */
  name: string | null
  onNameChange: (name: string) => void
  onClose: () => void
  /** The pull request (title, review decision, reviewers, checks) and the Patronus robot of the branch, shown above the commits */
  renderReviewSections?: (branch: Branch) => ReactNode
}

/**
 * Centered preview of one branch, as the commit preview shows a commit: the name and the meta line, then the commits
 * newest first with their whole messages, so it reads like the description of a pull request.
 * ↑/↓ show the previous or next branch of the table, P pushes (a push that drops remote commits asks first, in the
 * description of the footer's push button), S asks the Safe Push of the row, its popover at the row's slot (the preview closes for it, as for
 * the footer's "Safe Push"), O opens the pull request, D reviews the branch in its window (the preview closes once it
 * opened), Space and Escape close it. The close focuses the row of the branch it showed last, so after ↑/↓ the keys of
 * the row (D, S, P) act on that branch.
 *
 * Stays mounted while closed (see `PreviewDialogFrame`).
 */
export function BranchPreviewDialog({ names, name, onNameChange, onClose, renderReviewSections }: BranchPreviewDialogProps) {
  const open = name !== null
  // the last shown branch stays while the closing animation runs
  const [shownName, setShownName] = useState(name)
  if (open && name !== shownName) {
    setShownName(name)
  }
  const currentName = open ? name : shownName
  const branch = useBranch(currentName ?? "")
  const index = currentName === null ? -1 : names.indexOf(currentName)

  const isPushing = useIsBranchPushing(currentName ?? "")
  const processing = useRowOperation(currentName ?? "") !== undefined
  // as Rust decides the push and the review of the branch, with what runs on the page over the push
  const actions = useRowPart(currentName ?? "", "actions")
  const canPush = branch !== undefined && isActionEnabled(actions, "push") && !isPushing && !processing
  const pullRequest = usePullRequest(currentName ?? "")
  const summary = branch === undefined ? null : getBranchSummary(branch.summary, branch.descriptionTitle)
  const patronusAvailable = usePatronusAvailable()
  const reviewBlocker = branch === undefined ? null : reviewBlockerOf(branch, actions)
  const canReview = branch !== undefined && reviewBlocker === null
  const safePushBlocker = useBranchNextStep(currentName ?? "")?.safePushBlocker ?? null
  // as the footer's "Safe Push" is shown (`SafePushButton`, where Patronus is, or Push to master where the repository
  // lands by a push); disabled, S says why
  const landsByPush = useLandsByPush()
  const offersSafePush = patronusAvailable || landsByPush

  const bodyRef = useRef<HTMLDivElement>(null)
  // the meta line of each commit shows its files
  const isLoadingFiles = useCommitFiles(open && branch !== undefined ? branch.commits.map(getCommitHash) : [])

  // the window opens: the preview stays until it did (a failure keeps it, and is told)
  const [isOpeningReview, setIsOpeningReview] = useState(false)
  // the branch, or one of its commits: the review of the branch at that commit
  const review = (commit?: SyncedCommit) => {
    if (branch === undefined || !canReview || isOpeningReview) {
      return
    }
    setIsOpeningReview(true)
    void ((commit === undefined ? null : reviewBranchCommit(branch.name, commit.originalHash, null)) ?? openBranchReviewWindow(branch.name))
      .then(opened => {
        if (opened) {
          onClose()
        }
      })
      .finally(() => setIsOpeningReview(false))
  }

  const handleKeyDown = (event: KeyboardEvent) => {
    // a key the content took
    if (event.defaultPrevented) {
      return
    }
    if (hasKeyModifier(event) || handlePreviewNavigationKey(event, index, names.length, next => onNameChange(names[next]!), onClose)) {
      return
    }
    // the physical key, in any keyboard layout
    if (event.code === "KeyP" && canPush) {
      event.preventDefault()
      pushFromButton(branch!.name, "preview")
    } else if (event.code === "KeyO" && pullRequest !== undefined) {
      event.preventDefault()
      openPullRequest(pullRequest)
    } else if (event.code === "KeyD" && canReview) {
      event.preventDefault()
      review()
    } else if (event.code === "KeyS" && offersSafePush && branch !== undefined) {
      event.preventDefault()
      // the popover of the start is the row's: the preview closes first; disabled, S says why
      if (safePushBlocker === null) {
        onClose()
      }
      void requestSafePush(branch.name, { mode: "rebaseAutosquash", via: "key" })
    }
  }

  return (
    <PreviewDialogFrame
      data-testid="branch-preview"
      open={open && branch !== undefined}
      onOpenChange={nextOpen => {
        if (!nextOpen) {
          onClose()
        }
      }}
      initialFocus={bodyRef}
      finalFocus={() => focusBranchRow(currentName)}
      onKeyDown={handleKeyDown}
    >
      {branch !== undefined && (
        // the forms of the row opened from inside (the Patronus details) close the preview first
        <ClosePreviewContext value={onClose}>
          <PreviewDialogHeader end={<BranchPreviewMenu name={branch.name} />}>
            <div className="flex min-w-0 items-center gap-2">
              <DialogTitle className="shrink-0 leading-6">
                <LinkedText text={branch.name} className="text-base font-medium" linkClassName="text-foreground hover:underline" />
              </DialogTitle>
              <StackBaseLabel name={branch.name} />
              {/* an issue branch is named after its issue */}
              {branch.issueKey !== null && branch.issueKey !== branch.name && <BranchIssueBadge issueKey={branch.issueKey} />}
              {summary !== null && <BranchSummary summary={summary} />}
            </div>
            <BranchPreviewMeta branch={branch} />
          </PreviewDialogHeader>

          <PreviewDialogBody ref={bodyRef} itemKey={branch.name} data-testid="branch-preview-body">
            <div className="flex flex-col gap-5">
              <BranchErrorNotice branch={branch} />
              <OrphanedStackPullRequestNotice root={branch.name} />
              {renderReviewSections?.(branch)}
              <ol data-testid="branch-preview-commits" aria-busy={isLoadingFiles || undefined} className="flex flex-col divide-y divide-border">
                {branch.commits.map(commit => (
                  <BranchPreviewCommit key={getCommitHash(commit)} commit={commit} onReview={canReview ? () => review(commit) : undefined} />
                ))}
              </ol>
            </div>
          </PreviewDialogBody>

          <PreviewDialogFooter
            data-testid="branch-preview-footer"
            start={<span data-testid="branch-preview-position" className="tabular-nums">{`${index + 1} / ${names.length}`}</span>}
          >
            <PreviewNavigationHints count={names.length} />
            {/* disabled with the reason, never hidden: the menu of the row says the same */}
            <Tooltip disabled={canReview}>
              <TooltipTrigger
                render={
                  <Button
                    variant="ghost"
                    size="xs"
                    disabled={!canReview || isOpeningReview}
                    focusableWhenDisabled
                    className={cn(!canReview && "opacity-50")}
                    data-testid="branch-preview-review"
                    onClick={() => review()}
                  />
                }
              >
                <Kbd>D</Kbd>
                Review Changes
              </TooltipTrigger>
              <TooltipContent>{reviewBlocker}</TooltipContent>
            </Tooltip>
            {/* the start form is a form of the row: the preview closes for it (`ClosePreviewContext`) */}
            <BranchPreviewActions branch={branch} />
          </PreviewDialogFooter>
        </ClosePreviewContext>
      )}
    </PreviewDialogFrame>
  )
}

/**
 * The close focuses the row of `name` itself and leaves the rest to nobody (false): Base UI would focus the first
 * tabbable element inside it, the expand toggle, since a row (`tabIndex={-1}`) is focusable but not tabbable. Without the
 * row (a filter hides it), the element focused before the preview opened.
 */
function focusBranchRow(name: string | null): boolean {
  const row = name === null ? null : document.querySelector<HTMLElement>(`tr[data-row-id="${CSS.escape(name)}"]`)
  if (row === null) {
    return true
  }
  row.focus({ preventScroll: true })
  return false
}

/**
 * Commit count, latest commit time, the status of the row (with its description on hover), the last push time, the
 * pull request chip, the Patronus chip and the merged Safe Push that landed the branch with another one.
 */
function BranchPreviewMeta({ branch }: { branch: Branch }) {
  const pullRequest = usePullRequest(branch.name)
  const robot = useBranchRobot(branch.name)
  const landedBy = useLandedBy(branch.name)
  const landedIn = useLandedIn(branch.name)
  const status = getStatusBadge(branch, landedIn)
  const lastPushTime = branch.remoteStatus?.lastPushTime ?? 0
  const items: ReactNode[] = []
  items.push(<span>{branch.commitCount === 1 ? "1 commit" : `${branch.commitCount} commits`}</span>)
  if (branch.latestCommitTime > 0) {
    items.push(<span>{formatTimestamp(branch.latestCommitTime)}</span>)
  }
  // the status starts with its own dot: no separator before it
  const statusIndex = items.length
  items.push(
    <Tooltip>
      <TooltipTrigger render={<span data-testid="branch-preview-status" className="flex items-center gap-1.5" />}>
        <span aria-hidden className={cn("size-1.5 shrink-0 rounded-full", statusDotClassName(status.color))} />
        <span>{status.text}</span>
      </TooltipTrigger>
      <TooltipContent className="max-w-sm whitespace-pre-line">{status.tooltip}</TooltipContent>
    </Tooltip>,
  )
  if (lastPushTime > 0) {
    items.push(<span data-testid="branch-preview-last-push">{`Pushed ${formatTimestamp(lastPushTime)}`}</span>)
  }
  if (pullRequest) {
    items.push(<PullRequestChip branchName={branch.name} />)
  }
  if (robot) {
    items.push(<PatronusChip branchName={branch.name} />)
  }
  // its own robot's branch landed whole: the Patronus chip says "Merged"
  if (landedBy !== null && formatLandedWith(landedBy) !== null) {
    items.push(<LandedWithChip landed={landedBy} />)
  }

  return (
    <div className="flex min-w-0 flex-wrap items-center gap-x-2 text-xs text-muted-foreground">
      {/* the items never reorder, so the index is a stable key */}
      {items.map((item, itemIndex) => (
        <Fragment key={itemIndex}>
          {itemIndex > 0 && itemIndex !== statusIndex && <span>·</span>}
          {item}
        </Fragment>
      ))}
    </div>
  )
}

/**
 * A merge conflict or a sync error of the branch: for a conflict, where the commits it misses are and the fix; the
 * expanded row shows the files.
 */
function BranchErrorNotice({ branch }: { branch: Branch }) {
  // the parent of a stacked branch, else the baseline: what the expanded row says too
  const conflictBase = useConflictBase(branch.name)
  const genericError = branch.errorDetails?.Generic
  const conflictCommit = branch.commits.find(commit => commit.error?.MergeConflict != null)
  const conflict = conflictCommit?.error?.MergeConflict ?? null
  const owners = useMissingCommitOwners(conflict?.conflictAnalysis.missingCommits ?? NO_MISSING_COMMITS)
  const problem = stackProblemOf(branch)
  if (problem !== null) {
    return <StackProblemAlert branch={branch} problem={problem} />
  }
  if (genericError === undefined && conflictCommit === undefined && branch.status !== "MergeConflict") {
    return null
  }
  if (genericError !== undefined) {
    return (
      <Alert variant="destructive" data-testid="branch-preview-conflict">
        <TriangleAlertIcon />
        <AlertTitle>Sync failed</AlertTitle>
        <AlertDescription>{genericError}</AlertDescription>
      </Alert>
    )
  }

  return (
    <div className="flex flex-col gap-2">
      <Alert variant="destructive" data-testid="branch-preview-conflict">
        <TriangleAlertIcon />
        <AlertTitle>Merge conflict</AlertTitle>
        <AlertDescription>
          {conflictCommit === undefined
            ? "A commit of the branch does not apply cleanly."
            : `“${getCommitSubject(conflictCommit)}” does not apply cleanly onto ${conflictBase ?? "the baseline"}: ${lowerFirst(formatMissingCommitsTitle(owners))}.`}
        </AlertDescription>
      </Alert>
      {conflict !== null && <ConflictFixPanel conflict={conflict} branchName={branch.name} builtOn={conflictBase} owners={owners} />}
    </div>
  )
}

const NO_MISSING_COMMITS: readonly MissingCommit[] = []

function lowerFirst(text: string): string {
  // "No commit outside…" reads on after a colon; a number or a branch name starts the other titles
  return text.startsWith("No ") ? `no ${text.slice(3)}` : text
}

function BranchPreviewCommit({ commit, onReview }: { commit: SyncedCommit; onReview: (() => void) | undefined }) {
  const subject = getCommitSubject(commit)
  const hash = getCommitHash(commit)
  const fileStats = useCommitFileStats(hash)
  return (
    <li data-testid="branch-preview-commit" className="py-4 first:pt-0 last:pb-0">
      <CommitHeading
        subject={subject}
        badge={
          <>
            {commit.error?.MergeConflict != null && <Badge variant="destructive">Merge conflict</Badge>}
            {/* the commit in the review of the branch, as D on its row opens it */}
            {onReview !== undefined && (
              <Tooltip>
                <TooltipTrigger
                  render={
                    <Button
                      size="icon-xs"
                      variant="ghost"
                      className="ml-auto shrink-0 text-muted-foreground"
                      aria-label={`Review Changes of ${formatShortHash(hash)}`}
                      data-testid="branch-preview-commit-review"
                      onClick={onReview}
                    />
                  }
                >
                  <FileDiffIcon />
                </TooltipTrigger>
                <TooltipContent>Review Changes</TooltipContent>
              </Tooltip>
            )}
          </>
        }
        hash={hash}
        newHash={commit.hash}
        authorTime={commit.authorTime}
        committerTime={commit.committerTime}
        fileStats={fileStats}
      >
        {hasCommitMessageBody(subject, commit.message) && <CommitMessageBody message={commit.message} size="reading" />}
      </CommitHeading>
    </li>
  )
}
