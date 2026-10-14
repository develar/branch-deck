import type { ComponentProps, ReactNode } from "react"
import { CircleCheckIcon, CircleXIcon, InfoIcon, TriangleAlertIcon } from "lucide-react"
import type { LucideIcon } from "lucide-react"
import { cn } from "cn"

/**
 * The colour rule of `docs/design/ui-theme.md`: info = already in the baseline, success = approved, passed or clean,
 * warning = attention without failure, destructive = failure or conflict.
 */
export type CalloutTone = "info" | "success" | "warning" | "destructive"

// full class names, so Tailwind finds them
const toneClasses: Record<CalloutTone, { box: string, accent: string, icon: LucideIcon }> = {
  info: { box: "border-info/20 bg-info/10", accent: "text-info", icon: InfoIcon },
  success: { box: "border-success/20 bg-success/10", accent: "text-success", icon: CircleCheckIcon },
  warning: { box: "border-warning/20 bg-warning/10", accent: "text-warning", icon: TriangleAlertIcon },
  destructive: { box: "border-destructive/20 bg-destructive/10", accent: "text-destructive", icon: CircleXIcon },
}

export interface CalloutProps extends Omit<ComponentProps<"div">, "title"> {
  tone: CalloutTone
  /** The lead icon; the tone's icon by default (`CircleXIcon` for an error; a conflict passes `TriangleAlertIcon`) */
  icon?: LucideIcon
  /** One line in the tone's colour; a callout of one sentence passes only the title */
  title?: ReactNode
  /** Details below the title, muted */
  children?: ReactNode
}

/**
 * A note inside a form, a popover or a panel: a tinted box with a `size-4` lead icon, a title in the tone's colour and
 * muted details. The state of a whole block (a branch that failed to sync, the repository path error) is an `Alert`.
 */
export function Callout({ tone, icon, title, children, className, ...props }: CalloutProps) {
  const classes = toneClasses[tone]
  const Icon = icon ?? classes.icon
  return (
    <div data-slot="callout" data-tone={tone} className={cn("flex items-start gap-2 rounded-lg border p-3 text-sm", classes.box, className)} {...props}>
      {/* one line high: the icon centres on the first line at any text size */}
      <span className="flex h-lh shrink-0 items-center">
        <Icon aria-hidden className={cn("size-4", classes.accent)} />
      </span>
      <div className="flex min-w-0 flex-1 flex-col gap-1.5 text-muted-foreground">
        {title !== undefined && <p className={cn("font-medium", classes.accent)}>{title}</p>}
        {children}
      </div>
    </div>
  )
}
