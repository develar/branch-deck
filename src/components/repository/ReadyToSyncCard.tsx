import { RefreshCwIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Kbd } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { InlineCode } from "@/components/shared/InlineCode"
import { RelativeTime } from "@/components/shared/RelativeTime"
import { isMac } from "@/lib/platform"
import { syncBranches, useBranchSync } from "@/stores/branch-sync"
import { useEffectiveBranchPrefix, useSelectedProject } from "@/stores/repository"

/**
 * The page before the first sync of the window: nothing reads or writes branches until the user syncs, so the page
 * says what Sync does (the first time in a repository) or when it last ran, instead of staying empty.
 */
export function ReadyToSyncCard() {
  const lastSyncTime = useSelectedProject()?.lastSyncTime
  const branchPrefix = useEffectiveBranchPrefix()
  // the first sync of a large repository takes a while: the card stays, with the progress on its button
  const isSyncing = useBranchSync(state => state.isSyncing)
  return (
    <Card data-testid="ready-to-sync-card">
      <CardHeader>
        <CardTitle>Ready to Sync</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-3 text-sm text-muted-foreground">
        {lastSyncTime
          ? (
              <p>
                {"Last synced "}
                <RelativeTime time={lastSyncTime} />
                . Sync to see the current branches, or turn on Auto-sync on Focus in the sync settings next to Sync.
              </p>
            )
          : (
              <>
                <p>
                  {"Sync groups the commits of your branch by the prefix of their subject, like "}
                  <InlineCode>(auth) Add login form</InlineCode>
                  {" or "}
                  <InlineCode>ABC-123 Fix login bug</InlineCode>
                  {", and creates a local branch "}
                  <InlineCode>{`${branchPrefix}/virtual/<name>`}</InlineCode>
                  {" for each group. Commits without a prefix are listed as unassigned."}
                </p>
                <p>Your working tree and the checked-out branch stay as they are.</p>
              </>
            )}
        <div className="flex items-center gap-2">
          <Button size="sm" disabled={isSyncing} aria-busy={isSyncing || undefined} data-testid="ready-to-sync-button" onClick={() => void syncBranches()}>
            {isSyncing ? <Spinner data-icon="inline-start" /> : <RefreshCwIcon data-icon="inline-start" />}
            {isSyncing ? "Syncing…" : "Sync"}
          </Button>
          <span className="flex items-center gap-1.5 text-xs">
            or press
            <Kbd>{isMac ? "⌘R" : "Ctrl+R"}</Kbd>
          </span>
        </div>
      </CardContent>
    </Card>
  )
}
