import { cn } from "cn"
import { RefreshCwIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { ShortcutHint } from "@/components/shared/ShortcutHint"
import { useAppCommand } from "@/hooks/useAppCommand"
import { useSyncAvailability } from "@/hooks/useSyncAvailability"
import { isMac } from "@/lib/platform"
import { describeSyncStep } from "@/lib/sync-step"
import { viewVersionKey } from "@/model/store"
import { branchSyncStore, syncBranches, syncIfChanged, useBranchSync } from "@/stores/branch-sync"

const SYNC_KBD = isMac ? "⌘R" : "Ctrl+R"

/**
 * Header button of Sync (⌘R), the one indicator of a running sync: its spinner, and its tooltip names the step. A
 * failed sync turns it `destructive` with the error as its tooltip, next to the page alert; an invalid repository or a
 * missing branch prefix disables it with the reason as its tooltip. The menu item (⌘R) does what the button does:
 * nothing it can't, and while a sync runs only a check once it is done.
 */
export function SyncButton() {
  const isSyncing = useBranchSync(state => state.isSyncing)
  const syncCount = useBranchSync(state => state.syncCount)
  const viewVersion = useBranchSync(state => viewVersionKey(state.viewVersion))
  const syncError = useBranchSync(state => state.syncError)
  const syncStep = useBranchSync(state => state.syncStep)
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
      void syncBranches("manual")
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
            // the version of the session's view: a test waits for the patch of a sync by its change
            data-view-version={viewVersion ?? undefined}
            onClick={() => void syncBranches("manual")}
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
          describeSyncStep(syncStep)
        ) : syncError ? (
          <div className="flex flex-col gap-0.5">
            <span>{syncError}</span>
            <ShortcutHint kbd={SYNC_KBD} className="opacity-70">
              Sync branches
            </ShortcutHint>
          </div>
        ) : (
          <ShortcutHint kbd={SYNC_KBD}>Sync branches</ShortcutHint>
        )}
      </TooltipContent>
    </Tooltip>
  )
}
