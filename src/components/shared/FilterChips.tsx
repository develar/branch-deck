import { createContext, use } from "react"
import type { ComponentProps, ReactNode } from "react"
import { cn } from "cn"
import { Toggle } from "@/components/ui/toggle"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"

export interface FilterChipsProps {
  /** The accessible name of the group ("Branch state", "Kind", "Files of") */
  label: string
  /** The chosen chip */
  value: string
  /** A press of another chip chooses it; a press of the chosen one goes to `fallback` (All), or keeps it without one */
  onValueChange: (value: string) => void
  fallback?: string
  className?: string
  "data-testid"?: string
  children: ReactNode
}

/** The chosen chip of the `FilterChips` around a chip */
const ChosenChip = createContext<string | null>(null)

/**
 * The chips of a filter of a list, one chosen at a time (the state chips of the branch table, the kinds of Activity, the
 * owners of the files of a review): the stock `ToggleGroup`, outlined, the chosen chip in its pressed state; the chips
 * wrap where the row is narrow
 */
export function FilterChips({ label, value, onValueChange, fallback, className, "data-testid": testId, children }: FilterChipsProps) {
  return (
    <ToggleGroup
      aria-label={label}
      variant="outline"
      size="sm"
      spacing={1}
      value={[value]}
      onValueChange={next => {
        const chosen = next.find(it => it !== value) ?? fallback
        if (chosen !== undefined && chosen !== value) {
          onValueChange(chosen)
        }
      }}
      className={cn("flex-wrap", className)}
      data-testid={testId}
    >
      <ChosenChip value={value}>{children}</ChosenChip>
    </ToggleGroup>
  )
}

/**
 * A chip of `FilterChips`: its name, then its count in tabular figures. The chosen one is the active item of the group
 * (`data-composite-item-active`, as a checked radio is): the tab stop of the group starts on it, and the group finds it
 * without asking each chip whether it is visible, a style recalc of the whole page while the rows of a first sync come
 */
export function FilterChip({ count, children, ...props }: ComponentProps<typeof ToggleGroupItem> & { count?: number | null }) {
  const chosen = use(ChosenChip)
  return (
    <ToggleGroupItem {...props} data-composite-item-active={chosen !== null && props.value === chosen ? "" : undefined}>
      {children}
      <ChipCount count={count} />
    </ToggleGroupItem>
  )
}

/**
 * A chip of its own beside `FilterChips`, on or off, in their look: a part of the list that shows only while it is on,
 * whatever chip is chosen (the operations of the agents in Activity); its count is what a press adds or takes away
 */
export function FilterToggle({ count, children, ...props }: ComponentProps<typeof Toggle> & { count?: number | null }) {
  return (
    <Toggle variant="outline" size="sm" {...props}>
      {children}
      <ChipCount count={count} />
    </Toggle>
  )
}

/** The count of a chip, in tabular figures */
function ChipCount({ count }: { count: number | null | undefined }) {
  return count != null && <span className="tabular-nums opacity-70">{count}</span>
}
