import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"
import { Channel } from "@tauri-apps/api/core"
import { toast } from "@/components/ui/toast"
import type { TransferProgress, UpdateCheck, UpdatePhase, UpdateProgress } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand } from "@/lib/command"
import { notifyError } from "@/lib/errors"
import { appliedUpdateToast } from "@/lib/landed"
import { recordUpdate } from "@/stores/activity"
import { setBaselineUpdate, syncBranches } from "@/stores/branch-sync"
import { addLandedUpdate } from "@/stores/landed"
import { getSelectedProject, useSelectedProjectPath } from "@/stores/repository"
import { updateProject } from "@/stores/settings/repositorySettings"

/** An update or a fetch of the baseline that runs in a repository */
export interface RunningUpdate {
  /** The step it is at */
  readonly phase: UpdatePhase
  /** How far its fetch is; null before git reports a stage and after the fetch */
  readonly fetch: TransferProgress | null
}

export interface UpdateProjectState {
  /**
   * What runs, by repository path: the Fetch of a merge toast fetches the baseline of its own repository, which need not
   * be the selected one, and the header shows only what runs in the selected repository
   */
  readonly running: Readonly<Record<string, RunningUpdate>>
  /** A conflicting or blocked update to explain; null while the dialog is closed */
  readonly refused: UpdateCheck | null
}

export const updateProjectStore = createStore<UpdateProjectState>(() => ({
  running: {},
  refused: null,
}))

/** What runs in the repository; undefined when nothing does, or without a repository */
export function selectRunningUpdate(state: UpdateProjectState, repositoryPath: string | undefined): RunningUpdate | undefined {
  return repositoryPath === undefined ? undefined : state.running[repositoryPath]
}

/** The step of the update or fetch running in the selected repository; null when none runs there */
export function useUpdatePhase(): UpdatePhase | null {
  const repositoryPath = useSelectedProjectPath()
  return useStore(updateProjectStore, state => selectRunningUpdate(state, repositoryPath)?.phase ?? null)
}

/** How far the fetch running in the selected repository is */
export function useFetchProgress(): TransferProgress | null {
  const repositoryPath = useSelectedProjectPath()
  return useStore(updateProjectStore, state => selectRunningUpdate(state, repositoryPath)?.fetch ?? null)
}

export function useRefusedUpdate(): UpdateCheck | null {
  return useStore(updateProjectStore, state => state.refused)
}

export function closeRefusedUpdate(): void {
  updateProjectStore.setState({ refused: null })
}

function setRunning(repositoryPath: string, running: RunningUpdate | null): void {
  const { [repositoryPath]: _previous, ...others } = updateProjectStore.getState().running
  updateProjectStore.setState({ running: running === null ? others : { ...others, [repositoryPath]: running } })
}

/** Takes the one-at-a-time slot of the repository for an update or a fetch; false when one runs there already */
function startRunning(repositoryPath: string): boolean {
  if (selectRunningUpdate(updateProjectStore.getState(), repositoryPath) !== undefined) {
    return false
  }
  setRunning(repositoryPath, { phase: "fetching", fetch: null })
  return true
}

/**
 * Fetches the baseline, checks the update and applies it if it is clean (Update Project, ⌘T). A conflict or a block
 * opens the dialog that explains it; nothing changed then. One update at a time in a repository.
 */
export async function runUpdateProject(): Promise<void> {
  const repositoryPath = getSelectedProject()?.path
  if (repositoryPath === undefined || !startRunning(repositoryPath)) {
    return
  }
  try {
    const progress = new Channel<UpdateProgress>()
    // the fetch is done once the check starts: an error after it (e.g. bd holds the repository) keeps the fetch time
    let fetched = false
    progress.onmessage = ({ phase, fetch }) => {
      fetched ||= phase !== "fetching"
      setRunning(repositoryPath, { phase, fetch })
    }
    const result = await invokeCommand(() => commands.updateProject({ repositoryPath }, progress))
    if (result.status !== "ok") {
      if (fetched) {
        updateProject(repositoryPath, { lastFetchTime: Date.now() })
      }
      notifyError("Cannot update the project", result.error)
      return
    }

    updateProject(repositoryPath, { lastFetchTime: Date.now() })
    const { check, applied, landed } = result.data
    recordUpdate(repositoryPath, result.data)
    switch (check.state.status) {
      case "upToDate":
        setBaselineUpdate(repositoryPath, check)
        toast.add({ type: "info", title: `${check.branch} is already up to date with ${check.upstream}` })
        break
      case "clean":
        if (applied !== null) {
          setBaselineUpdate(repositoryPath, { ...check, head: applied.newHead, state: { status: "upToDate" } })
          // the island of what landed shows at once, the sync that follows confirms it
          addLandedUpdate(repositoryPath, landed)
          toast.add({ type: "success", ...appliedUpdateToast(check, applied, landed) })
          // the table shows the selected repository: another one selected meanwhile has nothing to sync for this update
          if (getSelectedProject()?.path === repositoryPath) {
            void syncBranches({ autoScroll: false, autoExpand: false })
          }
        }
        break
      case "conflict":
      case "blocked":
        setBaselineUpdate(repositoryPath, check)
        updateProjectStore.setState({ refused: check })
        break
    }
  }
  finally {
    setRunning(repositoryPath, null)
  }
}

/**
 * Fetches only the baseline of the repository, without updating (the Fetch of a merged robot's toast): the button shows
 * the fetch as for an update, then a sync of the selected repository checks the update against the fetched baseline.
 * Shares the one-at-a-time slot of the update in the repository.
 */
export async function runFetchBaseline(repositoryPath: string): Promise<void> {
  if (!startRunning(repositoryPath)) {
    return
  }
  try {
    const progress = new Channel<TransferProgress>()
    progress.onmessage = (fetch) => {
      setRunning(repositoryPath, { phase: "fetching", fetch })
    }
    const result = await invokeCommand(() => commands.fetchBaseline({ repositoryPath }, progress))
    if (result.status !== "ok") {
      notifyError("Cannot fetch the baseline", result.error)
      return
    }
    updateProject(repositoryPath, { lastFetchTime: Date.now() })
    if (getSelectedProject()?.path === repositoryPath) {
      void syncBranches({ autoScroll: false, autoExpand: false })
    }
  }
  finally {
    setRunning(repositoryPath, null)
  }
}
