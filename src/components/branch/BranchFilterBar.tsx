import { useEffect, useRef } from "react"
import { SearchIcon, XIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput } from "@/components/ui/input-group"
import { Kbd } from "@/components/ui/kbd"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { ChoiceChip } from "@/components/shared/ChoiceChip"
import { useChipCounts } from "@/hooks/useBranchFilter"
import { CHIP_LABELS, CHIPS, FILTER_BAR_MIN_BRANCHES, formatNoMatch, isFilterActive } from "@/lib/branch-filter"
import { KEY_OWNERS } from "@/lib/preview-keys"
import { clearBranchFilter, setFilterText, toggleFilterChip, useBranchFilter } from "@/stores/branch-filter"

// `/` keeps its meaning in these: typing, a menu or a dialog

export interface BranchFilterBarProps {
  /** ↓ in the search: the focus moves to the first branch row */
  onFocusRows: () => void
  /** Escape in the empty search: the focus moves to the table, whose keys then work */
  onLeave: () => void
}

/**
 * The filter bar at the top of the branch table: a search ("/" focuses it from anywhere in the window, Escape clears
 * it, then leaves it) and the state chips with their counts of all branches (All, To Push, Running, Attention; one at a
 * time, the chosen one again goes back to All), a chip shown while it counts a branch or is chosen. Hidden in a table
 * of fewer than `FILTER_BAR_MIN_BRANCHES` branches, unless a filter is active.
 */
export function BranchFilterBar({ onFocusRows, onLeave }: BranchFilterBarProps) {
  const filter = useBranchFilter()
  const counts = useChipCounts()
  const isShown = counts.all >= FILTER_BAR_MIN_BRANCHES || isFilterActive(filter)
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (!isShown) {
      return
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "/" || event.defaultPrevented || event.metaKey || event.ctrlKey || event.altKey) {
        return
      }
      if (event.target instanceof Element && event.target.closest(KEY_OWNERS) !== null) {
        return
      }
      event.preventDefault()
      inputRef.current?.focus()
      inputRef.current?.select()
    }
    document.addEventListener("keydown", onKeyDown)
    return () => document.removeEventListener("keydown", onKeyDown)
  }, [isShown])

  if (!isShown) {
    return null
  }
  return (
    <div data-testid="branch-filter" className="flex flex-wrap items-center gap-x-3 gap-y-2 border-b border-border px-4 py-2">
      <InputGroup className="h-7 w-60">
        <InputGroupAddon>
          <SearchIcon />
        </InputGroupAddon>
        <InputGroupInput
          ref={inputRef}
          type="text"
          value={filter.text}
          placeholder="Filter branches"
          aria-label="Filter branches"
          autoComplete="off"
          spellCheck={false}
          className="h-7"
          onChange={event => setFilterText(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Escape") {
              event.preventDefault()
              if (filter.text === "") {
                onLeave()
              }
              else {
                setFilterText("")
              }
            }
            else if (event.key === "ArrowDown") {
              event.preventDefault()
              onFocusRows()
            }
          }}
        />
        <InputGroupAddon align="inline-end">
          {filter.text === ""
            ? <Kbd>/</Kbd>
            : (
                <InputGroupButton size="icon-xs" aria-label="Clear the filter text" onClick={() => setFilterText("")}>
                  <XIcon />
                </InputGroupButton>
              )}
        </InputGroupAddon>
      </InputGroup>
      <div role="group" aria-label="Branch state" className="flex items-center gap-1">
        {CHIPS.filter(chip => chip === "all" || counts[chip] > 0 || filter.chip === chip).map((chip) => {
          const choice = (
            <ChoiceChip key={chip} chosen={filter.chip === chip} data-chip={chip} onClick={() => toggleFilterChip(chip)}>
              {CHIP_LABELS[chip]}
              <span className="tabular-nums opacity-70">{counts[chip]}</span>
            </ChoiceChip>
          )
          // F2 steps through these rows from anywhere in the window: the chip is where the key is told
          return chip === "attention" && counts.attention > 0
            ? (
                <Tooltip key={chip}>
                  <TooltipTrigger render={choice} />
                  <TooltipContent>
                    Go to the next one
                    <Kbd>F2</Kbd>
                    , the previous one
                    <Kbd>⇧F2</Kbd>
                  </TooltipContent>
                </Tooltip>
              )
            : choice
        })}
      </div>
    </div>
  )
}

/** The row of a table the filter left empty: what found nothing, and Clear Filter */
export function BranchFilterEmptyRow({ columnCount }: { columnCount: number }) {
  const filter = useBranchFilter()
  return (
    <tr data-testid="branch-filter-empty">
      <td colSpan={columnCount} className="px-6 py-4">
        <div className="flex items-center gap-3 text-sm text-muted-foreground">
          <span className="min-w-0 truncate">{formatNoMatch(filter)}</span>
          <Button variant="link" size="xs" className="h-auto px-0" onClick={clearBranchFilter}>Clear Filter</Button>
        </div>
      </td>
    </tr>
  )
}
