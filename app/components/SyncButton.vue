<template>
  <UTooltip v-if="!canSync && pathValidation.error" :text="pathValidation.error">
    <UButton
      data-testid="sync-button"
      :disabled="isSyncing || !canSync"
      :loading="isSyncing"
      :color="getButtonColor()"
      icon="i-lucide-refresh-cw"
      size="sm"
      @click="syncBranches()"
    >
      Sync
    </UButton>
  </UTooltip>
  <UButton
    v-else
    data-testid="sync-button"
    :disabled="isSyncing || !canSync"
    :loading="isSyncing"
    :color="getButtonColor()"
    icon="i-lucide-refresh-cw"
    size="sm"
    @click="syncBranches()"
  >
    Sync
  </UButton>
</template>

<script lang="ts" setup>
const { pathValidation, selectedProject } = useRepository()
const { syncBranches, isSyncing, syncError } = useBranchSync()

const canSync = computed(() => {
  return selectedProject.value && pathValidation.value.valid
})

const getButtonColor = () => {
  if (syncError.value) {
    return "error"
  }
  return "primary"
}
</script>