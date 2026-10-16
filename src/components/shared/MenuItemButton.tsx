import type { RefObject } from "react"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { MenuItem } from "@/lib/menu"

/** A `Kbd` inside a primary button: the muted key would vanish on the primary colour */
export const PRIMARY_KBD_CLASS_NAME = "bg-primary-foreground/20 text-primary-foreground"

export interface MenuItemButtonProps {
  /** The action, as the menus of the object list it: label, key, disabled with the reason */
  "item": MenuItem
  "variant"?: "default" | "ghost" | "outline"
  /** `xs` a key hint of a footer (the key first), `sm` a decision of a header (the icon, the label, then the key) */
  "size"?: "xs" | "sm"
  /** The action runs: a spinner in place of the key */
  "pending"?: boolean
  /** The button, as the anchor of the popover its action opens */
  "ref"?: RefObject<HTMLButtonElement | null>
  "data-testid"?: string
}

/**
 * An action of an object as a button: at `xs` (a footer) the key hint first, at `sm` (a header) the icon, the label and
 * the key last; the key only where it works, the reason it is disabled in a tooltip, so the button and the menus of the
 * object say the same.
 */
export function MenuItemButton({ item, variant = "ghost", size = "xs", pending = false, ref, "data-testid": testId }: MenuItemButtonProps) {
  const key = item.kbds?.at(-1)
  const kbd = key !== undefined && <Kbd className={variant === "default" ? PRIMARY_KBD_CLASS_NAME : undefined}>{key.toUpperCase()}</Kbd>
  const Icon = item.icon
  const button = size === "xs"
    ? (
        <Button ref={ref} variant={variant} size="xs" disabled={item.disabled === true || pending} data-testid={testId} data-variant={variant} onClick={item.onSelect}>
          {pending ? <Spinner data-icon="inline-start" /> : kbd}
          {item.label}
        </Button>
      )
    : (
        <Button ref={ref} variant={variant} size="sm" disabled={item.disabled === true || pending} data-testid={testId} data-variant={variant} onClick={item.onSelect}>
          {pending ? <Spinner data-icon="inline-start" /> : Icon !== undefined && <Icon data-icon="inline-start" />}
          {item.label}
          {kbd}
        </Button>
      )
  if (item.description === undefined) {
    return button
  }
  return (
    <Tooltip>
      {/* a span: a disabled button gets no pointer events */}
      <TooltipTrigger render={<span className="inline-flex" />}>{button}</TooltipTrigger>
      <TooltipContent className="max-w-xs">{item.description}</TooltipContent>
    </Tooltip>
  )
}
