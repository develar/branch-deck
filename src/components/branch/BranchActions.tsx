import { BranchPushButton } from "@/components/branch/BranchPushButton"
import { CopyButton } from "@/components/shared/CopyButton"
import type { Branch } from "@/stores/branch-sync"
import { getFullBranchName } from "@/stores/repository"

/** Actions column of an active branch row: copy the full branch name, push (disabled for a blocked stacked branch). */
export function BranchActions({ branch }: { branch: Branch }) {
  return (
    <div className="flex items-center gap-2">
      <CopyButton text={() => getFullBranchName(branch.name)} tooltip="Copy full branch name" alwaysVisible />

      {!branch.hasError && branch.commitCount > 0 && <BranchPushButton branch={branch} confirmTarget="row" />}
    </div>
  )
}
