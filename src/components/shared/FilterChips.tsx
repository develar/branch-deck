import type { ComponentProps, ReactNode } from "react"
import { cn } from "cn"
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
      {children}
    </ToggleGroup>
  )
}

/** A chip of `FilterChips`: its name, then its count in tabular figures */
export function FilterChip({ count, children, ...props }: ComponentProps<typeof ToggleGroupItem> & { count?: number | null }) {
  return (
    <ToggleGroupItem {...props}>
      {children}
      {count != null && <span className="tabular-nums opacity-70">{count}</span>}
    </ToggleGroupItem>
  )
}
