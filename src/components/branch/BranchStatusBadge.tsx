import { cn } from "cn"
import { Badge } from "@/components/ui/badge"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useStatusBadge } from "@/hooks/useStatusBadge"
import { statusDotClassName, subtleBadgeVariant } from "@/lib/ui-types"
import type { Branch } from "@/stores/branch-sync"

/**
 * Sync/remote status of an active branch; the full description is a tooltip, as in the branch preview.
 * Only an error or a conflict is a badge; a routine state ("not pushed", "1 (3)↑ · 2↓") is muted text with a dot in
 * the push colour. The node of the row says where the branch is on its way into master, not its push.
 */
export function BranchStatusBadge({ branch }: { branch: Branch }) {
  const status = useStatusBadge(branch)
  const exceptional = status.badge === true
  return (
    <div>
      <Tooltip>
        <TooltipTrigger
          delay={300}
          // the texts are lower case already ("blocked by IJPL-123" keeps its issue key); E2E reads the `branch-status` test id
          render={
            exceptional ? (
              <Badge variant={subtleBadgeVariant(status.color)} className="block max-w-40 truncate tabular-nums" data-testid="branch-status" />
            ) : (
              <span className="flex max-w-40 items-center gap-2 text-sm text-muted-foreground tabular-nums" />
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
