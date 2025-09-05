<template>
  <UTooltip text="Your personal prefix (e.g., username) prepended to all branch names">
    <UFieldGroup>
      <UInput
        v-model="appSettingsStore.globalUserBranchPrefix"
        :disabled="disabled"
        :color="getInputColor()"
        :placeholder="placeholder"
        size="sm"
        class="w-28"
      />
      <BranchPrefixHelp
        :configured="configured"
        :disabled="!!disabled"
      />
    </UFieldGroup>
  </UTooltip>
</template>

<script lang="ts" setup>
const props = defineProps<{
  disabled?: boolean
}>()

// Use the repository injection and app settings store
const { gitProvidedBranchPrefix, isLoadingBranchPrefix, effectiveBranchPrefix } = useRepository()
const appSettingsStore = useAppSettingsStore()

// Compute placeholder - show git prefix when available and user hasn't entered anything
const placeholder = computed(() => {
  if (gitProvidedBranchPrefix.value.status === "ok" && gitProvidedBranchPrefix.value.data) {
    return gitProvidedBranchPrefix.value.data
  }
  return "Set prefix..."
})

// Compute configured state from repository
// Configured if we have any effective branch prefix
const configured = computed(() =>
  isLoadingBranchPrefix.value // Still loading, don't show warning
  || effectiveBranchPrefix.value !== "",
)

const getInputColor = () => {
  // Don't show warning color when disabled
  if (props.disabled) {
    return "primary"
  }
  if (!configured.value) {
    return "warning"
  }
  return "primary"
}
</script>
