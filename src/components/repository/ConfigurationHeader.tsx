import { lazy, Suspense, useState } from "react"
import { CircleAlertIcon, Settings2Icon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { Switch } from "@/components/ui/switch"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import { BranchPrefixField } from "@/components/repository/BranchPrefixField"
import { PullRequestChip } from "@/components/branch/PullRequestChip"
import { OutgoingIndicator } from "@/components/repository/OutgoingIndicator"
import { PullRequestStatusLine } from "@/components/repository/PullRequestStatusLine"
import { RepositorySelector } from "@/components/repository/RepositorySelector"
import { SyncButton } from "@/components/repository/SyncButton"
import { UpdateProjectButton, UpdateStateLine } from "@/components/repository/UpdateProjectButton"
import { usePullRequestOverview } from "@/hooks/usePullRequestOverview"
import { useTauriListen } from "@/hooks/useTauriListen"
import { formatRelativeTime } from "@/lib/time"
import { useBranchSync } from "@/stores/branch-sync"
import { useRepository, useSelectedProject } from "@/stores/repository"
import { setAutoSyncOnFocus, useAppSettings } from "@/stores/settings/appSettings"
import { closeRefusedUpdate, runUpdateProject, useRefusedUpdate } from "@/stores/update-project"

const UpdateBlockedDialog = lazy(() => import("@/components/repository/UpdateBlockedDialog").then(it => ({ default: it.UpdateBlockedDialog })))

function renderPullRequestChip(branchName: string) {
  return <PullRequestChip branchName={branchName} />
}

export function ConfigurationHeader() {
  const isSyncing = useBranchSync(state => state.isSyncing)
  const syncError = useBranchSync(state => state.syncError)
  const branchCount = useBranchSync(state => state.branchOrder.length)
  const pathValidation = useRepository(state => state.pathValidation)
  const selectedProject = useSelectedProject()
  const autoSyncOnFocus = useAppSettings(state => state.autoSyncOnFocus)
  const lastSyncTime = selectedProject?.lastSyncTime
  const refusedUpdate = useRefusedUpdate()
  const pullRequestOverview = usePullRequestOverview()
  // mounted on the first refusal and kept, so the dialog animates and returns the focus when it closes
  const [isRefusedDialogMounted, setRefusedDialogMounted] = useState(false)
  if (refusedUpdate !== null && !isRefusedDialogMounted) {
    setRefusedDialogMounted(true)
  }
  // the menu item Update Project (⌘T); for a local baseline the update says there is nothing to update from
  useTauriListen("update-project", () => {
    if (pathValidation.valid) {
      void runUpdateProject()
    }
  })

  return (
    <Card className="gap-0 py-0" data-testid="configuration-header">
      <div className="flex h-14 items-center justify-between gap-4 px-4">
        {/* repository info */}
        <div className="flex min-w-0 items-center gap-3">
          <RepositorySelector disabled={isSyncing} />

          <BranchPrefixField disabled={isSyncing || !pathValidation.valid} />

          <Popover>
            <PopoverTrigger
              render={<Button variant="ghost" size="icon-sm" aria-label="Sync settings" data-testid="sync-settings-button" />}
            >
              <Settings2Icon />
            </PopoverTrigger>
            <PopoverContent className="w-auto min-w-[200px] gap-2 p-3">
              <label className="flex items-center gap-2 text-sm">
                <Switch size="sm" checked={autoSyncOnFocus} onCheckedChange={setAutoSyncOnFocus} />
                Auto-sync on focus
              </label>
              <div className="text-xs text-muted-foreground">
                Automatically sync when the window gains focus
              </div>
            </PopoverContent>
          </Popover>

          {selectedProject && branchCount > 0 && pathValidation.valid && (
            <div className="hidden items-center gap-3 text-xs text-muted-foreground sm:flex">
              <span className="whitespace-nowrap">
                {branchCount}
                {" "}
                {branchCount === 1 ? "branch" : "branches"}
              </span>
            </div>
          )}
        </div>

        {/* actions: incoming and outgoing together, the last sync time next to Sync */}
        <div className="flex items-center gap-3">
          {pathValidation.valid && (
            // hidden while empty: an empty flex item would still take a gap
            <div className="flex items-center gap-1 empty:hidden">
              <UpdateProjectButton />
              <OutgoingIndicator
                renderPullRequest={renderPullRequestChip}
                reviewBranches={pullRequestOverview.openBranches}
                needsAttention={pullRequestOverview.needsAttention}
                footer={(
                  <>
                    <UpdateStateLine />
                    <PullRequestStatusLine />
                  </>
                )}
              />
            </div>
          )}
          {lastSyncTime && !isSyncing && pathValidation.valid
            ? (
                <div className="text-xs text-muted-foreground">
                  {`Last synced ${formatRelativeTime(lastSyncTime)}`}
                </div>
              )
            : null}
          {syncError && (
            <Tooltip>
              <TooltipTrigger render={<span className="inline-flex" />}>
                <CircleAlertIcon className="size-4 text-destructive" />
              </TooltipTrigger>
              <TooltipContent>{syncError}</TooltipContent>
            </Tooltip>
          )}
          <SyncButton />
        </div>
      </div>
      {isRefusedDialogMounted && (
        <Suspense fallback={null}>
          <UpdateBlockedDialog check={refusedUpdate} onClose={closeRefusedUpdate} />
        </Suspense>
      )}
    </Card>
  )
}
