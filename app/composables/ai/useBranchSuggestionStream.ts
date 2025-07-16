import { Channel } from "@tauri-apps/api/core"
import { commands } from "~/utils/bindings"
import type { BranchSuggestion, SuggestionProgress, SuggestBranchNameParams } from "~/utils/bindings"
// notifyError is auto-imported from shared-ui layer
import { useModelStore } from "~/stores/modelState"

export interface SuggestionStreamState {
  isGenerating: boolean
  suggestions: (BranchSuggestion | null)[]
  error: string | null
  totalExpected: number
}

export function useBranchSuggestionStream() {
  const modelStore = useModelStore()

  const state = reactive<SuggestionStreamState>({
    isGenerating: false,
    suggestions: [null, null], // Initialize with 2 null slots
    error: null,
    totalExpected: 2,
  })

  // Track current generation to handle cancellation
  let currentGenerationId = 0
  let activeGenerationId = 0

  const resetState = () => {
    state.isGenerating = false
    state.suggestions = [null, null]
    state.error = null
    state.totalExpected = 2
  }

  const cancel = () => {
    // Increment generation ID to invalidate current operation
    currentGenerationId++
    state.isGenerating = false
    // Don't clear suggestions - keep them available
  }

  const generateSuggestionsStream = async (params: SuggestBranchNameParams): Promise<void> => {
    // Start new generation
    activeGenerationId = ++currentGenerationId
    const thisGenerationId = activeGenerationId

    // Mark as generating immediately
    state.isGenerating = true
    state.error = null

    // Don't clear suggestions immediately - wait for new ones to arrive

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
            modelStore.requestDownload({
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
        state.error = result.error
        state.isGenerating = false
        notifyError("Branch Name Generation Failed", result.error)
      }
    }
    catch (error) {
      state.error = error instanceof Error ? error.message : "Unknown error occurred"
      state.isGenerating = false
      notifyError("Branch Name Generation Failed", error)
    }
  }

  const hasAnySuggestion = computed(() =>
    state.suggestions.some(s => s !== null),
  )

  const completedSuggestionCount = computed(() =>
    state.suggestions.filter(s => s !== null).length,
  )

  const isComplete = computed(() =>
    !state.isGenerating && completedSuggestionCount.value === state.totalExpected,
  )

  const progress = computed(() =>
    state.totalExpected > 0 ? (completedSuggestionCount.value / state.totalExpected) * 100 : 0,
  )

  return {
    state: readonly(state),
    generateSuggestionsStream,
    resetState,
    cancel,
    hasAnySuggestion,
    completedSuggestionCount,
    isComplete,
    progress,
  }
}