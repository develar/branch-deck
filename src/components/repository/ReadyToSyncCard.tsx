import { RefreshCwIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Kbd } from "@/components/ui/kbd"
import { Spinner } from "@/components/ui/spinner"
import { InlineCode } from "@/components/shared/InlineCode"
import { isMac } from "@/lib/platform"
import { syncBranches, useBranchSync } from "@/stores/branch-sync"
import { useEffectiveBranchPrefix } from "@/stores/repository"

/**
 * The page of a repository never synced: nothing reads or writes branches until the user syncs, so the page says what
 * Sync does and when it runs from then on. A repository synced before syncs when the window opens it (`useSyncOnOpen`).
 */
export function ReadyToSyncCard() {
  const branchPrefix = useEffectiveBranchPrefix()
  // the first sync of a large repository takes a while: the card stays, with the spinner on its button, as on Sync
  const isSyncing = useBranchSync(state => state.isSyncing)
  return (
    <Card data-testid="ready-to-sync-card">
      <CardHeader>
        <CardTitle>Ready to Sync</CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-3 text-sm text-muted-foreground">
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
        <p>
          After this first sync, Branch Deck syncs whenever it opens the repository, after each change it makes, and when you come back to the window after a commit, a fetch or a
          push elsewhere (Auto-sync on Focus, in the sync settings next to Sync).
        </p>
        <div className="flex items-center gap-2">
          <Button size="sm" disabled={isSyncing} aria-busy={isSyncing || undefined} data-testid="ready-to-sync-button" onClick={() => void syncBranches({ manual: true })}>
            {isSyncing ? <Spinner data-icon="inline-start" /> : <RefreshCwIcon data-icon="inline-start" />}
            Sync
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
