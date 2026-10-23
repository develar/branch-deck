import type { RefObject } from "react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { MenuItem } from "@/lib/menu"
import { operationLabel, stopWaitingItem, useIndicatorClaim, useSurfaceOperation } from "@/stores/operations"

/** A `Kbd` inside a primary button: the muted key would vanish on the primary colour */
export const PRIMARY_KBD_CLASS_NAME = "bg-primary-foreground/20 text-primary-foreground"

export interface MenuItemButtonProps {
  /** The action, as the menus of the object list it: label, key, disabled with the reason */
  item: MenuItem
  /** `link` an action inside a text (an alert) */
  variant?: "default" | "ghost" | "outline" | "link"
  /** `xs` a key hint of a footer (the key first), `sm` a decision of a header (the icon, the label, then the key) */
  size?: "xs" | "sm"
  /** The action runs: a spinner in place of the key */
  pending?: boolean
  /**
   * At `sm` in a narrow title bar (its `@container` below 56rem): `key` hides the key hint, `label` the label and the
   * key, the icon left alone and named by the hover
   */
  narrow?: "key" | "label"
  /**
   * The key of this button as the surface that starts the action (`SelectContext.surface`): while the action it started
   * runs, the button shows its spinner and its busy state ("Unstacking tool-events…"), and while its command waits for
   * another holder of the repository lock, whom for and Stop Waiting beside it; without one the row shows it
   */
  surface?: string
  className?: string
  /** The button, as the anchor of the popover its action opens */
  ref?: RefObject<HTMLButtonElement | null>
  "data-testid"?: string
}

/**
 * An action of an object as a button: at `xs` (a footer) the key hint first, at `sm` (a header) the icon, the label and
 * the key last; the key only where it works, the reason it is disabled in a tooltip, so the button and the menus of the
 * object say the same. The action it started shows here while it runs (`surface`).
 */
export function MenuItemButton({ item, variant = "ghost", size = "xs", pending = false, narrow, surface, className, ref, "data-testid": testId }: MenuItemButtonProps) {
  useIndicatorClaim(surface)
  const operation = useSurfaceOperation(surface)
  const busy = operation === undefined ? null : operationLabel(operation)
  const stopWaiting = stopWaitingItem(operation)
  const running = pending || busy !== null
  const key = item.kbds?.at(-1)
  const kbd = key !== undefined && <Kbd className={cn(variant === "default" && PRIMARY_KBD_CLASS_NAME, narrow !== undefined && "hidden @4xl:inline-flex")}>{key.toUpperCase()}</Kbd>
  const Icon = item.icon
  const label = busy === null ? item.label : `${busy}…`
  const onClick = () => item.onSelect({ surface: surface ?? null })
  const common = {
    ref,
    variant,
    disabled: item.disabled === true || running,
    "aria-busy": running || undefined,
    className,
    "data-testid": testId,
    "data-variant": variant,
    onClick,
  }
  const button =
    size === "xs" ? (
      <Button {...common} size="xs">
        {/* a link in a text has no key: its icon leads, as in a menu */}
        {running ? <Spinner data-icon="inline-start" /> : variant === "link" ? Icon !== undefined && <Icon data-icon="inline-start" /> : kbd}
        {label}
      </Button>
    ) : (
      <Button {...common} size="sm">
        {running ? <Spinner data-icon="inline-start" /> : Icon !== undefined && <Icon data-icon="inline-start" />}
        {narrow === "label" ? <span className="sr-only @4xl:not-sr-only">{label}</span> : label}
        {kbd}
      </Button>
    )
  // the busy state is the label: no reason to hover for; an icon left alone is named by its hover
  const description = item.description ?? (narrow === "label" ? item.label : undefined)
  if (stopWaiting !== null) {
    return (
      <span className="inline-flex items-center gap-1">
        {button}
        <MenuItemButton item={stopWaiting} variant="outline" size={size} data-testid="stop-waiting" />
      </span>
    )
  }
  if (description === undefined || busy !== null) {
    return button
  }
  return (
    <Tooltip>
      {/* a span: a disabled button gets no pointer events */}
      <TooltipTrigger render={<span className="inline-flex" />}>{button}</TooltipTrigger>
      <TooltipContent className="flex max-w-xs items-center gap-2">
        {description}
        {item.description === undefined && key !== undefined && <Kbd>{key.toUpperCase()}</Kbd>}
      </TooltipContent>
    </Tooltip>
  )
}
