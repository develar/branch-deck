import { useRef, useState } from "react"
import type { ComponentProps, ReactNode, RefObject } from "react"
import { cn } from "cn"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { HoverDescription } from "@/components/shared/HoverDescription"
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
  /** The control a chip rendered anew in its place stays for the hover (`controlKey` of `Popover`) */
  controlKey?: string
}

/**
 * A text-sized status after the branch status (pull request, Patronus robot, landed commits): a plain button that
 * underlines on hover, opens its details on hover and the item on click, which leaves the details as they are
 * (`HoverDescription`); a chip without an item opens them on a click. A click does not toggle the row. No chip has an
 * outline: the icon and the hue tell the state; the focus ring follows the `rounded-2xl` of buttons and badges.
 */
export function StatusChip({ label, color, data, children, details, detailsTestId, onActivate, onOpenChange, className, controlKey }: StatusChipProps) {
  const Root = onActivate === undefined ? Popover : HoverDescription
  const trigger = useRef<HTMLButtonElement>(null)
  const held = useHeldAnchor(trigger)
  return (
    <Root
      lazy
      controlKey={controlKey}
      onOpenChange={open => {
        if (!open) {
          held.reset()
        }
        onOpenChange?.(open)
      }}
    >
      <PopoverTrigger
        openOnHover
        delay={300}
        render={
          <button
            ref={trigger}
            type="button"
            {...data}
            aria-label={label}
            className={cn(
              "inline-flex h-5 shrink-0 items-center gap-1 rounded-2xl px-1 text-xs tabular-nums outline-none hover:underline focus-visible:bd-focus",
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
      <PopoverContent align="start" anchor={held.anchor} className="w-80 gap-3 p-3" data-testid={detailsTestId} {...held.popupProps}>
        {details}
      </PopoverContent>
    </Root>
  )
}

/** The anchor of a popover that holds still: the place its trigger had */
type HeldAnchor = Exclude<ComponentProps<typeof PopoverContent>["anchor"], undefined>

/**
 * The anchor of the details while the pointer is in them, or a press that began in them lasts: the place the chip had
 * as the pointer came, so the card holds still under it while the chip moves (the Status column of a row moves as its
 * Actions widen: Stop Waiting comes in the slot of a start pressed in the card, and the card followed its chip 97 px
 * away from the pointer). Once the pointer left the card and no press lasts, the card follows its chip again: the
 * trigger is its anchor once more, which places it anew (docs/contributing/frontend.md, "What comes late beside a hovered
 * control"). The anchor changes, not `disableAnchorTracking` of the positioner: Base UI 1.9 arms the tracking of an
 * anchor again only for a new anchor (floating-ui keeps `whileElementsMounted` in a ref), never for a change of that prop
 */
function useHeldAnchor(trigger: RefObject<HTMLElement | null>): {
  anchor: HeldAnchor | undefined
  popupProps: Pick<ComponentProps<"div">, "onPointerEnter" | "onPointerLeave" | "onPointerDown">
  reset: () => void
} {
  const [anchor, setAnchor] = useState<HeldAnchor | undefined>(undefined)
  const inside = useRef(false)
  const pressing = useRef(false)
  const release = () => {
    if (!inside.current && !pressing.current) {
      setAnchor(undefined)
    }
  }
  return {
    anchor,
    popupProps: {
      onPointerEnter: () => {
        inside.current = true
        const element = trigger.current
        if (element !== null) {
          const rect = element.getBoundingClientRect()
          setAnchor(current => current ?? { getBoundingClientRect: () => rect })
        }
      },
      onPointerLeave: () => {
        inside.current = false
        release()
      },
      onPointerDown: () => {
        pressing.current = true
        // the press ends with the pointer up, or with its cancel (a drag of a text or a link of the card): a cancel
        // left the card held where it stood, away from its chip
        const ended = new AbortController()
        const end = () => {
          ended.abort()
          pressing.current = false
          release()
        }
        window.addEventListener("pointerup", end, { signal: ended.signal })
        window.addEventListener("pointercancel", end, { signal: ended.signal })
      },
    },
    reset: () => {
      inside.current = false
      pressing.current = false
      setAnchor(undefined)
    },
  }
}

/** The `size-1.5` dot of a chip, as the status dot of the row */
export function StatusChipDot({ color, testId }: { color: "error" | "warning" | "success"; testId: string }) {
  return <span aria-hidden data-testid={testId} className={cn("size-1.5 shrink-0 rounded-full", dotClasses[color])} />
}

const dotClasses = { error: "bg-destructive", warning: "bg-warning", success: "bg-success" } as const
