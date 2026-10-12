import { UploadIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { Spinner } from "@/components/ui/spinner"
import { CopyButton } from "@/components/shared/CopyButton"
import { usePushButton } from "@/hooks/usePushButton"
import { pushBranch } from "@/lib/git/push"
import { ghostButtonColorClass } from "@/lib/ui-types"
import type { Branch } from "@/stores/branch-sync"
import { useBranchSync } from "@/stores/branch-sync"
import { useIsRowProcessing } from "@/stores/inline-row-action"
import { getFullBranchName } from "@/stores/repository"

/** Actions column of an active branch row: copy the full branch name, push. */
export function BranchActions({ branch }: { branch: Branch }) {
  const isSyncing = useBranchSync(state => state.isSyncing)
  const processing = useIsRowProcessing(branch.name)
  const push = usePushButton(branch, isSyncing)

  return (
    <div className="flex items-center gap-2">
      <CopyButton text={() => getFullBranchName(branch.name)} tooltip="Copy full branch name" alwaysVisible />

      {!branch.hasError && branch.commitCount > 0 && (
        <Popover>
          <PopoverTrigger
            openOnHover
            delay={300}
            render={(
              <Button
                size="xs"
                variant="ghost"
                className={ghostButtonColorClass(push.color)}
                disabled={push.disabled || processing}
                aria-busy={branch.isPushing || undefined}
                onClick={(event) => {
                  event.stopPropagation()
                  void pushBranch(branch.name)
                }}
              />
            )}
          >
            {branch.isPushing ? <Spinner data-icon="inline-start" /> : <UploadIcon data-icon="inline-start" />}
            {push.text}
          </PopoverTrigger>
          <PopoverContent className="w-auto max-w-sm px-3 py-2 text-sm leading-snug break-words whitespace-pre-line">
            {push.tooltip}
          </PopoverContent>
        </Popover>
      )}
    </div>
  )
}
