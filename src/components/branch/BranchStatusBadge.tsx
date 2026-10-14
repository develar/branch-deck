import { cn } from "cn"
import { Badge } from "@/components/ui/badge"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { useStatusBadge } from "@/hooks/useStatusBadge"
import { statusDotClassName, subtleBadgeVariant } from "@/lib/ui-types"
import type { Branch } from "@/stores/branch-sync"

/**
 * Sync/remote status of an active branch; the full description is shown on hover.
 * Only an error or a conflict is a badge; a routine state ("not pushed", "1 (3)↑ · 2↓") is muted text with a colour dot.
 */
export function BranchStatusBadge({ branch }: { branch: Branch }) {
  const status = useStatusBadge(branch)
  const exceptional = status.badge === true
  return (
    <div>
      <Popover>
        <PopoverTrigger
          openOnHover
          delay={300}
          nativeButton={false}
          // the texts are lower case already ("blocked by IJPL-123" keeps its issue key); E2E reads the `branch-status` test id
          render={exceptional
            ? <Badge variant={subtleBadgeVariant(status.color)} className="block max-w-40 truncate" data-testid="branch-status" />
            : <span className="flex max-w-40 items-center gap-2 text-sm text-muted-foreground" />}
        >
          {!exceptional && <span aria-hidden className={cn("size-1.5 shrink-0 rounded-full", statusDotClassName(status.color))} />}
          {exceptional ? status.text : <span className="truncate" data-testid="branch-status">{status.text}</span>}
        </PopoverTrigger>
        <PopoverContent className="w-auto max-w-sm px-3 py-2 text-sm leading-snug break-words whitespace-pre-line">
          {status.tooltip}
        </PopoverContent>
      </Popover>
    </div>
  )
}
