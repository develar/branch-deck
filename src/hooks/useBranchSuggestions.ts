import { useEffect, useRef, useState } from "react"
import { Channel } from "@tauri-apps/api/core"
import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"
import { commands } from "@/lib/bindings"
import type { BranchSuggestion, Commit, SuggestBranchNameParams, SuggestionProgress } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { clearAIError, modelStore, requestDownload, setAIError, useModelStore } from "@/stores/model"
import { useAISettings } from "@/stores/settings/aiSettings"
import type { AIMode } from "@/stores/settings/aiSettings"

export interface UseBranchSuggestionsOptions {
  repositoryPath: string
  branchPrefix: string
  commits: readonly Commit[]
  isActive: boolean
}

/** Two slots until the backend reports the actual count */
const EMPTY_SLOTS: readonly (BranchSuggestion | null)[] = [null, null]

/** Leading + trailing throttle of generation requests (selection changes, mode switches, download completion) */
const GENERATION_THROTTLE_MS = 300

/**
 * AI branch name suggestions for the selected commits, streamed with progress.
 * Regenerates when the selection changes, AI gets enabled or the model download completes;
 * an obsolete generation is ignored (generation id) and cancelled on deactivation/unmount.
 */
export function useBranchSuggestions({ repositoryPath, branchPrefix, commits, isActive }: UseBranchSuggestionsOptions) {
  const aiMode = useAISettings(state => state.aiMode)
  const isDownloading = useModelStore(state => state.isDownloading)

  const [session] = useState(createSuggestionSession)
  const slots = useStore(session.store, state => state.slots)
  const isGenerating = useStore(session.store, state => state.isGenerating)

  const commitHashes = getCommitHashes(commits)

  // first effect: the requests below read the inputs of the current render
  useEffect(() => {
    session.setInputs({ repositoryPath, branchPrefix, commits, isActive, aiMode })
  })

  useEffect(() => {
    if (!isActive) {
      session.cancel()
      return
    }

    session.request()
    return () => session.dispose()
  }, [session, isActive])

  const previousAiModeRef = useRef(aiMode)
  useEffect(() => {
    const previous = previousAiModeRef.current
    previousAiModeRef.current = aiMode
    if (aiMode === "enabled" && previous !== "enabled") {
      session.request()
    }
    else if (aiMode === "disabled" && previous !== "disabled") {
      session.clear()
    }
  }, [session, aiMode])

  const previousCommitHashesRef = useRef(commitHashes)
  useEffect(() => {
    if (previousCommitHashesRef.current !== commitHashes) {
      previousCommitHashesRef.current = commitHashes
      session.request()
    }
  }, [session, commitHashes])

  const wasDownloadingRef = useRef(isDownloading)
  useEffect(() => {
    const wasDownloading = wasDownloadingRef.current
    wasDownloadingRef.current = isDownloading
    // retry once the model download completes
    if (wasDownloading && !isDownloading && session.shouldGenerate()) {
      session.request()
    }
  }, [session, isDownloading])

  const suggestions = slots.filter(it => it !== null)
  return {
    suggestions,
    isGenerating,
    /** Percentage (0-100) of received suggestions */
    loadingProgress: slots.length > 0 ? (suggestions.length / slots.length) * 100 : 0,
  }
}

interface SuggestionInputs {
  repositoryPath: string
  branchPrefix: string
  commits: readonly Commit[]
  isActive: boolean
  aiMode: AIMode
}

interface SuggestionState {
  slots: readonly (BranchSuggestion | null)[]
  isGenerating: boolean
}

function getCommitHashes(commits: readonly Commit[]): string {
  return commits.map(commit => commit.originalHash).join(",")
}

/** Generation state of one `useBranchSuggestions` instance, outside React so streamed events update it directly. */
function createSuggestionSession() {
  const store = createStore<SuggestionState>(() => ({ slots: EMPTY_SLOTS, isGenerating: false }))

  let inputs: SuggestionInputs | null = null
  // incremented to invalidate the running generation
  let generationId = 0
  // commit hashes the last generation was started for
  let lastSuggestedHashes = ""
  let lastRunTime = 0
  let trailingTimer: ReturnType<typeof setTimeout> | undefined

  function shouldGenerate(): boolean {
    return inputs !== null
      && inputs.isActive
      && inputs.commits.length > 0
      && inputs.aiMode === "enabled"
      && (getCommitHashes(inputs.commits) !== lastSuggestedHashes || store.getState().slots.every(it => it === null))
  }

  /** Throttled (leading + trailing); the conditions are checked when the request runs */
  function request(): void {
    const wait = GENERATION_THROTTLE_MS - (Date.now() - lastRunTime)
    if (wait <= 0) {
      run()
    }
    else if (trailingTimer === undefined) {
      trailingTimer = setTimeout(() => {
        trailingTimer = undefined
        run()
      }, wait)
    }
  }

  function run(): void {
    lastRunTime = Date.now()
    if (inputs !== null && shouldGenerate()) {
      lastSuggestedHashes = getCommitHashes(inputs.commits)
      void generate(inputs)
    }
  }

  async function generate({ repositoryPath, branchPrefix, commits }: SuggestionInputs): Promise<void> {
    // a download is running; its completion triggers a new request
    if (modelStore.getState().isDownloading) {
      return
    }

    const params: SuggestBranchNameParams = {
      repositoryPath,
      branchPrefix,
      // original hash, not the synced one
      commits: commits.map(commit => ({ hash: commit.originalHash, message: commit.message })),
    }

    const thisGenerationId = ++generationId
    const isObsolete = () => thisGenerationId !== generationId
    store.setState({ isGenerating: true })

    const channel = new Channel<SuggestionProgress>()
    channel.onmessage = (progress) => {
      if (isObsolete()) {
        return
      }

      switch (progress.type) {
        case "Started": {
          store.setState({ slots: Array.from({ length: progress.data.total }, () => null) })
          break
        }

        case "SuggestionReady": {
          const { suggestion, index } = progress.data
          const { slots } = store.getState()
          if (index < slots.length) {
            store.setState({ slots: slots.with(index, suggestion) })
          }
          break
        }

        case "Completed": {
          store.setState({ isGenerating: false })
          clearAIError()
          break
        }

        case "Cancelled": {
          // e.g., backend lock contention with another request; keep the suggestions
          store.setState({ isGenerating: false })
          break
        }

        case "Error": {
          store.setState({ isGenerating: false })
          notifyError("Branch Name Generation Failed", progress.data.message)
          break
        }

        case "ModelDownloadInProgress": {
          // the model is missing: download it, the download completion triggers a new request
          store.setState({ isGenerating: false })
          requestDownload({ name: progress.data.model_name, size: progress.data.model_size })
          break
        }
      }
    }

    try {
      const result = await commands.suggestBranchNameStream(params, channel)
      if (result.status === "error") {
        throw new Error(result.error.message)
      }
    }
    catch (error) {
      if (isObsolete()) {
        return
      }
      store.setState({ isGenerating: false })
      setAIError(error instanceof Error ? error : new Error("Failed to generate suggestions"))
      notifyError("Branch Name Generation Failed", error)
    }
  }

  /** Ignores the running generation; keeps the suggestions */
  function cancel(): void {
    generationId++
    if (store.getState().isGenerating) {
      store.setState({ isGenerating: false })
    }
  }

  return {
    store,
    shouldGenerate,
    request,
    cancel,
    setInputs: (value: SuggestionInputs) => {
      inputs = value
    },
    clear: () => store.setState({ slots: EMPTY_SLOTS }),
    dispose: () => {
      cancel()
      clearTimeout(trailingTimer)
      trailingTimer = undefined
    },
  }
}
