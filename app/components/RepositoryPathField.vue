<template>
  <UFormField
    label="Repository Path"
    name="repo-path"
    :error="pathValidation.error"
  >
    <UButtonGroup class="flex">
      <USelect
        v-model="repositoryPath"
        :disabled="disabled"
        :items="recentPaths"
        :loading="isValidatingPath"
        class="flex-1"
        creatable
        placeholder="Select or enter repository path..."
        searchable
        @update:model-value="onRepositoryPathChange"
      />
      <UButton
        :disabled="disabled"
        icon="i-lucide-folder-search"
        variant="outline"
        @click="browseRepository"
      >
        Browse
      </UButton>
    </UButtonGroup>
  </UFormField>
</template>

<script lang="ts" setup>
import type { PathValidation } from "~/composables/repositoryPath"

interface Props {
  disabled?: boolean
}

defineProps<Props>()

const emit = defineEmits<{
  "update:modelValue": [value: string]
  "update:validation": [validation: PathValidation]
}>()

const {
  repositoryPath,
  recentPaths,
  pathValidation,
  isValidatingPath,
  onRepositoryPathChange,
  browseRepository,
} = useRepositoryPath()

// Emit changes to parent
watch(repositoryPath, (value) => {
  emit("update:modelValue", value)
})

watch(pathValidation, (validation) => {
  emit("update:validation", validation)
}, { deep: true })
</script>