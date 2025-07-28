import { Channel } from "@tauri-apps/api/core"
import type { Commit, BranchSuggestion, SuggestBranchNameParams, SuggestionProgress } from "~/utils/bindings"
import { commands } from "~/utils/bindings"
import { useModelState } from "./modelProvider"
// notifyError is auto-imported from shared-ui layer

interface UseBranchSuggestionsOptions {
  repositoryPath: string
  branchPrefix: string
  commits: Ref<Commit[]>
  isActive: Ref<boolean>
}

interface SuggestionState {
  isGenerating: boolean
  suggestions: (BranchSuggestion | null)[]
  error: string | null
  totalExpected: number
}

export function useBranchSuggestions(options: UseBranchSuggestionsOptions) {
  const { repositoryPath, branchPrefix, commits, isActive } = options

  // AI state management
  const { aiEnabled, setAIError, clearAIError } = useAIToggle()
  const modelState = useModelState()

  // State for streaming suggestions
  const state = reactive<SuggestionState>({
    isGenerating: false,
    suggestions: [null, null], // Initialize with 2 null slots
    error: null,
    totalExpected: 2,
  })

  // Track current generation to handle cancellation
  let currentGenerationId = 0
  let activeGenerationId = 0

  // Track last commits we generated suggestions for
  const lastSuggestedCommits = ref<string>("")

  // Get non-null suggestions
  const suggestions = computed(() => {
    return state.suggestions.filter(s => s !== null) as BranchSuggestion[]
  })

  // Progress tracking
  const completedSuggestionCount = computed(() =>
    state.suggestions.filter(s => s !== null).length,
  )

  const progress = computed(() =>
    state.totalExpected > 0 ? (completedSuggestionCount.value / state.totalExpected) * 100 : 0,
  )

  const hasAnySuggestion = computed(() =>
    suggestions.value.length > 0,
  )

  const isComplete = computed(() =>
    !state.isGenerating && completedSuggestionCount.value === state.totalExpected,
  )

  // Cancel ongoing generation
  const cancel = () => {
    // Increment generation ID to invalidate current operation
    currentGenerationId++
    state.isGenerating = false
    // Don't clear suggestions - keep them available
  }

  // Reset state
  const resetState = () => {
    state.isGenerating = false
    state.suggestions = [null, null]
    state.error = null
    state.totalExpected = 2
  }

  // Generate suggestions using streaming
  async function generateSuggestions() {
    const params: SuggestBranchNameParams = {
      repositoryPath,
      branchPrefix,
      commits: commits.value.map(c => ({
        hash: c.originalHash, // Use original hash, not synced hash
        message: c.message,
      })),
    }

    // Start new generation
    activeGenerationId = ++currentGenerationId
    const thisGenerationId = activeGenerationId

    // Mark as generating immediately
    state.isGenerating = true
    state.error = null

    // Try streaming first
    try {
      // Create channel for streaming progress updates
      const channel = new Channel<SuggestionProgress>()

      channel.onmessage = async (progress: SuggestionProgress) => {
        // Ignore messages from cancelled generations
        if (thisGenerationId !== activeGenerationId) {
          return
        }

        switch (progress.type) {
          case "Started": {
            state.totalExpected = progress.data.total
            state.suggestions = new Array(progress.data.total).fill(null)
            break
          }

          case "SuggestionReady": {
            const { suggestion, index } = progress.data
            if (index < state.suggestions.length) {
              state.suggestions[index] = suggestion
            }
            break
          }

          case "Completed": {
            state.isGenerating = false
            clearAIError() // Clear any previous errors on success
            break
          }

          case "Cancelled": {
            // Generation was cancelled by backend (e.g., another request is running)
            state.isGenerating = false
            // Don't clear suggestions - this might be due to backend lock contention
            break
          }

          case "Error": {
            state.error = progress.data.message
            state.isGenerating = false
            notifyError("Branch Name Generation Failed", progress.data.message)
            break
          }

          case "ModelDownloadInProgress": {
            // Model needs to be downloaded - trigger the download flow via reactive state
            state.isGenerating = false
            modelState.requestDownload({
              name: progress.data.model_name,
              size: progress.data.model_size,
            })
            break
          }
        }
      }

      // Start the streaming generation
      const result = await commands.suggestBranchNameStream(params, channel)

      if (result.status === "error") {
        throw new Error(result.error)
      }
    }
    catch (error) {
      state.error = error instanceof Error ? error.message : "Unknown error occurred"
      state.isGenerating = false
      setAIError(error instanceof Error ? error : new Error("Failed to generate suggestions"))
      notifyError("Branch Name Generation Failed", error)
    }
  }

  // Watch for activation and generate suggestions
  watch(isActive, (active) => {
    if (active) {
      // Check if commits have changed since last generation
      const currentCommitHashes = commits.value.map(c => c.originalHash).join(",")

      if (currentCommitHashes !== lastSuggestedCommits.value || suggestions.value.length === 0) {
        // Generate suggestions for new commits or if no suggestions exist
        generateSuggestions()
        lastSuggestedCommits.value = currentCommitHashes
      }
    }
    else {
      // Cancel any ongoing generation when deactivated
      cancel()
      // Don't clear suggestions - keep them for when form reopens
    }
  })

  // Watch for when AI is enabled
  watch(aiEnabled, (enabled) => {
    if (isActive.value && enabled) {
      const currentCommitHashes = commits.value.map(c => c.originalHash).join(",")
      if (lastSuggestedCommits.value !== currentCommitHashes || suggestions.value.length === 0) {
        generateSuggestions()
        lastSuggestedCommits.value = currentCommitHashes
      }
    }
  })

  // Watch for selection changes to regenerate suggestions
  watch(commits, () => {
    // Only regenerate if active
    if (isActive.value && commits.value.length > 0) {
      const currentCommitHashes = commits.value.map(c => c.originalHash).join(",")
      lastSuggestedCommits.value = currentCommitHashes
      generateSuggestions()
    }
  }, { deep: true })

  return {
    // Main API
    suggestions,
    isGenerating: computed(() => state.isGenerating),
    loadingProgress: progress,
    generateSuggestions,

    // Additional utilities
    cancel,
    resetState,
    hasAnySuggestion,
    completedSuggestionCount,
    isComplete,
  }
}