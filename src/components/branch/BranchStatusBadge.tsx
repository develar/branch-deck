import { Badge } from "@/components/ui/badge"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { useStatusBadge } from "@/hooks/useStatusBadge"
import { subtleBadgeVariant } from "@/lib/ui-types"
import type { Branch } from "@/stores/branch-sync"

/** Sync/remote status of an active branch; the full description is shown on hover. */
export function BranchStatusBadge({ branch }: { branch: Branch }) {
  const status = useStatusBadge(branch)
  return (
    <div className="w-40">
      <Popover>
        <PopoverTrigger
          openOnHover
          delay={300}
          nativeButton={false}
          // E2E locates the status text by the `lowercase` class
          render={<Badge variant={subtleBadgeVariant(status.color)} className="block max-w-40 truncate lowercase" />}
        >
          {status.text}
        </PopoverTrigger>
        <PopoverContent className="w-auto max-w-sm px-3 py-2 text-sm leading-snug break-words whitespace-pre-line">
          {status.tooltip}
        </PopoverContent>
      </Popover>
    </div>
  )
}
