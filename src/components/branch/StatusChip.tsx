import { useRef, useState } from "react"
import type { ComponentProps, ReactNode, RefObject } from "react"
import { cn } from "cn"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { HoverDescription } from "@/components/shared/HoverDescription"
import { observeSize } from "@/lib/observe-size"
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
      <PopoverContent {...held.placement} className="w-80 gap-3 p-3" data-testid={detailsTestId} data-hang={held.hang} {...held.popupProps}>
        {details}
      </PopoverContent>
    </Root>
  )
}

/** The anchor of a popover that holds still: the place its trigger had */
type HeldAnchor = Exclude<ComponentProps<typeof PopoverContent>["anchor"], undefined>

type PopupSide = "top" | "bottom" | "left" | "right"

/** The gap between a chip and its details (`sideOffset` of their positioner) */
const SIDE_OFFSET = 4

/**
 * No correction of an overflow while the details hold still: they stay where they stood, also when what comes in them
 * takes them past the edge of the window until the pointer leaves (accepted: docs/contributing/frontend.md)
 */
const PINNED_COLLISIONS = { side: "none", align: "none", fallbackAxisSide: "none" } as const

/**
 * Where the details hold still while the pointer is in them: a virtual anchor, side and alignment whose placement,
 * without collision avoidance, is the place the details had, hung by the edge they hung by (`hang`)
 */
interface Pin {
  anchor: HeldAnchor
  side: PopupSide
  align: "start" | "end"
  hang: "top" | "bottom"
}

/**
 * The edge the details of `side` and `align` hang by, of their box `popup` beside their chip `chip`: the chip's edge of
 * a card under or over its chip; of a card beside it the top edge, the bottom one when aligned by their ends or shifted
 * up against the bottom of the viewport (`shift` of the positioner), so what they show more or less goes the other way
 */
function hangOf(side: PopupSide, align: string | undefined, popup: DOMRect, chip: DOMRect): "top" | "bottom" {
  if (side === "top" || side === "bottom") {
    return side === "top" ? "bottom" : "top"
  }
  if (align === "end") {
    return popup.bottom > chip.bottom + 1 ? "top" : "bottom"
  }
  return popup.top < chip.top - 1 ? "bottom" : "top"
}

/** The side of the popup as its positioner placed it; null before it was placed */
function sideOf(popup: HTMLElement): PopupSide | null {
  const side = popup.dataset.side
  return side === "top" || side === "bottom" || side === "left" || side === "right" ? side : null
}

/** The pin of the details `popup` where they stand now beside their chip `chip` (`Pin`) */
function pinOf(popup: HTMLElement, chip: DOMRect): Pin | undefined {
  const side = sideOf(popup)
  // the positioner has the box of the popup without the scale of its open
  const box = popup.parentElement?.getBoundingClientRect()
  if (side === null || box === undefined) {
    return undefined
  }
  const hang = hangOf(side, popup.dataset.align, box, chip)
  const vertical = side === "top" || side === "bottom"
  const x = vertical ? box.left : side === "right" ? box.left - SIDE_OFFSET - chip.width : box.right + SIDE_OFFSET
  const y = !vertical ? (hang === "top" ? box.top : box.bottom - chip.height) : side === "bottom" ? box.top - SIDE_OFFSET - chip.height : box.bottom + SIDE_OFFSET
  const rect = new DOMRect(x, y, chip.width, chip.height)
  return { anchor: { getBoundingClientRect: () => rect }, side, align: !vertical && hang === "bottom" ? "end" : "start", hang }
}

/**
 * The details hold still while the pointer is in them, or a press that began in them lasts: placed where they stood as
 * the pointer came, hung by the edge they hung by (`Pin`), without collision avoidance, so the card holds still under
 * the pointer while its chip moves (the Status column of a row moves as its Actions widen: Stop Waiting comes in the
 * slot of a start pressed in the card, and the card followed its chip 97 px away from the pointer), and what comes or
 * goes in it changes its size on its other edge only: no control between moves. `hang` (`data-hang` of the popup) is
 * the edge the details hang by, which their content keeps its actions next to (docs/contributing/frontend.md, "What
 * comes late beside a hovered control"). Once the pointer left the card and no press lasts, the card is placed by its
 * chip again. The anchor changes, not `disableAnchorTracking` of the positioner: Base UI 1.9 arms the tracking of an
 * anchor again only for a new anchor (floating-ui keeps `whileElementsMounted` in a ref), never for a change of that prop
 */
function useHeldAnchor(trigger: RefObject<HTMLElement | null>): {
  placement: Pick<ComponentProps<typeof PopoverContent>, "anchor" | "side" | "align" | "sideOffset" | "collisionAvoidance">
  hang: "top" | "bottom" | undefined
  popupProps: Pick<ComponentProps<"div">, "onPointerEnter" | "onPointerLeave" | "onPointerDown" | "ref">
  reset: () => void
} {
  const [pin, setPin] = useState<Pin | undefined>(undefined)
  // the edge the details hang by while no pin holds them, as their positioner placed them last; unknown until they are
  // placed (`data-side` then says it of a card over or under its chip)
  const [placedHang, setPlacedHang] = useState<"top" | "bottom" | undefined>(undefined)
  const inside = useRef(false)
  const pressing = useRef(false)
  const pinned = useRef(false)
  const release = () => {
    if (!inside.current && !pressing.current) {
      pinned.current = false
      setPin(undefined)
    }
  }
  const measure = (popup: HTMLElement) => {
    const side = sideOf(popup)
    const chip = trigger.current?.getBoundingClientRect()
    const box = popup.parentElement?.getBoundingClientRect()
    if (!pinned.current && side !== null && chip !== undefined && box !== undefined) {
      setPlacedHang(hangOf(side, popup.dataset.align, box, chip))
    }
  }
  return {
    placement: {
      anchor: pin?.anchor,
      side: pin?.side ?? "bottom",
      align: pin?.align ?? "start",
      sideOffset: SIDE_OFFSET,
      collisionAvoidance: pin === undefined ? undefined : PINNED_COLLISIONS,
    },
    hang: pin?.hang ?? placedHang,
    popupProps: {
      // the edge they hang by, measured once placed and whenever their positioner places them anew: as their size
      // changes, as they move without a change of size (a scroll of the table, rows above, a resize of the window: the
      // style of the positioner) and as their side or alignment changes
      ref: (popup: HTMLDivElement | null) => {
        if (popup === null) {
          return
        }
        const now = () => measure(popup)
        const frame = requestAnimationFrame(now)
        const sizes = observeSize(now)
        sizes.observe(popup)
        const moves = new MutationObserver(now)
        moves.observe(popup, { attributes: true, attributeFilter: ["data-side", "data-align"] })
        if (popup.parentElement !== null) {
          moves.observe(popup.parentElement, { attributes: true, attributeFilter: ["style"] })
        }
        return () => {
          sizes.disconnect()
          moves.disconnect()
          cancelAnimationFrame(frame)
        }
      },
      onPointerEnter: event => {
        inside.current = true
        const chip = trigger.current?.getBoundingClientRect()
        if (!pinned.current && chip !== undefined) {
          const next = pinOf(event.currentTarget, chip)
          if (next !== undefined) {
            pinned.current = true
            setPin(next)
          }
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
        // a pointer up outside the window reaches no listener of it: the press ends as the window loses the focus
        window.addEventListener("blur", end, { signal: ended.signal })
      },
    },
    reset: () => {
      inside.current = false
      pressing.current = false
      pinned.current = false
      setPin(undefined)
      // the next open measures its own edge
      setPlacedHang(undefined)
    },
  }
}

/** The `size-1.5` dot of a chip, as the status dot of the row */
export function StatusChipDot({ color, testId }: { color: "error" | "warning" | "success"; testId: string }) {
  return <span aria-hidden data-testid={testId} className={cn("size-1.5 shrink-0 rounded-full", dotClasses[color])} />
}

const dotClasses = { error: "bg-destructive", warning: "bg-warning", success: "bg-success" } as const
