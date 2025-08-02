import { Channel } from "@tauri-apps/api/core"
import type { Commit, BranchSuggestion, SuggestBranchNameParams, SuggestionProgress } from "~/utils/bindings"
import { commands } from "~/utils/bindings"
import { useModelState } from "./modelProvider"
import { useThrottleFn } from "@vueuse/core"
// notifyError is auto-imported from shared-ui layer

interface UseBranchSuggestionsOptions {
  repositoryPath: string
  branchPrefix: string
  commits: Ref<Commit[]>
  isActive: Ref<boolean>
}

// Remove SuggestionState interface as we'll use individual refs

/**
 * Composable for generating AI-powered branch name suggestions based on commits.
 * Handles streaming suggestions with real-time progress updates and automatic regeneration.
 *
 * Features:
 * - Streaming suggestion generation with progress tracking
 * - Automatic regeneration on commit selection changes
 * - Graceful handling of model downloads and errors
 * - Cancellation support for ongoing generations
 *
 * @param {UseBranchSuggestionsOptions} options - Configuration options:
 *   - repositoryPath: Path to the git repository
 *   - branchPrefix: Prefix to apply to generated branch names
 *   - commits: Reactive array of commits to analyze
 *   - isActive: Whether the suggestion UI is currently active
 *
 * @returns {Promise<Object>} Object containing:
 *   - suggestions: Computed array of non-null suggestions
 *   - isGenerating: Readonly ref indicating generation in progress
 *   - loadingProgress: Computed progress percentage (0-100)
 *
 * @example
 * const { suggestions, isGenerating } = await useBranchSuggestions({
 *   repositoryPath: '/path/to/repo',
 *   branchPrefix: 'feature/',
 *   commits: selectedCommits,
 *   isActive: isFormOpen
 * })
 */
export async function useBranchSuggestions(options: UseBranchSuggestionsOptions) {
  const { repositoryPath, branchPrefix, commits, isActive } = options

  const modelState = useModelState()
  // AI state management
  const { aiMode, setAIError, clearAIError } = await useAIToggle()

  // State for streaming suggestions
  const isGenerating = ref(false)
  const suggestions = ref<(BranchSuggestion | null)[]>([null, null]) // Initialize with 2 null slots
  const error = ref<string | null>(null)
  const totalExpected = ref(2)

  // Track current generation to handle cancellation
  let currentGenerationId = 0
  let activeGenerationId = 0

  // Track last commits we generated suggestions for
  const lastSuggestedCommits = ref<string>("")

  // Computed commit hash string for efficient comparison
  const currentCommitHashes = computed(() =>
    commits.value.map(c => c.originalHash).join(","),
  )

  // Get non-null suggestions
  const nonNullSuggestions = computed(() => {
    return suggestions.value.filter(s => s !== null) as BranchSuggestion[]
  })

  // Progress tracking
  const completedSuggestionCount = computed(() =>
    suggestions.value.filter(s => s !== null).length,
  )

  const progress = computed(() =>
    totalExpected.value > 0 ? (completedSuggestionCount.value / totalExpected.value) * 100 : 0,
  )

  // Cancel ongoing generation
  const cancel = () => {
    // Increment generation ID to invalidate current operation
    currentGenerationId++
    isGenerating.value = false
    // Don't clear suggestions - keep them available
  }

  // Generate suggestions using streaming
  async function generateSuggestions() {
    // Don't start new generation if download is in progress
    // This prevents repeated API calls during download
    if (modelState.isDownloading.value) {
      return
    }

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
    isGenerating.value = true
    error.value = null

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
            totalExpected.value = progress.data.total
            suggestions.value = new Array(progress.data.total).fill(null)
            break
          }

          case "SuggestionReady": {
            const { suggestion, index } = progress.data
            if (index < suggestions.value.length) {
              suggestions.value[index] = markRaw(suggestion)
            }
            break
          }

          case "Completed": {
            isGenerating.value = false
            clearAIError() // Clear any previous errors on success
            break
          }

          case "Cancelled": {
            // Generation was cancelled by backend (e.g., another request is running)
            isGenerating.value = false
            // Don't clear suggestions - this might be due to backend lock contention
            break
          }

          case "Error": {
            error.value = progress.data.message
            isGenerating.value = false
            notifyError("Branch Name Generation Failed", progress.data.message)
            break
          }

          case "ModelDownloadInProgress": {
            // Model needs to be downloaded - trigger the download flow via reactive state
            isGenerating.value = false
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
    catch (err) {
      error.value = err instanceof Error ? err.message : "Unknown error occurred"
      isGenerating.value = false
      setAIError(err instanceof Error ? err : new Error("Failed to generate suggestions"))
      notifyError("Branch Name Generation Failed", err)
    }
  }

  /**
   * Determines if new suggestions should be generated based on current state.
   * Checks for active UI, available commits, AI enablement, and commit changes.
   *
   * @returns {boolean} True if suggestions should be generated
   * @internal
   */
  function shouldGenerateSuggestions() {
    const should = isActive.value
      && commits.value.length > 0
      && aiMode.value === "enabled"
      && (currentCommitHashes.value !== lastSuggestedCommits.value || nonNullSuggestions.value.length === 0)

    return should
  }

  /**
   * Triggers suggestion generation if conditions are met.
   * Updates the last suggested commits hash to prevent duplicate generations.
   *
   * @internal Called by watchers when state changes
   */
  function triggerSuggestionGeneration() {
    if (shouldGenerateSuggestions()) {
      generateSuggestions()
      lastSuggestedCommits.value = currentCommitHashes.value
    }
  }

  // Create a throttled version to prevent rapid successive calls
  // Using throttle instead of debounce to get immediate execution on first call
  const SUGGESTION_THROTTLE_MS = 300
  const triggerSuggestionGenerationThrottled = useThrottleFn(
    triggerSuggestionGeneration,
    SUGGESTION_THROTTLE_MS,
    true, // trailing: execute after the throttle period
    true, // leading: execute immediately on first call
  )

  // Watch for activation and generate suggestions
  watch(isActive, (active) => {
    if (active) {
      triggerSuggestionGenerationThrottled()
    }
    else {
      // Cancel any ongoing generation when deactivated
      cancel()
      // Don't clear suggestions - keep them for when form reopens
    }
  }, { immediate: true })

  // Watch for when AI mode changes from initial/disabled to enabled
  watch(() => aiMode.value, (newMode, oldMode) => {
    if (newMode === "enabled" && oldMode !== "enabled") {
      triggerSuggestionGenerationThrottled()
    }
    else if (newMode === "disabled") {
      // Clear suggestions when AI is disabled
      suggestions.value = [null, null]
    }
  })

  // Watch for selection changes to regenerate suggestions
  watch(currentCommitHashes, () => {
    triggerSuggestionGenerationThrottled()
  })

  // Watch for model download completion to retry suggestions
  watch(() => modelState.isDownloading.value, (isDownloading, wasDownloading) => {
    // If download just completed and we should generate suggestions
    if (wasDownloading && !isDownloading && shouldGenerateSuggestions()) {
      triggerSuggestionGenerationThrottled()
    }
  })

  return {
    suggestions: nonNullSuggestions,
    isGenerating: readonly(isGenerating),
    loadingProgress: progress,
  }
}