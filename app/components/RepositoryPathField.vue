<template>
  <UFormField
    label="Repository Path"
    name="repo-path"
    :error="store.pathValidation.error"
  >
    <UButtonGroup class="flex">
      <USelect
        :model-value="store.repositoryPath"
        :disabled="disabled"
        :items="store.recentPaths"
        :loading="store.isValidatingPath"
        class="flex-1"
        creatable
        placeholder="Select or enter repository path..."
        searchable
        @update:model-value="store.setRepositoryPath"
      />
      <UButton
        :disabled="disabled"
        icon="i-lucide-folder-search"
        variant="outline"
        @click="store.browseRepository"
      >
        Browse
      </UButton>
    </UButtonGroup>
  </UFormField>
</template>

<script lang="ts" setup>
import { useRepositoryStore } from "~/stores/repository"

interface Props {
  disabled?: boolean
}

defineProps<Props>()

// Use the repository store
const store = useRepositoryStore()
</script>