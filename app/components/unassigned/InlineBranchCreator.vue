<template>
  <BaseInlineInput
    ref="baseInputRef"
    v-model="branchName"
    :is-active="isActive"
    :placeholder="`Branch name for ${selectedCommits.length} ${selectedCommits.length === 1 ? 'commit' : 'commits'}`"
    :validation-state="validationState"
    :is-valid="isValid"
    :dialog-title="dialogTitle"
    :dialog-description="dialogDescription"
    :portal-target="portalTarget"
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
        v-if="aiEnabled || allSuggestions.length > 0 || isGenerating"
        :suggestions="allSuggestions"
        :is-loading="isGenerating"
        :loading-progress="loadingProgress"
        @select="branchName = $event"
      />
    </template>
  </BaseInlineInput>
</template>

<script lang="ts" setup>
import type { Commit } from "~/utils/bindings"
import BranchNameSuggestions from "~/components/BranchNameSuggestions.vue"
// BaseInlineInput is auto-imported from shared-ui layer
// notifyError is auto-imported from shared-ui layer
// AI composables are auto-imported from ai layer
// Branch composables are now auto-imported

const props = withDefaults(defineProps<{
  selectedCommits: Commit[]
  isActive: boolean
  dialogTitle?: string
  dialogDescription?: string
  portalTarget?: string
}>(), {
  dialogTitle: "Create Branch",
  dialogDescription: "Create a new branch from selected commits",
  portalTarget: "inline-branch-creator-portal",
})

const emit = defineEmits<{
  cancel: []
  success: []
}>()

// Use repository injection
const { selectedProject, effectiveBranchPrefix } = useRepository()
const { syncBranches } = useBranchSync()

// State
const branchName = ref("")
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
const { isDownloading, aiEnabled, aiStatus, aiError, toggleAI } = useAIToggle()
const { isCreating, createBranch } = useBranchCreation()

// Branch suggestions with AI
const { suggestions: allSuggestions, isGenerating, loadingProgress } = useBranchSuggestions({
  repositoryPath: selectedProject.value?.path || "",
  branchPrefix: effectiveBranchPrefix.value,
  commits: computed(() => props.selectedCommits),
  isActive: computed(() => props.isActive),
})

// Handle activation to reset auto-population
watch(() => props.isActive, (active) => {
  if (active) {
    // Reset auto-population flag when form is activated
    hasAutoPopulated.value = false
  }
})

// Auto-populate with first suggestion when available
watch(
  [() => props.isActive, hasAutoPopulated, branchName, () => allSuggestions.value[0]],
  ([isActive, hasAutoPop, name, firstSuggestion]) => {
    // Only proceed if form is active and we haven't auto-populated yet
    if (!isActive || hasAutoPop || name !== "") {
      return
    }

    // Check if we have suggestions
    if (firstSuggestion) {
      branchName.value = firstSuggestion.name
      hasAutoPopulated.value = true

      // Select all text for easy override
      nextTick(() => {
        baseInputRef.value?.selectText()
      })
    }
  },
)

// Create branch handler
async function handleCreateBranch() {
  if (!isValid.value || isCreating.value) {
    return
  }

  const commitIds = props.selectedCommits.map(c => c.originalHash)
  // save the branch name before clearing
  const effectiveBranchName = sanitizedBranchName.value
  const success = await createBranch({
    repositoryPath: selectedProject.value?.path || "",
    branchName: effectiveBranchName,
    commitIds: commitIds,
  })

  if (!success) {
    return
  }

  branchName.value = ""
  emit("success")
  useToast().add({
    title: "Success",
    description: `Branch "${effectiveBranchName}" created successfully`,
    color: "success",
  })
  await syncBranches({
    targetBranchName: effectiveBranchName,
    autoExpand: true,
    autoScroll: true,
  })
}

function cancel() {
  branchName.value = ""
  emit("cancel")
}
</script>