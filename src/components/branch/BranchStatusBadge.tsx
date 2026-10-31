import { cn } from "cn"
import { Badge } from "@/components/ui/badge"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { StatusTexts } from "@/lib/remote-status"
import { statusDotClassName, subtleBadgeVariant } from "@/lib/ui-types"

/**
 * The status text of an active branch (`presentRowCell`); the full description is a tooltip, as in the branch preview.
 * Only an error or a conflict is a badge; a routine state ("not pushed", "1 (3)↑ · 2↓") is muted text with a dot in
 * the push colour. A state that needs no action ("up to date") shows nothing: most rows are in it, and the states that
 * need the user stand out. The node of the row says where the branch is on its way into master, not its push.
 */
export function BranchStatusBadge({ status }: { status: StatusTexts }) {
  const exceptional = status.badge === true
  if (status.quiet === true) {
    return null
  }
  return (
    // the item of the state track truncates it at the cap of the track
    <div className="min-w-0">
      <Tooltip lazy>
        <TooltipTrigger
          delay={300}
          // the texts are lower case already ("blocked by IJPL-123" keeps its issue key); E2E reads the `branch-status` test id
          render={
            exceptional ? (
              <Badge variant={subtleBadgeVariant(status.color)} className="block max-w-full truncate tabular-nums" data-testid="branch-status" />
            ) : (
              <span className="flex max-w-full items-center gap-2 text-sm text-muted-foreground tabular-nums" />
            )
          }
        >
          {!exceptional && (
            <span aria-hidden className={cn("size-1.5 shrink-0 rounded-full", statusDotClassName(status.color))} data-testid="branch-status-dot" data-color={status.color} />
          )}
          {exceptional ? (
            status.text
          ) : (
            <span className="truncate" data-testid="branch-status">
              {status.text}
            </span>
          )}
        </TooltipTrigger>
        <TooltipContent className="max-w-sm break-words whitespace-pre-line">{status.tooltip}</TooltipContent>
      </Tooltip>
    </div>
  )
}
