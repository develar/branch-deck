<template>
  <BaseInlineInput
    v-model="inputText"
    :is-active="isActive"
    :placeholder="`Type ${simpleName} to confirm deletion`"
    :validation-state="validationState"
    :is-valid="isValid"
    :dialog-title="dialogTitle"
    :dialog-description="dialogDescription"
    :portal-target="portalTarget"
    primary-button-text="Delete"
    data-testid="inline-delete-input"
    @submit="handleSubmit"
    @cancel="cancel"
  >
    <template #leading-icon>
      <UIcon name="i-lucide-trash-2" class="size-4 text-muted" />
    </template>

    <template #help-text>
      This action will permanently delete the archived branch. This cannot be undone.
    </template>
  </BaseInlineInput>
</template>

<script lang="ts" setup>
// BaseInlineInput is auto-imported from shared-ui layer
const props = defineProps<{
  branchName: string
  dialogTitle?: string
  dialogDescription?: string
  portalTarget?: string
  isActive?: boolean
}>()

const emit = defineEmits<{
  cancel: []
  submit: []
}>()

// State
const inputText = ref("")
const showError = ref(false)

watch(inputText, () => {
  showError.value = false
})

const simpleName = computed(() => getSimpleBranchName(props.branchName))

const isValid = computed(() => {
  return inputText.value === simpleName.value
})

const validationState = computed(() => {
  if (!showError.value) {
    return { color: "primary" as const, message: "", textClass: "" }
  }

  if (!isValid.value) {
    return {
      color: "error" as const,
      message: `Type the exact name: ${simpleName.value}`,
      textClass: "text-error",
    }
  }

  return { color: "primary" as const, message: "", textClass: "" }
})

function handleSubmit() {
  if (!isValid.value) {
    showError.value = true
    return
  }
  emit("submit")
}

function cancel() {
  emit("cancel")
}
</script>
