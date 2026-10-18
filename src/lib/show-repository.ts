import { repositoryName } from "@/lib/repository-config"
import { branchSyncStore } from "@/stores/branch-sync"
import { getSelectedProject, selectProject } from "@/stores/repository"
import { getRecentProjects } from "@/stores/settings/repositorySettings"

/** How long the switch waits for the first sync of the repository it shows; a sync that takes longer is told */
const SHOW_TIMEOUT_MS = 120_000

/**
 * Switches the main window to the repository a sub-window acts on and waits until its branches are synced, so the action
 * the window asked for runs on them: a review window of another repository never has the user select it first. Main
 * window only. Resolves to null, or why the repository can't be shown (no recent project any more, no sync).
 */
export async function showRepository(repositoryPath: string): Promise<string | null> {
  const name = repositoryName(repositoryPath)
  if (getSelectedProject()?.path !== repositoryPath) {
    const project = getRecentProjects().find(it => it.path === repositoryPath)
    if (project === undefined) {
      return `${name} is no longer among the repositories of the main window`
    }
    selectProject(project)
  }
  return (await whenRepositorySynced(repositoryPath)) ? null : `The branches of ${name} didn't sync`
}

/** Resolves once the branches of `repositoryPath` are synced and no sync runs; false after `SHOW_TIMEOUT_MS` */
function whenRepositorySynced(repositoryPath: string): Promise<boolean> {
  const synced = () => {
    const state = branchSyncStore.getState()
    return state.syncedRepositoryPath === repositoryPath && !state.isSyncing && getSelectedProject()?.path === repositoryPath
  }
  if (synced()) {
    return Promise.resolve(true)
  }
  return new Promise(resolve => {
    const timer = setTimeout(() => {
      unsubscribe()
      resolve(false)
    }, SHOW_TIMEOUT_MS)
    const unsubscribe = branchSyncStore.subscribe(() => {
      if (synced()) {
        clearTimeout(timer)
        unsubscribe()
        resolve(true)
      }
    })
  })
}
