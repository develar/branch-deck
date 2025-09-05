<template>
  <div class="ml-4 border-l-2 border-primary/50 pl-2 pr-6">
    <UAlert
      v-if="hasGenericError"
      color="error"
      variant="subtle"
    >
      <template #description>
        {{ genericError }}
      </template>
    </UAlert>

    <!-- Commit list -->
    <CommitList
      v-else-if="branch.commits && branch.commits.length > 0"
      :commits="branch.commits"
      variant="status"
      :show-file-count="false"
      :highlight-tip-commit="shouldHighlightTipCommit"
    />

    <!-- No commits found -->
    <div v-else class="text-muted text-sm">
      No commits found
    </div>
  </div>
</template>

<script lang="ts" setup>
import type { ReactiveBranch } from "~/composables/branchSyncProvider"

const props = defineProps<{
  branch: ReactiveBranch
  highlightTipCommit?: boolean
}>()

// Default to false if not provided
const shouldHighlightTipCommit = computed(() => props.highlightTipCommit || false)

// Error handling computed properties
const hasErrorDetails = computed(() => {
  return props.branch.errorDetails
})

const hasGenericError = computed(() => {
  if (!hasErrorDetails.value) {
    return false
  }
  return props.branch.errorDetails && "Generic" in props.branch.errorDetails
})

const genericError = computed(() => {
  if (!hasGenericError.value) {
    return null
  }
  return props.branch.errorDetails && "Generic" in props.branch.errorDetails
    ? props.branch.errorDetails.Generic
    : null
})
</script>