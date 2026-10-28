import { resetStackIndependence } from "@/hooks/useStackIndependence"
import { useTauriListen } from "@/hooks/useTauriListen"
import type { RepositoryConfigChanged } from "@/lib/repository-config"
import { resubscribeSession } from "@/model/store"
import { syncBranches } from "@/stores/branch-sync"
import { refreshPullRequests } from "@/stores/pull-requests"
import { getSelectedProject } from "@/stores/repository"
import { getRepositoryState } from "@/stores/settings/repositoryState"

/**
 * The main window after a write of Settings → Repository, → Diffs & Merge, → Landing or → Agents (`repository_config_changed`, sent by the
 * backend): what the key changes for the selected repository, nothing for another one. A repository synced before
 * syncs again, as the branches are grouped, reviewed or merged by the key; a write of the prefix subscribes the window
 * again (the session of a new prefix syncs as it opens); one never synced waits for the user's first sync.
 */
export function useRepositoryConfigChanges(): void {
  useTauriListen<RepositoryConfigChanged>("repository_config_changed", ({ repositoryPath, key }) => {
    const project = getSelectedProject()
    if (project?.path !== repositoryPath) {
      return
    }
    const resync = () => {
      if (getRepositoryState(repositoryPath)?.lastSyncTime !== undefined) {
        void syncBranches("afterRewrite")
      }
    }
    switch (key) {
      case "branchPrefix":
        // the names the page shows: the window subscribes again, and Rust resolves the prefix; a changed one is another
        // session, which syncs as it opens, the same one again changes nothing
        void resubscribeSession()
        break
      case "stacks":
        resync()
        break
      case "symbols":
        // the mode On orders and stacks the branches, and decides what a stacked branch needs of its parent
        resetStackIndependence()
        resync()
        break
      case "githubRepository":
        resync()
        void refreshPullRequests("manual")
        break
      case "resolveByWords":
        // a conflict of git that its words resolve is a commit of the sync, or a conflict again
        resync()
        break
      case "landing":
        // the route into master is read by the sync with its config: the steps of the rows follow it
        resync()
        break
      case "pushSizeLimit":
        // a push reads it as it starts: nothing to sync
        break
      case "agentLease":
      case "agentSubjectBranch":
        // only coding agents read them (`bd workspace`): the branches stay as they are
        break
    }
  })
}
