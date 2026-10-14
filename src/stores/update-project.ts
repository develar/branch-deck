import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"
import { Channel } from "@tauri-apps/api/core"
import { toast } from "@/components/ui/toast"
import type { AppliedUpdate, FetchProgress, UpdateCheck, UpdatePhase, UpdateProgress } from "@/lib/bindings"
import { commands } from "@/lib/bindings"
import { invokeCommand } from "@/lib/command"
import { formatCommitCount } from "@/lib/commit"
import { notifyError } from "@/lib/errors"
import { setBaselineUpdate, syncBranches } from "@/stores/branch-sync"
import { getSelectedProject } from "@/stores/repository"
import { updateProject } from "@/stores/settings/repositorySettings"

export interface UpdateProjectState {
  /** The step of the running update; null when none runs */
  phase: UpdatePhase | null
  /** How far the fetch of the running update is; null before git reports a stage and after the fetch */
  fetch: FetchProgress | null
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

export function useFetchProgress(): FetchProgress | null {
  return useStore(updateProjectStore, state => state.fetch)
}

export function useRefusedUpdate(): UpdateCheck | null {
  return useStore(updateProjectStore, state => state.refused)
}

export function closeRefusedUpdate(): void {
  updateProjectStore.setState({ refused: null })
}

function appliedTitle(check: UpdateCheck, applied: AppliedUpdate): string {
  if (applied.kind === "fastForward") {
    return `Updated ${check.branch}: fast-forwarded to ${check.upstream}`
  }
  return `Updated ${check.branch}: rebased ${formatCommitCount(applied.replayed)} onto ${check.upstream}`
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
    const { check, applied } = result.data
    switch (check.state.status) {
      case "upToDate":
        setBaselineUpdate(repositoryPath, check)
        toast.add({ type: "info", title: `${check.branch} is already up to date with ${check.upstream}` })
        break
      case "clean":
        if (applied !== null) {
          setBaselineUpdate(repositoryPath, { ...check, head: applied.newHead, state: { status: "upToDate" } })
          toast.add({
            type: "success",
            title: appliedTitle(check, applied),
            description: applied.droppedAlreadyUpstream > 0
              ? `Dropped ${formatCommitCount(applied.droppedAlreadyUpstream)} already upstream`
              : undefined,
          })
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
