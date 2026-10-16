import type { ReactNode } from "react"
import { cn } from "cn"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import type { StatusChipColor } from "@/lib/status-chip"
import { statusChipTextColors } from "@/lib/status-chip"

export interface StatusChipProps {
  /** The accessible name, e.g. "Pull request #760: Approved" */
  label: string
  color: StatusChipColor
  /** `data-*` attributes of the trigger, read by E2E tests */
  data: Readonly<Record<`data-${string}`, string | boolean | undefined>>
  /** The icon, the text and the dots */
  children: ReactNode
  /** The hover popover */
  details: ReactNode
  detailsTestId: string
  /** A click: opens the item in the browser; without it a click opens the details */
  onActivate?: () => void
  onOpenChange?: (open: boolean) => void
  /** Overrides of the trigger's look: the Safe Push slot of the row is a button among the buttons of the Actions cell */
  className?: string
}

/**
 * A text-sized status after the branch status (pull request, Patronus robot, landed commits): a plain button that
 * underlines on hover, opens its details on hover and the item on click. A click does not toggle the row. No chip has an
 * outline: the icon and the hue tell the state; the focus ring follows the `rounded-2xl` of buttons and badges.
 */
export function StatusChip({ label, color, data, children, details, detailsTestId, onActivate, onOpenChange, className }: StatusChipProps) {
  return (
    <Popover onOpenChange={onOpenChange}>
      <PopoverTrigger
        openOnHover
        delay={300}
        render={
          <button
            type="button"
            {...data}
            aria-label={label}
            className={cn(
              "inline-flex h-5 shrink-0 items-center gap-1 rounded-2xl px-1 text-xs tabular-nums outline-none hover:underline focus-visible:ring-3 focus-visible:ring-ring/30",
              statusChipTextColors[color],
              className,
            )}
            onClick={event => {
              // the row toggles on click
              event.stopPropagation()
              onActivate?.()
            }}
          />
        }
      >
        {children}
      </PopoverTrigger>
      <PopoverContent align="start" className="w-80 gap-3 p-3" data-testid={detailsTestId}>
        {details}
      </PopoverContent>
    </Popover>
  )
}

/** The `size-1.5` dot of a chip, as the status dot of the row */
export function StatusChipDot({ color, testId }: { color: "error" | "warning" | "success"; testId: string }) {
  return <span aria-hidden data-testid={testId} className={cn("size-1.5 shrink-0 rounded-full", dotClasses[color])} />
}

const dotClasses = { error: "bg-destructive", warning: "bg-warning", success: "bg-success" } as const
