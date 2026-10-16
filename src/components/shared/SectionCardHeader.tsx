import type { ReactNode } from "react"
import { Badge } from "@/components/ui/badge"
import { subtleBadgeVariant } from "@/lib/ui-types"
import type { UIColor } from "@/lib/ui-types"

export interface SectionCardHeaderProps {
  title: string
  count?: number
  itemSingular?: string
  itemPlural?: string
  badgeColor?: UIColor
  /** After the count, e.g. what of the items needs a look ("3 not merged") */
  note?: ReactNode
  /** A `HelpPopover`, right after the title, its count and note */
  help?: ReactNode
  actions?: ReactNode
}

/**
 * Card header: title with an optional item count badge (neutral: a count is not a status), the help after them, actions
 * on the right.
 */
export function SectionCardHeader({
  title,
  count,
  itemSingular = "item",
  itemPlural = "items",
  badgeColor = "neutral",
  note,
  help,
  actions,
}: SectionCardHeaderProps) {
  return (
    <div className="flex items-center justify-between">
      <div className="flex items-center gap-3">
        <h2 className="text-sm font-semibold text-foreground">{title}</h2>
        {count !== undefined && (
          <Badge variant={subtleBadgeVariant(badgeColor)}>
            {count}
            {" "}
            {count === 1 ? itemSingular : itemPlural}
          </Badge>
        )}
        {note}
        {help}
      </div>
      {actions && <div className="flex items-center gap-3">{actions}</div>}
    </div>
  )
}
