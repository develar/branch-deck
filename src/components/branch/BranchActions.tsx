import { UploadIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { BranchPushButton } from "@/components/branch/BranchPushButton"
import { CopyButton } from "@/components/shared/CopyButton"
import { blockingBranchOf } from "@/lib/stack"
import type { Branch } from "@/stores/branch-sync"
import { getFullBranchName } from "@/stores/repository"

/** Actions column of an active branch row: copy the full branch name, push (disabled for a blocked stacked branch). */
export function BranchActions({ branch }: { branch: Branch }) {
  return (
    <div className="flex items-center gap-2">
      <CopyButton text={() => getFullBranchName(branch.name)} tooltip="Copy full branch name" alwaysVisible />

      {branch.status === "Blocked"
        ? <BlockedPushButton branch={branch} />
        : !branch.hasError && branch.commitCount > 0 && <BranchPushButton branch={branch} />}
    </div>
  )
}

/** A blocked branch keeps its last build, which doesn't contain its parent's state: pushing it would mislead */
function BlockedPushButton({ branch }: { branch: Branch }) {
  const blocker = blockingBranchOf(branch)
  return (
    <Tooltip>
      {/* a disabled button gets no pointer events: the tooltip is on a wrapper */}
      <TooltipTrigger render={<span data-testid="blocked-push" />}>
        <Button size="xs" variant="ghost" disabled>
          <UploadIcon data-icon="inline-start" />
          Push
        </Button>
      </TooltipTrigger>
      <TooltipContent>{blocker === null ? "Blocked: fix the parent branch first" : `Blocked by ${blocker}: fix it first, then sync`}</TooltipContent>
    </Tooltip>
  )
}
