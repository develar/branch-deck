import { useEffect } from "react"
import { ArchiveRestoreIcon, CheckCheckIcon, GitMergeIcon, GitPullRequestCreateIcon, GitPullRequestIcon, InfoIcon, ShieldCheckIcon, UploadIcon } from "lucide-react"
import type { LucideIcon } from "lucide-react"
import { Alert, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { BranchReviewStackBar } from "@/components/review/BranchReviewStackBar"
import { RangeReview } from "@/components/review/RangeReview"
import { ReviewOwnerChips } from "@/components/review/ReviewOwnerChips"
import { ReviewPlanCard } from "@/components/review/ReviewPlanCard"
import { ActionButton } from "@/components/shared/ActionButton"
import { Callout } from "@/components/shared/Callout"
import { LoadingLine } from "@/components/shared/LoadingLine"
import type { BranchReviewAction, BranchReviewModel } from "@/hooks/useBranchReview"
import { getReviewCommitCopyMenuItems } from "@/hooks/useCommitCopyActions"
import { branchGoneReason, formatKeptOrigin, keptGoneReason, reviewCommitKey } from "@/lib/branch-review"
import type { MenuItemGroups } from "@/lib/menu"
import { reviewOwnerMarks } from "@/lib/review-owners"
import type { ReviewOwners } from "@/lib/review-owners"
import { markWindowContentReady } from "@/lib/sub-window"
import type { ReviewedBranch, WholeStackScope } from "@/types/sub-window"

/*
 * The parts of the review of a branch (`useBranchReview`), in the dialog of the branch preview and in its window: the
 * stack bar (the title bar of the window, the header of the dialog), the commits of the part above the files and the
 * owner chips beside the diffs, the actions on the branch of the part.
 */

/** How the review moves to another scope of the stack: up to the whole stack, down to a branch of it */
export interface BranchReviewMoves {
  /** "Whole Stack" of the stack bar of a chain; null where it holds nothing more */
  readonly wholeStack: WholeStackScope | null
  readonly onWholeStack: () => void
  /** The review of the whole stack: the chain it came from */
  readonly from: readonly ReviewedBranch[]
  /** The branch of each commit of the whole stack, by `reviewCommitKey` */
  readonly stackCommits: Readonly<Record<string, string>>
  /** The review of a branch of the stack, at a commit of the whole stack (`commitKey`) */
  readonly onBranch: (name: string, commitKey?: string) => void
  /** The menu of each commit of the chain, by its id in the virtual branch, as the window runs it (`useReviewActions`) */
  readonly commitMenus: Readonly<Record<string, MenuItemGroups>>
}

/** The stack bar: the part of the stack the review shows, the scopes of the stack */
export function BranchReviewBar({ model, moves }: { model: BranchReviewModel; moves: BranchReviewMoves }) {
  const { loaded } = model
  if (loaded === null) {
    return null
  }
  const kept = model.source.kept
  const base = loaded.segments[0]?.base
  return (
    <BranchReviewStackBar
      // a kept branch is read on its own base, not on today's baseline
      baselineBranch={kept === undefined ? model.source.baselineBranch : formatKeptOrigin(kept, base)}
      segments={loaded.segments}
      range={loaded.range}
      onRangeChange={model.setRange}
      wholeStack={model.source.wholeStack === true}
      wholeStackScope={moves.wholeStack}
      onWholeStack={moves.onWholeStack}
      from={moves.from}
      onBranch={name => moves.onBranch(name)}
    />
  )
}

/**
 * "auth left the stack (landed or renamed)": the branches of the chain the last sync no longer built, an alert on the
 * canvas of the window
 */
export function BranchReviewGone({ model }: { model: BranchReviewModel }) {
  if (model.gone.length === 0) {
    return null
  }
  return (
    <Alert variant="info" data-testid="branch-review-gone">
      <InfoIcon />
      <AlertTitle>{model.gone.map(name => (model.source.kept === undefined ? branchGoneReason(name) : keptGoneReason(model.source.kept, name))).join("; ")}</AlertTitle>
    </Alert>
  )
}

/** Mark Reviewed of the title bar, with the review plan on: the next review shows what changed since */
export function BranchReviewMarkReviewed({ model }: { model: BranchReviewModel }) {
  const markReviewed = model.loaded?.markReviewed
  if (markReviewed == null) {
    return null
  }
  return (
    <ActionButton
      data-testid="branch-review-mark-reviewed"
      label="Mark Reviewed"
      icon={CheckCheckIcon}
      description={`Remembers ${model.loaded!.target} as reviewed: its next review shows what changed since`}
      onClick={markReviewed}
    />
  )
}

/** The commits of the part beside the diffs; the error, or the loading line, before they are read */
export function BranchReviewBody({ model, moves }: { model: BranchReviewModel; moves: BranchReviewMoves }) {
  const { state, loaded, source } = model
  // the error is the content of the window (the window of a review shows by its content, `readyByContent`)
  const failed = state.status === "error"
  useEffect(() => {
    if (failed) {
      markWindowContentReady()
    }
  }, [failed])
  if (state.status === "error") {
    return (
      <div className="p-6">
        <Callout tone="destructive" title={`Cannot read the changes of ${source.chain.at(-1)}`}>
          {state.message}
        </Callout>
      </div>
    )
  }
  if (loaded === null) {
    return <LoadingLine className="p-6">Loading changes…</LoadingLine>
  }
  // the commits of the whole stack come in the order of the history: each says its branch, and opens its review
  const items =
    source.wholeStack === true ? loaded.items.map(item => (item.kind === "commit" ? { ...item, branch: moves.stackCommits[reviewCommitKey(item.commit)] } : item)) : loaded.items
  const owners: ReviewOwners = { files: loaded.files, groups: source.groups, owners: loaded.owners }
  return (
    <RangeReview
      items={items}
      selected={loaded.selected}
      onSelect={model.select}
      subject={loaded.subject}
      initialPath={model.lastPath}
      onPositionChange={model.showPosition}
      ownerMarks={reviewOwnerMarks(model.filter, owners)}
      fileOwners={loaded.owners}
      rangeNoun="branch"
      filesHeader={count => <ReviewOwnerChips of={owners} filter={model.filter} onFilterChange={model.setFilter} count={count} />}
      scopeTop={loaded.plan === null ? undefined : <ReviewPlanCard state={loaded.plan} />}
      commitTop={loaded.plan === null ? undefined : <ReviewPlanCard state={loaded.plan} />}
      reviewFacts={loaded.reviewFacts}
      commitMenu={commit => {
        // the menu of a commit of a branch of the chain, not of a commit of the whole stack (its copies carry no branch)
        const groups: MenuItemGroups = source.wholeStack === true ? [] : (moves.commitMenus[commit.id] ?? [])
        return [...groups, getReviewCommitCopyMenuItems(commit)]
      }}
      commitAction={item => {
        const branch = item.branch
        return (
          branch !== undefined && (
            <Button size="xs" variant="ghost" data-testid="branch-review-commit-branch" onClick={() => moves.onBranch(branch, reviewCommitKey(item.commit))}>
              {`Review ${branch}`}
            </Button>
          )
        )
      }}
    />
  )
}

/** The icons of the actions on the branch, for an item that comes without its own */
const ACTION_ICONS: Readonly<Record<string, LucideIcon>> = {
  "open-pull-request": GitPullRequestIcon,
  "create-pull-request": GitPullRequestCreateIcon,
  "safe-push": ShieldCheckIcon,
  "push-to-baseline": GitMergeIcon,
  "merge-pull-request": GitMergeIcon,
  push: UploadIcon,
  "open-stack-pull-request": GitPullRequestIcon,
  "create-stack-pull-request": GitPullRequestCreateIcon,
  "safe-push-whole-stack": ShieldCheckIcon,
  "push-with-children": UploadIcon,
  reapply: ArchiveRestoreIcon,
}

/** The actions on the target branch in the title bar of the window: O or Create Pull Request…, S Safe Push, Push */
export function BranchReviewActions<T extends BranchReviewAction & { readonly icon?: LucideIcon }>({
  actions,
  branchName,
  onRun,
}: {
  actions: readonly T[]
  branchName: string
  onRun: (action: T) => void
}) {
  return actions.map(action => (
    <ActionButton
      key={action.id}
      data-testid={`branch-review-${action.id}`}
      label={action.label}
      icon={action.icon ?? ACTION_ICONS[action.id]}
      keyHint={action.kbds?.length === 1 ? action.kbds[0] : undefined}
      disabled={action.disabled === true}
      description={action.description ?? (action.id === "push" ? `Pushes ${branchName}` : action.label)}
      onClick={() => onRun(action)}
    />
  ))
}
