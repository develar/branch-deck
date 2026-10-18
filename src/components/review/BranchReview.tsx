import { useEffect } from "react"
import { GitPullRequestCreateIcon, GitPullRequestIcon, InfoIcon, ShieldAlertIcon, ShieldCheckIcon, UploadIcon } from "lucide-react"
import type { LucideIcon } from "lucide-react"
import { Alert, AlertTitle } from "@/components/ui/alert"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { BranchReviewStackBar } from "@/components/review/BranchReviewStackBar"
import { RangeReview } from "@/components/review/RangeReview"
import { ActionButton } from "@/components/shared/ActionButton"
import { Callout } from "@/components/shared/Callout"
import { ChoiceChip } from "@/components/shared/ChoiceChip"
import { LoadingLine } from "@/components/shared/LoadingLine"
import { SectionLabel } from "@/components/shared/SectionLabel"
import type { BranchReviewAction, BranchReviewModel } from "@/hooks/useBranchReview"
import { branchGoneReason } from "@/lib/branch-review"
import { filterEmptyText, filterPaths, reviewFilterChips, sameFilter, soleOwner } from "@/lib/review-owners"
import type { ReviewFilter, ReviewFilterChip } from "@/lib/review-owners"
import { markSubWindowContentReady } from "@/lib/sub-window"

/*
 * The parts of the review of a branch (`useBranchReview`), in the dialog of the branch preview and in its window: the
 * stack bar (the title bar of the window, the header of the dialog), the commits of the part above the files and the
 * owner chips beside the diffs, the actions on the branch of the part.
 */

/** The stack bar: the part of the stack the review shows */
export function BranchReviewBar({ model }: { model: BranchReviewModel }) {
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
function BranchReviewFilesHeader({ model }: { model: BranchReviewModel }) {
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
      <SectionLabel title="Files" count={loaded.files.length} className="flex-1">
        {owner ?? undefined}
      </SectionLabel>
    </span>
  )
}

/** The commits of the part beside the diffs; the error, or the loading line, before they are read */
export function BranchReviewBody({ model }: { model: BranchReviewModel }) {
  const { state, loaded, filter, source } = model
  // the error is the content of the window (the window of a review shows by its content, `readyByContent`)
  const failed = state.status === "error"
  useEffect(() => {
    if (failed) {
      markSubWindowContentReady()
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
  return (
    <RangeReview
      items={loaded.items}
      selected={loaded.selected}
      onSelect={model.select}
      subject={loaded.subject}
      initialPath={model.lastPath}
      onPositionChange={model.showPosition}
      pathFilter={loaded.files === undefined ? null : filterPaths(filter, loaded.files, source.groups, loaded.owners)}
      filteredEmptyText={filterEmptyText(filter)}
      fileOwners={loaded.owners}
      rangeNoun="branch"
      filesHeader={<BranchReviewFilesHeader model={model} />}
    />
  )
}

/** The icons of the actions on the branch: the items come over the link of the window, without their icons */
const ACTION_ICONS: Readonly<Record<string, LucideIcon>> = {
  "open-pull-request": GitPullRequestIcon,
  "create-pull-request": GitPullRequestCreateIcon,
  "safe-push": ShieldCheckIcon,
  push: UploadIcon,
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
  return (
    <div className="flex flex-wrap items-center gap-1.5" data-testid="branch-review-owners" role="group" aria-label="Files of">
      {chips.map(chip => (
        <FilterChip key={chip.label} chip={chip} pressed={sameFilter(chip.filter, filter)} onPress={() => onFilterChange(chip.filter)} />
      ))}
    </div>
  )
}

function FilterChip({ chip, pressed, onPress }: { chip: ReviewFilterChip; pressed: boolean; onPress: () => void }) {
  const button = (
    <ChoiceChip chosen={pressed} disabled={chip.count === null} data-testid="branch-review-owner" data-owner={chip.label} onClick={onPress}>
      {chip.needsApproval && <ShieldAlertIcon data-icon="inline-start" className="text-warning" />}
      {chip.label}
      {chip.count !== null && <span className="tabular-nums opacity-70">{chip.count}</span>}
    </ChoiceChip>
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
