import { RefreshCwIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useRepository, useSelectedProject } from "@/stores/repository"

export interface SyncButtonProps {
  isSyncing: boolean
  syncError: string | null
  onSync: () => void
}

// TODO(Phase 5): read isSyncing/syncError/syncBranches from the branch-sync store instead of props
export function SyncButton({ isSyncing, syncError, onSync }: SyncButtonProps) {
  const pathValidation = useRepository(state => state.pathValidation)
  const hasSelectedProject = useSelectedProject() !== null
  const canSync = hasSelectedProject && pathValidation.valid

  const button = (
    <Button
      size="sm"
      variant={syncError ? "destructive" : "default"}
      disabled={isSyncing || !canSync}
      aria-busy={isSyncing || undefined}
      data-testid="sync-button"
      onClick={onSync}
    >
      {isSyncing ? <Spinner data-icon="inline-start" /> : <RefreshCwIcon data-icon="inline-start" />}
      Sync
    </Button>
  )

  if (canSync || !pathValidation.error) {
    return button
  }

  // a disabled button gets no pointer events, so the tooltip is attached to a wrapper
  return (
    <Tooltip>
      <TooltipTrigger render={<span className="inline-flex" />}>
        {button}
      </TooltipTrigger>
      <TooltipContent>{pathValidation.error}</TooltipContent>
    </Tooltip>
  )
}
