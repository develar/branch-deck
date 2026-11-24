import { cn } from "cn"
import { Badge } from "@/components/ui/badge"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { SHOWN_BADGE } from "@/presenters/shown"

/**
 * "Shown" of a pull request that is a review after landing (`ReviewRequest.shown`, Rust's by its base): its change is
 * in master already, a review of it gates no Safe Push; its hover says so. Of the row of Your Turn and Reviews, and of
 * the review window
 */
export function ShownBadge({ className }: { className?: string }) {
  return (
    <Tooltip>
      <TooltipTrigger render={<Badge variant="info" tabIndex={0} className={cn("shrink-0", className)} data-testid="shown-badge" />}>{SHOWN_BADGE.label}</TooltipTrigger>
      <TooltipContent className="max-w-80">{SHOWN_BADGE.hint}</TooltipContent>
    </Tooltip>
  )
}
