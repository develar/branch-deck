import type { ReactNode } from "react"
import { GitPullRequestCreateIcon, ShieldAlertIcon, UploadIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { BranchReviewStackBar } from "@/components/review/BranchReviewStackBar"
import { RangeReview } from "@/components/review/RangeReview"
import { Callout } from "@/components/shared/Callout"
import type { BranchReviewAction, BranchReviewModel } from "@/hooks/useBranchReview"
import { branchGoneReason } from "@/lib/branch-review"
import { filterEmptyText, filterPaths, reviewFilterChips, sameFilter } from "@/lib/review-owners"
import type { ReviewFilter, ReviewFilterChip } from "@/lib/review-owners"

/*
 * The parts of the review of a branch (`useBranchReview`), in the dialog of the branch preview and in its window: the
 * stack bar and the owner chips, the commits of the part beside the diffs, the actions on the branch of the part.
 */

/** The stack bar and the owner chips, and the line of a branch that left */
export function BranchReviewBar({ model }: { model: BranchReviewModel }) {
  const { loaded } = model
  if (loaded === null) {
    return null
  }
  return (
    <div className="flex flex-col gap-2">
      <BranchReviewStackBar baselineBranch={model.source.baselineBranch} segments={loaded.segments} range={loaded.range} onRangeChange={model.setRange} />
      {loaded.files !== undefined && <OwnerFilter chips={reviewFilterChips(loaded.files, model.source.groups, loaded.owners)} filter={model.filter} onFilterChange={model.setFilter} />}
      {model.gone.length > 0 && <p className="text-xs text-muted-foreground" data-testid="branch-review-gone">{model.gone.map(branchGoneReason).join("; ")}</p>}
    </div>
  )
}

/** The commits of the part beside the diffs; the error, or the loading line, before they are read */
export function BranchReviewBody({ model }: { model: BranchReviewModel }) {
  const { state, loaded, filter, source } = model
  if (state.status === "error") {
    return (
      <div className="p-6">
        <Callout tone="destructive" title={`Cannot read the changes of ${source.chain.at(-1)}`}>{state.message}</Callout>
      </div>
    )
  }
  if (loaded === null) {
    return (
      <p className="flex items-center gap-2 p-6 text-sm text-muted-foreground">
        <Spinner className="size-4" />
        Loading changes…
      </p>
    )
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
    />
  )
}

/** The actions on the target branch in the footer: O or Create Pull Request, S Safe Push…, Push without a key */
export function BranchReviewActions<T extends BranchReviewAction>({ actions, branchName, onRun }: { actions: readonly T[], branchName: string, onRun: (action: T) => void }) {
  return actions.map(action => (
    <FooterAction
      key={action.id}
      testId={`branch-review-${action.id}`}
      reason={action.disabled === true ? (action.description ?? null) : null}
      description={action.description ?? (action.id === "push" ? `Pushes ${branchName}` : action.label)}
      onClick={() => onRun(action)}
    >
      <FooterActionIcon action={action} />
      {action.label}
    </FooterAction>
  ))
}

/** The key of an action (`O`, `S`), else its icon */
function FooterActionIcon({ action }: { action: BranchReviewAction }) {
  const key = action.kbds?.length === 1 ? action.kbds[0]! : null
  if (key !== null) {
    return <Kbd>{key.toUpperCase()}</Kbd>
  }
  return action.id === "create-pull-request" ? <GitPullRequestCreateIcon data-icon="inline-start" /> : <UploadIcon data-icon="inline-start" />
}

/** A ghost `xs` button of the footer, disabled with the reason on hover, its description otherwise */
function FooterAction({ testId, reason, description, onClick, children }: { testId: string, reason: string | null, description: string, onClick: () => void, children: ReactNode }) {
  const disabled = reason !== null
  return (
    <Tooltip>
      <TooltipTrigger
        render={(
          <Button
            variant="ghost"
            size="xs"
            disabled={disabled}
            // the hover says why; Base UI keeps the button focusable, so the dimming is ours
            focusableWhenDisabled
            className={cn(disabled && "opacity-50")}
            data-testid={testId}
            onClick={onClick}
          />
        )}
      >
        {children}
      </TooltipTrigger>
      <TooltipContent>{reason ?? description}</TooltipContent>
    </Tooltip>
  )
}

function OwnerFilter({ chips, filter, onFilterChange }: { chips: readonly ReviewFilterChip[], filter: ReviewFilter, onFilterChange: (filter: ReviewFilter) => void }) {
  // "All" alone filters nothing
  if (chips.length === 1) {
    return null
  }
  return (
    <div className="flex flex-wrap items-center gap-1.5" data-testid="branch-review-owners" role="group" aria-label="Files of">
      {chips.map(chip => <FilterChip key={chip.label} chip={chip} pressed={sameFilter(chip.filter, filter)} onPress={() => onFilterChange(chip.filter)} />)}
    </div>
  )
}

function FilterChip({ chip, pressed, onPress }: { chip: ReviewFilterChip, pressed: boolean, onPress: () => void }) {
  const button = (
    <Button
      size="xs"
      variant="outline"
      aria-pressed={pressed}
      disabled={chip.count === null}
      data-testid="branch-review-owner"
      data-owner={chip.label}
      className={cn(pressed && "border-primary/50 bg-primary/10 text-primary hover:bg-primary/15 hover:text-primary")}
      onClick={onPress}
    >
      {chip.needsApproval && <ShieldAlertIcon data-icon="inline-start" className="text-warning" />}
      {chip.label}
      {chip.count !== null && <span className="text-muted-foreground tabular-nums">{chip.count}</span>}
    </Button>
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
