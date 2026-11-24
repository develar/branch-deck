import { useState } from "react"
import { Combobox as ComboboxPrimitive } from "@base-ui/react/combobox"
import { CheckIcon, ShieldAlertIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Combobox, ComboboxContent, ComboboxEmpty, ComboboxInput, ComboboxItem, ComboboxList } from "@/components/ui/combobox"
import { toggleVariants } from "@/components/ui/toggle"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { FilesCount } from "@/components/commit/CommitReview"
import { FilterChip, FilterChips } from "@/components/shared/FilterChips"
import { SectionLabel } from "@/components/shared/SectionLabel"
import { canonicalOwnerFilter, ownerChipKey, ownerChipRow, ownerEmptyText, ownerPaths, reviewFilterChips, soleOwner } from "@/lib/review-owners"
import type { ReviewFilter, ReviewFilterChip, ReviewOwners } from "@/lib/review-owners"

/*
 * The owner chips of a review window, the branch window and the pull request window alike (openspec/specs/branch/branch-review/spec.md,
 * "Code owners"): the header of its files; the files the chosen chip marks are `reviewOwnerMarks`.
 */

/**
 * The header of the files: the owner chips, the count of files shown under a filter, and the note that says when the
 * chosen chip has no files here; or the owner of them all ("18 files · Air Plugin")
 */
export function ReviewOwnerChips({
  of,
  filter,
  onFilterChange,
  count,
}: {
  of: ReviewOwners
  filter: ReviewFilter
  onFilterChange: (filter: ReviewFilter) => void
  count: FilesCount | null
}) {
  const { files, groups, owners } = of
  if (files === undefined) {
    return null
  }
  const chips = reviewFilterChips(files, groups, owners, filter)
  if (chips.length > 1) {
    const paths = ownerPaths(filter, files, groups, owners)
    const empty = paths?.size === 0 ? ownerEmptyText(filter) : null
    const shownCount = count !== null && count.shown !== count.total ? count : null
    const showAll = filter.kind !== "all" && paths !== null && paths.size > 0
    return (
      <>
        <OwnerFilter chips={chips} filter={filter} onFilterChange={onFilterChange} />
        {shownCount !== null && (
          <div className="flex w-full min-w-0 items-center gap-2">
            <SectionLabel title="Files" count={shownCount.shown} of={shownCount.total} className="flex-1" />
            {showAll && (
              <Button variant="link" size="xs" className="h-auto px-0" onClick={() => onFilterChange({ kind: "all" })}>
                Show All Files
              </Button>
            )}
          </div>
        )}
        {empty !== null && (
          <span data-testid="branch-review-owner-empty" className="w-full">
            {empty}
          </span>
        )}
      </>
    )
  }
  const owner = soleOwner(files, owners)
  return (
    <span data-testid="branch-review-files" className="flex min-w-0 flex-1">
      <SectionLabel title="Files" count={count?.shown ?? files.length} of={count === null || count.shown === count.total ? undefined : count.total} className="flex-1">
        {owner ?? undefined}
      </SectionLabel>
    </span>
  )
}

function OwnerFilter({ chips, filter, onFilterChange }: { chips: readonly ReviewFilterChip[]; filter: ReviewFilter; onFilterChange: (filter: ReviewFilter) => void }) {
  // a chip by its filter (a group and an owner may share a label), the one shown by the filter of the review
  const canonical = canonicalOwnerFilter(chips, filter)
  const chosen = ownerChipKey(canonical)
  const { shown, more } = ownerChipRow(chips, canonical)
  return (
    <>
      <FilterChips
        label="Files of"
        value={chosen}
        onValueChange={key => {
          const chip = chips.find(it => ownerChipKey(it.filter) === key)
          if (chip !== undefined) {
            onFilterChange(chip.filter)
          }
        }}
        // All pressed while it is chosen: chosen on purpose, so the groups that come later do not move it
        onChosenPress={() => onFilterChange(filter)}
        data-testid="branch-review-owners"
      >
        {shown.map(chip => (
          <OwnerChip key={ownerChipKey(chip.filter)} chip={chip} />
        ))}
      </FilterChips>
      {more.length > 0 && <MoreOwners chips={more} onChoose={onFilterChange} />}
    </>
  )
}

/**
 * "N More": the chips the header has no room for, each found by its name in a search, with the shield of a group that
 * still needs its approval; the trigger shows the shield too while one of them does
 */
function MoreOwners({ chips, onChoose }: { chips: readonly ReviewFilterChip[]; onChoose: (filter: ReviewFilter) => void }) {
  const [open, setOpen] = useState(false)
  const awaiting = chips.filter(chip => chip.needsApproval && !chip.approved).length
  return (
    <Combobox<ReviewFilterChip>
      items={chips}
      value={null}
      open={open}
      onOpenChange={setOpen}
      onValueChange={chip => {
        if (chip !== null) {
          onChoose(chip.filter)
          setOpen(false)
        }
      }}
      itemToStringLabel={chip => chip.label}
      isItemEqualToValue={(item, value) => ownerChipKey(item.filter) === ownerChipKey(value.filter)}
    >
      <ComboboxPrimitive.Trigger
        className={toggleVariants({ variant: "outline", size: "sm" })}
        aria-label={awaiting > 0 ? `${chips.length} more owners, ${awaiting} of them need approval` : `${chips.length} more owners`}
        data-testid="branch-review-more-owners"
      >
        {awaiting > 0 && <ShieldAlertIcon data-icon="inline-start" className="text-warning" />}
        {chips.length} More
      </ComboboxPrimitive.Trigger>
      <ComboboxContent className="w-80">
        <ComboboxInput showTrigger={false} placeholder="Search owners" aria-label="Owner" />
        <ComboboxEmpty>No owner matches</ComboboxEmpty>
        <ComboboxList data-testid="branch-review-more-owner-list">
          {(chip: ReviewFilterChip) => (
            <ComboboxItem key={ownerChipKey(chip.filter)} value={chip} disabled={chip.count === null} data-testid="branch-review-more-owner">
              {chip.needsApproval && (chip.approved ? <CheckIcon className="text-success" /> : <ShieldAlertIcon className="text-warning" />)}
              <span className="min-w-0 flex-1 truncate">{chip.label}</span>
              {chip.yours && <span className="text-muted-foreground">You</span>}
              {chip.count !== null && <span className="text-muted-foreground tabular-nums">{chip.count}</span>}
            </ComboboxItem>
          )}
        </ComboboxList>
      </ComboboxContent>
    </Combobox>
  )
}

/** A group's chip: a check once it has its approvals, else the shield; "You" where the user can approve for it */
function OwnerChip({ chip }: { chip: ReviewFilterChip }) {
  const button = (
    <FilterChip
      value={ownerChipKey(chip.filter)}
      count={chip.count}
      disabled={chip.count === null}
      data-testid="branch-review-owner"
      data-owner={chip.label}
      data-yours={chip.yours ? "" : undefined}
      data-approved={chip.approved ? "" : undefined}
    >
      {chip.needsApproval &&
        (chip.approved ? <CheckIcon data-icon="inline-start" className="text-success" /> : <ShieldAlertIcon data-icon="inline-start" className="text-warning" />)}
      {chip.label}
      {chip.yours && <span className="text-muted-foreground">· You</span>}
    </FilterChip>
  )
  if (!chip.needsApproval) {
    return button
  }
  const need = chip.approved ? "Approved" : chip.count === null ? "Needs approval; Patronus doesn't tell which files" : "Needs approval for a Safe Push"
  return (
    <Tooltip>
      {/* a span: a disabled button gets no pointer events */}
      <TooltipTrigger render={<span />}>{button}</TooltipTrigger>
      <TooltipContent>{chip.yours ? `${need}. You can approve for this group` : need}</TooltipContent>
    </Tooltip>
  )
}
