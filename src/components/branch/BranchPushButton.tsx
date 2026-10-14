import { UploadIcon } from "lucide-react"
import { DroppedCommitList } from "@/components/branch/DroppedCommitList"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { Spinner } from "@/components/ui/spinner"
import { usePushButton } from "@/hooks/usePushButton"
import { pushBranch } from "@/lib/git/push"
import { ghostButtonColorClass } from "@/lib/ui-types"
import type { Branch } from "@/stores/branch-sync"
import { useBranchSync, useIsBranchPushing } from "@/stores/branch-sync"
import { useIsRowProcessing } from "@/stores/inline-row-action"

export interface BranchPushButtonProps {
  branch: Branch
  /** The text takes the push colour of the branch (`getPushColor`); off where a bar carries it (the outgoing overview) */
  colored?: boolean
}

/**
 * Push button of an active branch ("Push (2)", "Create Remote", "Force Push (1)"); the full description is shown on
 * hover, with the remote commits a force push drops.
 */
export function BranchPushButton({ branch, colored = true }: BranchPushButtonProps) {
  const isSyncing = useBranchSync(state => state.isSyncing)
  const isPushing = useIsBranchPushing(branch.name)
  const processing = useIsRowProcessing(branch.name)
  const push = usePushButton(branch, isSyncing, isPushing)

  return (
    <Popover>
      <PopoverTrigger
        openOnHover
        delay={300}
        render={(
          <Button
            size="xs"
            variant="ghost"
            className={colored ? ghostButtonColorClass(push.color) : undefined}
            disabled={push.disabled || processing}
            aria-busy={isPushing || undefined}
            onClick={(event) => {
              event.stopPropagation()
              void pushBranch(branch.name)
            }}
          />
        )}
      >
        {isPushing ? <Spinner data-icon="inline-start" /> : <UploadIcon data-icon="inline-start" />}
        {push.text}
      </PopoverTrigger>
      <PopoverContent className="w-auto max-w-sm gap-2 px-3 py-2 text-sm leading-snug">
        <div className="break-words whitespace-pre-line">{push.tooltip}</div>
        {push.dropped !== null && <DroppedCommitList dropped={push.dropped} />}
      </PopoverContent>
    </Popover>
  )
}
