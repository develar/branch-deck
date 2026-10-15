import { RefreshCwIcon } from "lucide-react"
import { cn } from "cn"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { isMac } from "@/lib/platform"
import { syncBranches, useBranchSync } from "@/stores/branch-sync"
import { useEffectiveBranchPrefix, useRepository, useSelectedProject } from "@/stores/repository"

/**
 * Header button of Sync (⌘R). A failed sync turns it `destructive` with the error as its tooltip, next to the page
 * alert; an invalid repository or a missing branch prefix disables it with the reason as its tooltip.
 */
export function SyncButton() {
  const isSyncing = useBranchSync(state => state.isSyncing)
  const syncCount = useBranchSync(state => state.syncCount)
  const syncError = useBranchSync(state => state.syncError)
  const pathValidation = useRepository(state => state.pathValidation)
  const hasSelectedProject = useSelectedProject() !== null
  // not while the prefix loads: the button would flicker; sync waits for the load
  const effectiveBranchPrefix = useEffectiveBranchPrefix()
  const isLoadingBranchPrefix = useRepository(state => state.isLoadingBranchPrefix)
  const hasBranchPrefix = effectiveBranchPrefix !== "" || isLoadingBranchPrefix
  const canSync = hasSelectedProject && pathValidation.valid && hasBranchPrefix

  return (
    <Tooltip>
      <TooltipTrigger
        render={(
          <Button
            size="sm"
            variant={syncError ? "destructive" : "ghost"}
            // a running sync keeps the full colour beside its spinner, as Update does; a repository that can't sync dims it
            className={cn(!canSync && "opacity-50")}
            disabled={isSyncing || !canSync}
            // the tooltip stays reachable while the button is disabled
            focusableWhenDisabled
            aria-busy={isSyncing || undefined}
            data-testid="sync-button"
            data-sync-count={syncCount}
            onClick={() => void syncBranches()}
          />
        )}
      >
        {isSyncing ? <Spinner data-icon="inline-start" /> : <RefreshCwIcon data-icon="inline-start" />}
        Sync
      </TooltipTrigger>
      <TooltipContent data-testid="sync-tooltip">
        {!canSync && pathValidation.error
          ? pathValidation.error
          : hasSelectedProject && !hasBranchPrefix
            ? "Set the branch prefix first"
            : syncError
              ? (
                  <div className="flex flex-col gap-0.5">
                    <span>{syncError}</span>
                    <SyncShortcut className="opacity-70" />
                  </div>
                )
              : <SyncShortcut />}
      </TooltipContent>
    </Tooltip>
  )
}

function SyncShortcut({ className }: { className?: string }) {
  return (
    <span className={cn("flex items-center gap-1.5", className)}>
      Sync branches
      <Kbd>{isMac ? "⌘R" : "Ctrl+R"}</Kbd>
    </span>
  )
}
