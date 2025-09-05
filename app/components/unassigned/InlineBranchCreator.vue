<template>
  <InlineInputDialog
    ref="inlineInputDialogRef"
    v-model="branchName"
    :open="isActive"
    :placeholder="`Branch name for ${selectedCommits.length} ${selectedCommits.length === 1 ? 'commit' : 'commits'}`"
    :can-submit="isValid && !isCreating"
    :title="dialogTitle"
    :description="dialogDescription"
    :portal-target="portalTarget"
    :validation-message="validationState.message"
    :validation-color="validationState.color || 'primary'"
    :validation-text-class="validationState.textClass"
    submit-text="Create"
    @submit="handleCreateBranch"
    @cancel="cancel"
  >
    <template #leading-icon>
      <UIcon name="i-lucide-git-branch" class="size-4 text-muted" />
    </template>

    <template #extra-actions>
      <div class="flex items-center gap-3">
        <AIStatusIndicator />
        <div class="w-px h-4 bg-border-default" />
      </div>
    </template>

    <template #after-controls>
      <!-- Suggestions (compact) - handles both initial prompt and suggestions -->
      <BranchNameSuggestions
        :repository-path="selectedProject?.path || ''"
        :branch-prefix="effectiveBranchPrefix"
        :commits="selectedCommits"
        :is-active="isActive"
        @select="handleSuggestionSelect"
      />
    </template>
  </InlineInputDialog>
</template>

<script lang="ts" setup>
import type { Commit } from "~/utils/bindings"

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
const inlineInputDialogRef = useTemplateRef("inlineInputDialogRef")

const { sanitizedBranchName, validationState: rawValidationState, isValid } = useBranchNameValidation(branchName)

// Transform validation state to match BaseInlineInput's expected type
const validationState = computed(() => ({
  color: rawValidationState.value.color || "primary" as const,
  message: rawValidationState.value.message,
  textClass: rawValidationState.value.textClass,
}))
const { isCreating, createBranch } = useBranchCreation()

// Handle activation to reset auto-population
watch(() => props.isActive, (active) => {
  if (active) {
    // Reset auto-population flag when form is activated
    hasAutoPopulated.value = false
  }
})

// Handle suggestion selection (both manual clicks and auto-population)
function handleSuggestionSelect(name: string, isAuto = false) {
  // For auto-population, check guards
  if (isAuto) {
    // Only proceed if form is active and we haven't auto-populated yet
    if (!props.isActive || hasAutoPopulated.value || branchName.value !== "") {
      return
    }
    hasAutoPopulated.value = true
  }

  branchName.value = name

  // Select all text for easy override and ensure focus
  nextTick(() => {
    inlineInputDialogRef.value?.selectText()
    inlineInputDialogRef.value?.focusInput()
  })
}

// Create branch handler
async function handleCreateBranch() {
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
