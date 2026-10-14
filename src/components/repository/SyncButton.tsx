import { RefreshCwIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { syncBranches, useBranchSync } from "@/stores/branch-sync"
import { useRepository, useSelectedProject } from "@/stores/repository"

export function SyncButton() {
  const isSyncing = useBranchSync(state => state.isSyncing)
  const syncCount = useBranchSync(state => state.syncCount)
  const syncError = useBranchSync(state => state.syncError)
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
      data-sync-count={syncCount}
      onClick={() => void syncBranches()}
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
