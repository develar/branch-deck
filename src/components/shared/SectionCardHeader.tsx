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
  actions?: ReactNode
}

/** Card header: title with an optional item count badge (neutral: a count is not a status), actions on the right. */
export function SectionCardHeader({
  title,
  count,
  itemSingular = "item",
  itemPlural = "items",
  badgeColor = "neutral",
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
      </div>
      {actions && <div className="flex items-center gap-3">{actions}</div>}
    </div>
  )
}
