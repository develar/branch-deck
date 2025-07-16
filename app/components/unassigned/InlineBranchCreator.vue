<template>
  <BaseInlineInput
    v-if="isActive"
    ref="baseInputRef"
    v-model="branchName"
    :is-active="isActive"
    :placeholder="`Branch name for ${selectedCommits.length} ${selectedCommits.length === 1 ? 'commit' : 'commits'}`"
    :validation-state="validationState"
    :is-valid="isValid"
    primary-button-text="Create"
    @submit="handleCreateBranch"
    @cancel="cancel"
  >
    <template #leading-icon>
      <UIcon name="i-lucide-git-branch" class="size-4 text-muted" />
    </template>

    <template #extra-actions>
      <div class="flex items-center gap-3">
        <!-- AI indicator icon with integrated help -->
        <UPopover v-if="!isDownloading" mode="hover">
          <UIcon
            name="i-lucide-sparkles"
            :class="[
              'size-3.5 cursor-pointer transition-all',
              aiStatus.iconClass
            ]"
            @click="toggleAI"
          />
          <template #content>
            <div class="p-3 space-y-2 text-xs max-w-xs">
              <!-- Error state -->
              <template v-if="aiError">
                <div class="flex items-center gap-2">
                  <UIcon name="i-lucide-alert-triangle" class="size-4 text-error" />
                  <p class="font-semibold text-error">AI Malfunction</p>
                </div>
                <div class="space-y-2 mt-2">
                  <div class="p-2 bg-error/10 border border-error/20 rounded-md">
                    <p class="font-medium text-error mb-1">Error:</p>
                    <p class="text-toned break-words">{{ aiError.message }}</p>
                  </div>
                  <details class="cursor-pointer">
                    <summary class="text-muted hover:text-highlighted">View full details</summary>
                    <pre class="mt-2 p-2 bg-subtle rounded text-[10px] overflow-x-auto whitespace-pre-wrap break-words">{{ aiError.details }}</pre>
                  </details>
                  <div class="flex items-center justify-between text-[10px] text-muted">
                    <span>{{ new Date(aiError.timestamp).toLocaleTimeString() }}</span>
                    <a
                      href="https://github.com/develar/branch-deck/issues"
                      target="_blank"
                      class="text-primary hover:underline flex items-center gap-1"
                    >
                      <UIcon name="i-lucide-external-link" class="size-2.5" />
                      Report issue
                    </a>
                  </div>
                </div>
                <div class="pt-2 border-t border-default">
                  <p class="text-muted mb-2">Click the icon to retry or disable AI</p>
                </div>
              </template>
              <!-- Normal state -->
              <template v-else>
                <p class="font-semibold text-highlighted">
                  AI {{ aiEnabled ? 'Enabled' : 'Disabled' }}
                  <span class="text-xs text-muted font-normal ml-1">(click to {{ aiEnabled ? 'disable' : 'enable' }})</span>
                </p>
                <p class="text-toned">
                  AI analyzes only commit metadata to suggest branch names:
                </p>
                <ul class="list-disc list-inside space-y-1 text-muted ml-2">
                  <li>Commit messages (title and body)</li>
                  <li>Modified file names and their status</li>
                </ul>
                <p class="text-toned">
                  This is equivalent to: <code class="text-[10px] bg-subtle px-1 py-0.5 rounded">git log --name-status</code>
                </p>
                <div class="pt-2 border-t border-default">
                  <p class="text-success flex items-center gap-1">
                    <UIcon name="i-lucide-shield-check" class="size-3" />
                    <span class="font-medium">100% Local & Private</span>
                  </p>
                  <p class="text-toned mt-1">
                    Uses the <a href="https://huggingface.co/Qwen/Qwen3-1.7B-GGUF" target="_blank" class="text-primary hover:underline">Qwen3-1.7B</a> model running entirely on your machine. No data is sent to any external service.
                  </p>
                </div>
              </template>
            </div>
          </template>
        </UPopover>
        <UTooltip v-else text="Downloading AI model...">
          <UIcon
            name="i-lucide-sparkles"
            class="size-3.5 text-primary animate-spin"
          />
        </UTooltip>

        <div class="w-px h-4 bg-border-default" />
      </div>
    </template>

    <template #validation-message>
      {{ validationState.message }}
    </template>

    <template #after-controls>
      <!-- Suggestions (compact) -->
      <BranchNameSuggestions
        v-if="allSuggestions.length > 0 || (aiPreference !== false && modelStatus?.available)"
        :suggestions="allSuggestions"
        :is-loading="suggestionStream.state.isGenerating"
        :loading-progress="suggestionStream.progress.value"
        @select="branchName = $event"
      />
    </template>
  </BaseInlineInput>
</template>

<script lang="ts" setup>
import type { CommitDetail, BranchSuggestion } from "~/utils/bindings"
import BranchNameSuggestions from "~/components/BranchNameSuggestions.vue"
// BaseInlineInput is auto-imported from shared-ui layer
// notifyError is auto-imported from shared-ui layer
import { useBranchSuggestionStream } from "~/composables/ai/useBranchSuggestionStream"
import { useAIToggle } from "~/composables/ai/useAIToggle"
import { useBranchNameValidation } from "~/composables/branch/useBranchNameValidation"
import { useBranchCreation } from "~/composables/branch/useBranchCreation"
import { commands } from "~/utils/bindings"

const props = defineProps<{
  selectedCommits: CommitDetail[]
  repositoryPath: string
  branchPrefix: string
  isActive: boolean
}>()

const emit = defineEmits<{
  cancel: []
  created: [branchName: string]
}>()

// State
const branchName = ref("")
const suggestions = ref<BranchSuggestion[]>([]) // Keep for backward compatibility
const hasAutoPopulated = ref(false)

// Template refs
const baseInputRef = useTemplateRef("baseInputRef")

const { sanitizedBranchName, validationState: rawValidationState, isValid } = useBranchNameValidation(branchName)

// Transform validation state to match BaseInlineInput's expected type
const validationState = computed(() => ({
  color: rawValidationState.value.color || "primary" as const,
  message: rawValidationState.value.message,
  textClass: rawValidationState.value.textClass,
}))
const { modelStatus, isDownloading, aiPreference, aiEnabled, aiStatus, aiError, toggleAI, setAIError, clearAIError } = useAIToggle()
const { isCreating, createBranch } = useBranchCreation()

// Streaming suggestions
const suggestionStream = useBranchSuggestionStream()

// Track last commits we generated suggestions for
const lastSuggestedCommits = ref<string>("")

// Combine streaming and fallback suggestions
const allSuggestions = computed(() => {
  const streamSuggestions = suggestionStream.state.suggestions.filter(s => s !== null) as BranchSuggestion[]
  return streamSuggestions.length > 0 ? streamSuggestions : suggestions.value
})

// Handle activation and deactivation
watch(() => props.isActive, (active) => {
  if (active) {
    // Reset auto-population flag when form is activated
    hasAutoPopulated.value = false

    // Check if commits have changed since last generation
    const currentCommitHashes = props.selectedCommits.map(c => c.originalHash).join(",")

    if (currentCommitHashes !== lastSuggestedCommits.value || suggestionStream.state.suggestions.every(s => s === null)) {
      // Generate suggestions for new commits or if no suggestions exist
      generateSuggestions()
      lastSuggestedCommits.value = currentCommitHashes
    }
  }
  else {
    // Cancel any ongoing generation when form is closed
    suggestionStream.cancel()
    // Don't clear suggestions - keep them for when form reopens
  }
})

// Watch for selection changes to regenerate suggestions
watch(() => props.selectedCommits, () => {
  // Only regenerate if the form is active
  if (props.isActive && props.selectedCommits.length > 0) {
    const currentCommitHashes = props.selectedCommits.map(c => c.originalHash).join(",")
    lastSuggestedCommits.value = currentCommitHashes
    generateSuggestions()
  }
}, { deep: true })

// Auto-populate with first suggestion when available
watchEffect(() => {
  // Only proceed if form is active and we haven't auto-populated yet
  if (!props.isActive || hasAutoPopulated.value || branchName.value !== "") {
    return
  }

  // Check if we have suggestions
  const firstSuggestion = allSuggestions.value[0]
  if (firstSuggestion) {
    branchName.value = firstSuggestion.name
    hasAutoPopulated.value = true

    // Select all text for easy override
    nextTick(() => {
      baseInputRef.value?.selectText()
    })
  }
})

// Generate suggestions from backend with streaming
async function generateSuggestions() {
  const params = {
    repositoryPath: props.repositoryPath,
    branchPrefix: props.branchPrefix,
    commits: props.selectedCommits.map(c => ({
      hash: c.originalHash, // Use original hash, not synced hash
      message: c.message,
    })),
  }

  // Try streaming first
  try {
    await suggestionStream.generateSuggestionsStream(params)
    clearAIError() // Clear any previous errors on success
    return // Success with streaming
  }
  catch (error) {
    // Set error state for UI
    setAIError(error instanceof Error ? error : new Error("Failed to generate suggestions"))
    // Fallback to sync mode if streaming fails
  }

  // Fallback to sync mode if streaming fails
  try {
    const result = await commands.suggestBranchName(params)

    if (result.status === "ok") {
      suggestions.value = result.data
      clearAIError() // Clear any previous errors on success
    }
    else {
      // Check if it's a model not downloaded error
      if (result.error.includes("Model not downloaded")) {
        // Don't report as error - the model download handler will show the toast
        suggestions.value = []
      }
      else {
        setAIError(result.error) // Set error state for UI
        notifyError("Branch Name Suggestions Failed", result.error)
        suggestions.value = []
      }
    }
  }
  catch (error) {
    setAIError(error instanceof Error ? error : new Error(String(error)))
    notifyError("Branch Name Suggestions Failed", error)
    suggestions.value = []
  }
}

// Create branch handler
async function handleCreateBranch() {
  if (!isValid.value || isCreating.value) return

  const commitIds = props.selectedCommits.map(c => c.originalHash)
  const success = await createBranch({
    repositoryPath: props.repositoryPath,
    branchName: sanitizedBranchName.value,
    commitIds: commitIds,
  })

  if (success) {
    emit("created", sanitizedBranchName.value)
    branchName.value = ""
  }
}

// Cancel creation
function cancel() {
  branchName.value = ""
  emit("cancel")
}
</script>