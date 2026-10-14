import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"
import { Channel } from "@tauri-apps/api/core"
import { toast } from "@/components/ui/toast"
import type { TransferProgress, UpdateCheck, UpdatePhase, UpdateProgress } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand } from "@/lib/command"
import { notifyError } from "@/lib/errors"
import { appliedUpdateToast } from "@/lib/landed"
import { setBaselineUpdate, syncBranches } from "@/stores/branch-sync"
import { addLandedUpdate } from "@/stores/landed"
import { getSelectedProject } from "@/stores/repository"
import { updateProject } from "@/stores/settings/repositorySettings"

export interface UpdateProjectState {
  /** The step of the running update; null when none runs */
  phase: UpdatePhase | null
  /** How far the fetch of the running update is; null before git reports a stage and after the fetch */
  fetch: TransferProgress | null
  /** A conflicting or blocked update to explain; null while the dialog is closed */
  refused: UpdateCheck | null
}

export const updateProjectStore = createStore<UpdateProjectState>(() => ({
  phase: null,
  fetch: null,
  refused: null,
}))

export function useUpdatePhase(): UpdatePhase | null {
  return useStore(updateProjectStore, state => state.phase)
}

export function useFetchProgress(): TransferProgress | null {
  return useStore(updateProjectStore, state => state.fetch)
}

export function useRefusedUpdate(): UpdateCheck | null {
  return useStore(updateProjectStore, state => state.refused)
}

export function closeRefusedUpdate(): void {
  updateProjectStore.setState({ refused: null })
}

/**
 * Fetches the baseline, checks the update and applies it if it is clean (Update Project, ⌘T). A conflict or a block
 * opens the dialog that explains it; nothing changed then. One update at a time.
 */
export async function runUpdateProject(): Promise<void> {
  const project = getSelectedProject()
  if (project === null || updateProjectStore.getState().phase !== null) {
    return
  }
  const repositoryPath = project.path
  updateProjectStore.setState({ phase: "fetching", fetch: null })
  try {
    const progress = new Channel<UpdateProgress>()
    progress.onmessage = ({ phase, fetch }) => {
      updateProjectStore.setState({ phase, fetch })
    }
    const result = await invokeCommand(() => commands.updateProject({ repositoryPath }, progress))
    if (result.status !== "ok") {
      notifyError("Failed to Update the Project", result.error)
      return
    }

    updateProject(repositoryPath, { lastFetchTime: Date.now() })
    const { check, applied, landed } = result.data
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
          void syncBranches({ autoScroll: false, autoExpand: false })
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
    updateProjectStore.setState({ phase: null, fetch: null })
  }
}

/**
 * Fetches only the baseline of the repository, without updating (the Fetch of a merged robot's toast): the button shows
 * the fetch as for an update, then a sync of the selected repository checks the update against the fetched baseline.
 * Shares the one-at-a-time slot of the update.
 */
export async function runFetchBaseline(repositoryPath: string): Promise<void> {
  if (updateProjectStore.getState().phase !== null) {
    return
  }
  updateProjectStore.setState({ phase: "fetching", fetch: null })
  try {
    const progress = new Channel<TransferProgress>()
    progress.onmessage = (fetch) => {
      updateProjectStore.setState({ fetch })
    }
    const result = await invokeCommand(() => commands.fetchBaseline({ repositoryPath }, progress))
    if (result.status !== "ok") {
      notifyError("Failed to Fetch the Baseline", result.error)
      return
    }
    updateProject(repositoryPath, { lastFetchTime: Date.now() })
    if (getSelectedProject()?.path === repositoryPath) {
      void syncBranches({ autoScroll: false, autoExpand: false })
    }
  }
  finally {
    updateProjectStore.setState({ phase: null, fetch: null })
  }
}
