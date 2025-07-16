<template>
  <BaseInlineInput
    v-model="issueReference"
    :is-active="isActive"
    :placeholder="`Issue reference for ${branchName} (${commitCount} ${commitCount === 1 ? 'commit' : 'commits'})`"
    :validation-state="validationState"
    :is-valid="isValid"
    :dialog-title="dialogTitle"
    :dialog-description="dialogDescription"
    :portal-target="portalTarget"
    primary-button-text="Add"
    data-testid="inline-issue-input"
    @submit="handleSubmit"
    @cancel="cancel"
  >
    <template #leading-icon>
      <UIcon name="i-lucide-tag" class="size-4 text-muted" />
    </template>

    <template #help-text>
      The issue reference will be added after the branch prefix in commit messages.
    </template>
  </BaseInlineInput>
</template>

<script lang="ts" setup>
// BaseInlineInput is auto-imported from shared-ui layer

defineProps<{
  branchName: string
  commitCount: number
  dialogTitle?: string
  dialogDescription?: string
  portalTarget?: string
  isActive?: boolean
}>()

const emit = defineEmits<{
  cancel: []
  submit: [issueReference: string]
}>()

// State
const issueReference = ref("")
const showError = ref(false)

// Reset error when typing
watch(issueReference, () => {
  showError.value = false
})

// Validation
const validationState = computed(() => {
  if (!issueReference.value || !showError.value) {
    return {
      color: "primary" as const,
      message: "",
      textClass: "",
    }
  }

  // Allow flexible issue formats: ABC-123, ISSUE-456, etc.
  const pattern = /^[A-Z]+-\d+$/
  if (!pattern.test(issueReference.value)) {
    return {
      color: "error" as const,
      message: "Issue reference must be in format like ABC-123",
      textClass: "text-error",
    }
  }

  return {
    color: "primary" as const,
    message: "",
    textClass: "",
  }
})

const isValid = computed(() => {
  if (!issueReference.value) {
    return false
  }

  const pattern = /^[A-Z]+-\d+$/
  return pattern.test(issueReference.value)
})

// Handle submission
function handleSubmit() {
  if (!isValid.value) {
    showError.value = true
    return
  }

  emit("submit", issueReference.value)
}

// Cancel action
function cancel() {
  emit("cancel")
}
</script>