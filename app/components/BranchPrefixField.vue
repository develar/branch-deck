<template>
  <UFormField label="Branch Prefix" name="branch-prefix">
    <UButtonGroup>
      <UInput
        v-model="store.branchPrefix"
        :disabled="disabled"
        class="flex-1"
        placeholder="Enter branch prefix..."
      />
      <BranchPrefixHelp
        :configured="configured"
        :disabled="!!disabled"
      />
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

// Compute configured state from store
const configured = computed(() =>
  store.gitProvidedBranchPrefix.status === "ok" && store.gitProvidedBranchPrefix.data !== "",
)
</script>