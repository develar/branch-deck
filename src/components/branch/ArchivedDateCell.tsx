import { formatTimestamp } from "@/lib/time"
import type { ArchivedBranch } from "@/stores/archived-branches"

/** Integration date of an archived branch; empty for a placeholder, whose spinner is in the Status column */
export function ArchivedDateCell({ branch }: { branch: ArchivedBranch }) {
  return (
    <div className="flex items-center gap-2">
      {branch.type !== "placeholder" && branch.integratedAt > 0 && <span className="text-sm text-muted-foreground">{formatTimestamp(branch.integratedAt)}</span>}
    </div>
  )
}
