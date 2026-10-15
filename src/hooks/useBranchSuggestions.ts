import { useEffect, useRef, useState } from "react"
import { createStore } from "zustand/vanilla"
import { useStore } from "zustand"
import { commands } from "@/lib/bindings"
import type { Commit, SuggestBranchNamesParams } from "@/lib/bindings"
import { notifyError } from "@/lib/errors"
import { clearAIError, setAIError } from "@/stores/ai-suggestions"
import { useAISettings } from "@/stores/settings/aiSettings"
import type { AIMode } from "@/stores/settings/aiSettings"

export interface UseBranchSuggestionsOptions {
  repositoryPath: string
  commits: readonly Commit[]
  isActive: boolean
}

/** Why there are no suggestions, shown in their place: what the user has to do for them */
export type SuggestionHint = "claudeMissing" | "notLoggedIn"

/** Leading + trailing throttle of generation requests (selection changes, mode switches) */
const GENERATION_THROTTLE_MS = 300

/**
 * Branch name suggestions for the selected commits from the user's Claude Code.
 * Regenerates when the selection changes or AI gets enabled; an obsolete request is ignored (generation id), and the
 * backend kills its `claude` once a newer one starts.
 */
export function useBranchSuggestions({ repositoryPath, commits, isActive }: UseBranchSuggestionsOptions) {
  const aiMode = useAISettings(state => state.aiMode)

  const [session] = useState(createSuggestionSession)
  const names = useStore(session.store, state => state.names)
  const isGenerating = useStore(session.store, state => state.isGenerating)
  const hint = useStore(session.store, state => state.hint)

  const commitHashes = getCommitHashes(commits)

  // first effect: the requests below read the inputs of the current render
  useEffect(() => {
    session.setInputs({ repositoryPath, commits, isActive, aiMode })
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
    if (aiMode === "claude" && previous !== "claude") {
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

  return { names, isGenerating, hint }
}

interface SuggestionInputs {
  repositoryPath: string
  commits: readonly Commit[]
  isActive: boolean
  aiMode: AIMode
}

interface SuggestionState {
  names: readonly string[]
  isGenerating: boolean
  hint: SuggestionHint | null
}

function getCommitHashes(commits: readonly Commit[]): string {
  return commits.map(commit => commit.originalHash).join(",")
}

/** Generation state of one `useBranchSuggestions` instance, outside React so a late answer updates it directly. */
function createSuggestionSession() {
  const store = createStore<SuggestionState>(() => ({ names: [], isGenerating: false, hint: null }))

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
      && inputs.aiMode === "claude"
      // without names (a hint) it asks again: the user may have installed or logged in to Claude Code meanwhile
      && (getCommitHashes(inputs.commits) !== lastSuggestedHashes || store.getState().names.length === 0)
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

  async function generate({ repositoryPath, commits }: SuggestionInputs): Promise<void> {
    const params: SuggestBranchNamesParams = {
      repositoryPath,
      // original hash, not the synced one
      commits: commits.map(commit => ({ hash: commit.originalHash, message: commit.message })),
    }

    const thisGenerationId = ++generationId
    const isObsolete = () => thisGenerationId !== generationId
    store.setState({ isGenerating: true })

    let result: Awaited<ReturnType<typeof commands.suggestBranchNames>>
    try {
      result = await commands.suggestBranchNames(params)
    }
    catch (error) {
      if (!isObsolete()) {
        fail(error)
      }
      return
    }
    if (isObsolete()) {
      return
    }
    if (result.status === "error") {
      fail(new Error(result.error.message))
      return
    }

    const outcome = result.data
    switch (outcome.status) {
      case "suggested": {
        store.setState({ names: outcome.names, isGenerating: false, hint: null })
        clearAIError()
        break
      }

      case "superseded": {
        // a request of another window replaced this one: the next request asks again
        lastSuggestedHashes = ""
        store.setState({ isGenerating: false })
        break
      }

      case "claudeMissing":
      case "notLoggedIn": {
        store.setState({ names: [], isGenerating: false, hint: outcome.status })
        clearAIError()
        break
      }

      case "error": {
        fail(new Error(outcome.message))
        break
      }
    }
  }

  function fail(error: unknown): void {
    store.setState({ isGenerating: false })
    setAIError(error)
    notifyError("Cannot suggest branch names", error)
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
    request,
    cancel,
    setInputs: (value: SuggestionInputs) => {
      inputs = value
    },
    clear: () => store.setState({ names: [], hint: null }),
    dispose: () => {
      cancel()
      clearTimeout(trailingTimer)
      trailingTimer = undefined
    },
  }
}
