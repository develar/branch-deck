import { cn } from "cn"
import type { LucideIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"

export interface ActionButtonProps {
  label: string
  icon?: LucideIcon
  /** The single key that runs the action where the button is (`S`) */
  keyHint?: string
  /** The action can't run: the button is dimmed, stays focusable for its hover, and does nothing */
  disabled?: boolean
  /** The hover: why a disabled action can't run, else what it does; the label by default */
  description?: string
  onClick: () => void
  "data-testid"?: string
}

/**
 * An action of a title bar or a toolbar, the one renderer of an action as a button: the icon, the label and the key; a
 * disabled action says why on hover. In a narrow title bar (its `@container`) the label of an action with an icon
 * gives way to the icon alone, and the hover names it.
 */
export function ActionButton({ label, icon: Icon, keyHint, disabled = false, description, onClick, "data-testid": testId }: ActionButtonProps) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <Button
            variant="ghost"
            size="sm"
            disabled={disabled}
            // the hover says why; Base UI keeps the button focusable, so the dimming is ours
            focusableWhenDisabled
            className={cn(disabled && "opacity-50")}
            data-testid={testId}
            onClick={onClick}
          />
        }
      >
        {Icon !== undefined && <Icon data-icon="inline-start" />}
        <span className={cn(Icon !== undefined && "sr-only @3xl:not-sr-only")}>{label}</span>
        {keyHint !== undefined && <Kbd className={cn(Icon !== undefined && "hidden @3xl:inline-flex")}>{keyHint.toUpperCase()}</Kbd>}
      </TooltipTrigger>
      <TooltipContent className="flex items-center gap-2">
        {description ?? label}
        {keyHint !== undefined && !disabled && <Kbd>{keyHint.toUpperCase()}</Kbd>}
      </TooltipContent>
    </Tooltip>
  )
}
