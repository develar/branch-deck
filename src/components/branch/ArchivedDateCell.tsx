import { Spinner } from "@/components/ui/spinner"
import { formatTimestamp } from "@/lib/time"
import type { ArchivedBranch } from "@/stores/archived-branches"

/** Integration date of an archived branch. */
export function ArchivedDateCell({ branch }: { branch: ArchivedBranch }) {
  return (
    <div className="flex items-center gap-2">
      {branch.type === "placeholder"
        ? <Spinner className="size-3 text-muted-foreground" />
        : branch.integratedAt > 0 && <span className="text-sm text-muted-foreground">{formatTimestamp(branch.integratedAt)}</span>}
    </div>
  )
}
