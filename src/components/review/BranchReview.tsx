import { useEffect } from "react"
import { CheckCheckIcon, GitMergeIcon, GitPullRequestCreateIcon, GitPullRequestIcon, InfoIcon, ShieldAlertIcon, ShieldCheckIcon, UploadIcon } from "lucide-react"
import type { LucideIcon } from "lucide-react"
import { Alert, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { FilesCount } from "@/components/commit/CommitReview"
import { BranchReviewStackBar } from "@/components/review/BranchReviewStackBar"
import { RangeReview } from "@/components/review/RangeReview"
import { ReviewPlanCard } from "@/components/review/ReviewPlanCard"
import { ActionButton } from "@/components/shared/ActionButton"
import { Callout } from "@/components/shared/Callout"
import { FilterChip, FilterChips } from "@/components/shared/FilterChips"
import { LoadingLine } from "@/components/shared/LoadingLine"
import { SectionLabel } from "@/components/shared/SectionLabel"
import type { BranchReviewAction, BranchReviewModel } from "@/hooks/useBranchReview"
import { getReviewCommitCopyMenuItems } from "@/hooks/useCommitCopyActions"
import { branchGoneReason, reviewCommitKey } from "@/lib/branch-review"
import type { MenuItemGroups } from "@/lib/menu"
import { filterEmptyText, filterPaths, reviewFilterChips, sameFilter, soleOwner } from "@/lib/review-owners"
import type { ReviewFilter, ReviewFilterChip } from "@/lib/review-owners"
import { markWindowContentReady } from "@/lib/sub-window"
import type { BranchReviewMenuItem, ReviewedBranch, WholeStackScope } from "@/types/sub-window"

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
  /** The menu of each commit of the chain, by its id, as the main window sends it */
  readonly commitMenus: Readonly<Record<string, readonly (readonly BranchReviewMenuItem[])[]>>
  /** Runs an item of the menu of a commit of `branchName` in the main window */
  readonly onCommitMenuItem: (branchName: string, commitId: string, itemId: string) => void
}

/** The stack bar: the part of the stack the review shows, the scopes of the stack */
export function BranchReviewBar({ model, moves }: { model: BranchReviewModel; moves: BranchReviewMoves }) {
  const { loaded } = model
  if (loaded === null) {
    return null
  }
  return (
    <BranchReviewStackBar
      baselineBranch={model.source.baselineBranch}
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
      <AlertTitle>{model.gone.map(branchGoneReason).join("; ")}</AlertTitle>
    </Alert>
  )
}

/** The header of the files: the owner chips, or the owner of them all ("18 files · Air Plugin") */
function BranchReviewFilesHeader({ model, count }: { model: BranchReviewModel; count: FilesCount | null }) {
  const { loaded } = model
  if (loaded === null || loaded.files === undefined) {
    return null
  }
  const chips = reviewFilterChips(loaded.files, model.source.groups, loaded.owners, model.filter)
  if (chips.length > 1) {
    return <OwnerFilter chips={chips} filter={model.filter} onFilterChange={model.setFilter} />
  }
  const owner = soleOwner(loaded.files, loaded.owners)
  return (
    <span data-testid="branch-review-files" className="flex min-w-0 flex-1">
      <SectionLabel title="Files" count={count?.shown ?? loaded.files.length} of={count === null || count.shown === count.total ? undefined : count.total} className="flex-1">
        {owner ?? undefined}
      </SectionLabel>
    </span>
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
  const { state, loaded, filter, source } = model
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
  return (
    <RangeReview
      items={items}
      selected={loaded.selected}
      onSelect={model.select}
      subject={loaded.subject}
      initialPath={model.lastPath}
      onPositionChange={model.showPosition}
      pathFilter={loaded.files === undefined ? null : filterPaths(filter, loaded.files, source.groups, loaded.owners)}
      filteredEmptyText={filterEmptyText(filter)}
      fileOwners={loaded.owners}
      rangeNoun="branch"
      filesHeader={count => <BranchReviewFilesHeader model={model} count={count} />}
      scopeTop={loaded.plan === null ? undefined : <ReviewPlanCard state={loaded.plan} />}
      commitTop={loaded.plan === null ? undefined : <ReviewPlanCard state={loaded.plan} />}
      reviewFacts={loaded.reviewFacts}
      onShowAllPaths={() => model.setFilter({ kind: "all" })}
      commitMenu={commit => {
        // the commit's branch: the segment that has it
        const branch = loaded.segments.find(segment => segment.commits.some(it => it.id === commit.id))?.name
        const groups: MenuItemGroups =
          branch === undefined || source.wholeStack === true
            ? []
            : (moves.commitMenus[commit.id] ?? []).map(group => group.map(item => ({ ...item, onSelect: () => moves.onCommitMenuItem(branch, commit.id, item.id) })))
        // as the items of the main window, which come without their icons
        return [...groups, getReviewCommitCopyMenuItems(commit).map(item => ({ ...item, icon: undefined }))]
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

/** The icons of the actions on the branch: the items come over the link of the window, without their icons */
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
}

/** The actions on the target branch in the title bar of the window: O or Create Pull Request…, S Safe Push, Push */
export function BranchReviewActions<T extends BranchReviewAction>({ actions, branchName, onRun }: { actions: readonly T[]; branchName: string; onRun: (action: T) => void }) {
  return actions.map(action => (
    <ActionButton
      key={action.id}
      data-testid={`branch-review-${action.id}`}
      label={action.label}
      icon={ACTION_ICONS[action.id]}
      keyHint={action.kbds?.length === 1 ? action.kbds[0] : undefined}
      disabled={action.disabled === true}
      description={action.description ?? (action.id === "push" ? `Pushes ${branchName}` : action.label)}
      onClick={() => onRun(action)}
    />
  ))
}

function OwnerFilter({ chips, filter, onFilterChange }: { chips: readonly ReviewFilterChip[]; filter: ReviewFilter; onFilterChange: (filter: ReviewFilter) => void }) {
  // a chip by its label, the one shown by the filter of the review
  const chosen = chips.find(chip => sameFilter(chip.filter, filter))?.label ?? ""
  return (
    <FilterChips
      label="Files of"
      value={chosen}
      onValueChange={label => {
        const chip = chips.find(it => it.label === label)
        if (chip !== undefined) {
          onFilterChange(chip.filter)
        }
      }}
      data-testid="branch-review-owners"
    >
      {chips.map(chip => (
        <OwnerChip key={chip.label} chip={chip} />
      ))}
    </FilterChips>
  )
}

function OwnerChip({ chip }: { chip: ReviewFilterChip }) {
  const button = (
    <FilterChip value={chip.label} count={chip.count} disabled={chip.count === null} data-testid="branch-review-owner" data-owner={chip.label}>
      {chip.needsApproval && <ShieldAlertIcon data-icon="inline-start" className="text-warning" />}
      {chip.label}
    </FilterChip>
  )
  if (!chip.needsApproval) {
    return button
  }
  return (
    <Tooltip>
      {/* a span: a disabled button gets no pointer events */}
      <TooltipTrigger render={<span />}>{button}</TooltipTrigger>
      <TooltipContent>{chip.count === null ? "Needs approval; Patronus doesn't tell which files" : "Needs approval for a Safe Push"}</TooltipContent>
    </Tooltip>
  )
}
