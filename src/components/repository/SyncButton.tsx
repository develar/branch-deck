import { cn } from "cn"
import { RefreshCwIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Kbd } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { useAppCommand } from "@/hooks/useAppCommand"
import { useSyncAvailability } from "@/hooks/useSyncAvailability"
import { isMac } from "@/lib/platform"
import { branchSyncStore, syncBranches, syncIfChanged, useBranchSync } from "@/stores/branch-sync"

/**
 * Header button of Sync (⌘R), the one indicator of a running sync: its spinner, and its tooltip names the step. A
 * failed sync turns it `destructive` with the error as its tooltip, next to the page alert; an invalid repository or a
 * missing branch prefix disables it with the reason as its tooltip. The menu item (⌘R) does what the button does:
 * nothing it can't, and while a sync runs only a check once it is done.
 */
export function SyncButton() {
  const isSyncing = useBranchSync(state => state.isSyncing)
  const syncCount = useBranchSync(state => state.syncCount)
  const syncError = useBranchSync(state => state.syncError)
  const { canSync, reason } = useSyncAvailability()

  // "Sync" menu item (⌘R); the store, not the render: the handler is replaced after a render
  useAppCommand("sync_branches", () => {
    if (!canSync) {
      return
    }
    if (branchSyncStore.getState().isSyncing) {
      // a commit elsewhere during a long sync that started before it: synced after it, not lost
      void syncIfChanged()
    } else {
      void syncBranches({ manual: true })
    }
  })

  return (
    <Tooltip>
      <TooltipTrigger
        render={
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
            onClick={() => void syncBranches({ manual: true })}
          />
        }
      >
        {isSyncing ? <Spinner data-icon="inline-start" /> : <RefreshCwIcon data-icon="inline-start" />}
        Sync
      </TooltipTrigger>
      <TooltipContent data-testid="sync-tooltip">
        {reason !== null ? (
          reason
        ) : isSyncing ? (
          // the shortcut does nothing while it runs
          "Syncing branches…"
        ) : syncError ? (
          <div className="flex flex-col gap-0.5">
            <span>{syncError}</span>
            <SyncShortcut className="opacity-70" />
          </div>
        ) : (
          <SyncShortcut />
        )}
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
