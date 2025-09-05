<template>
  <InlineForm
    :open="isActive"
    :title="dialogTitle"
    :description="dialogDescription"
    :portal-target="portalTarget"
    :can-submit="canAmend"
    :loading="processing"
    submit-text="Amend Changes"
    @cancel="closeInline"
    @submit="onSubmit"
  >
    <!-- Uncommitted Changes Preview -->
    <UncommittedChangesCard
      :diff-data="diffData"
      :loading="loading"
      :error="error"
    />

    <template #submit-icon>
      <UIcon name="i-lucide-edit-3" class="size-3" />
    </template>
  </InlineForm>
</template>

<script lang="ts" setup>
import pDebounce from "p-debounce"
import UncommittedChangesCard from "./UncommittedChangesCard.vue"

// Access context and composables
const { branches } = useBranchSync()
const { amendChanges, diffData, diffLoading, diffError, loadUncommittedChanges } = useAmendChanges()
const inline = useInlineRowAction()
const { activeInline, portalTargetIdFor, closeInline } = inline

// Computed properties from activeInline
const branchName = computed(() => activeInline.value?.branchName || "")
const isActive = computed(() => {
  return activeInline.value?.type === "amend-changes" && !!activeInline.value?.branchName
})
const dialogTitle = computed(() => branchName.value ? `Amend Changes to ${branchName.value}` : "")
const dialogDescription = computed(() => branchName.value ? `Amend uncommitted changes to the tip commit of ${branchName.value} branch` : "")
const portalTarget = computed(() => {
  return branchName.value ? portalTargetIdFor(branchName.value) : undefined
})

// Map composable state to component's expected interface
const loading = computed(() => diffLoading.value)
const error = computed(() => diffError.value)
const processing = computed(() => branchName.value ? inline.isProcessing(branchName.value) : false)

// Submit handler: amend changes to the active branch
const onSubmit = () => {
  const branch = branches.value.find(b => b.name === branchName.value)
  if (!branch) {
    return
  }
  amendChanges(branch)
}

// Create debounced loader with immediate execution on first call
const debouncedLoad = pDebounce(loadUncommittedChanges, 100, { before: true })

// Load uncommitted changes when branch changes or component first renders
watch(branchName, (name) => {
  if (name) {
    debouncedLoad()
  }
}, { immediate: true })

// Computed properties
const canAmend = computed(() => {
  return !loading.value && !error.value && diffData.value?.hasChanges && !processing.value
})
</script>