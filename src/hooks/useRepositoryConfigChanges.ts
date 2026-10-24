import { resetStackIndependence } from "@/hooks/useStackIndependence"
import { useTauriListen } from "@/hooks/useTauriListen"
import type { RepositoryConfigChanged } from "@/lib/repository-config"
import { syncBranches } from "@/stores/branch-sync"
import { refreshPullRequests } from "@/stores/pull-requests"
import { getSelectedProject, refreshBranchPrefix } from "@/stores/repository"

/**
 * The main window after a write of Settings → Repository, → Diffs & Merge or → Agents (`repository_config_changed`, sent by the
 * backend): what the key changes for the selected repository, nothing for another one. A repository synced before
 * syncs again, as the branches are grouped, named, reviewed or merged by the key; one never synced waits for the
 * user's first sync.
 */
export function useRepositoryConfigChanges(): void {
  useTauriListen<RepositoryConfigChanged>("repository_config_changed", ({ repositoryPath, key }) => {
    const project = getSelectedProject()
    if (project?.path !== repositoryPath) {
      return
    }
    const resync = () => {
      if (project.lastSyncTime !== undefined) {
        void syncBranches()
      }
    }
    switch (key) {
      case "branchPrefix":
        // the names the page shows; the sync resolves the new prefix in Rust from the config it reads
        refreshBranchPrefix()
        resync()
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
      case "agentLease":
      case "agentSubjectBranch":
        // only coding agents read them (`bd workspace`): the branches stay as they are
        break
    }
  })
}
