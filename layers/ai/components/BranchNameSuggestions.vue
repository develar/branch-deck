<template>
  <!-- Show suggestions header and content -->
  <div v-if="isInitial || suggestions.length > 0 || isLoading" class="space-y-2" data-testid="branch-name-suggestions">
    <!-- Header section -->
    <div class="flex items-center justify-between">
      <p class="text-xs text-muted flex items-center gap-1">
        <UIcon name="i-lucide-sparkles" class="size-3" />
        Suggestions:
      </p>
      <!-- Progress indicator when loading -->
      <div v-if="isLoading && (loadingProgress ?? 0) > 0" class="text-xs text-muted">
        {{ Math.round(loadingProgress ?? 0) }}%
      </div>
    </div>

    <!-- Suggestions content -->
    <div class="flex flex-wrap gap-2">
      <!-- Initial state buttons -->
      <template v-if="isInitial && !isLoading && suggestions.length === 0">
        <UButton
          variant="soft"
          color="neutral"
          size="xs"
          @click="aiMode = 'disabled'"
        >
          Not now
        </UButton>
        <UButton
          variant="soft"
          color="primary"
          size="xs"
          @click="aiMode = 'enabled'"
        >
          Enable AI
        </UButton>
      </template>

      <!-- Existing suggestions -->
      <template v-if="!isInitial || suggestions.length > 0 || isLoading">
        <UButton
          v-for="(suggestion, idx) in suggestions"
          :key="suggestion.name"
          size="xs"
          variant="soft"
          :color="idx === 0 ? 'primary' : 'neutral'"
          @click="$emit('select', suggestion.name)"
        >
          <span>{{ suggestion.name }}</span>
        </UButton>

        <!-- Loading placeholders for pending suggestions -->
        <div
          v-for="n in pendingSlots"
          :key="`loading-${n}`"
          class="inline-flex items-center gap-1 justify-center px-2 py-1 text-xs font-medium rounded-md bg-default border border-default animate-pulse"
        >
          <UIcon name="i-lucide-sparkles" class="size-3 text-muted animate-spin" />
          <span class="text-muted">Generating...</span>
        </div>
      </template>
    </div>
  </div>
</template>

<script lang="ts" setup>
import type { Commit } from "~/utils/bindings"

const props = defineProps<{
  repositoryPath: string
  branchPrefix: string
  commits: Commit[]
  isActive: boolean
}>()

const emit = defineEmits<{
  select: [name: string, isAuto?: boolean]
}>()

// AI state
const { aiMode, isInitial } = await useAIToggle()

// Branch suggestions with AI
const { suggestions, isGenerating: isLoading, loadingProgress } = await useBranchSuggestions({
  repositoryPath: props.repositoryPath,
  branchPrefix: props.branchPrefix,
  commits: computed(() => props.commits),
  isActive: computed(() => props.isActive),
})

// Emit first suggestion for auto-population when it changes
watch(() => suggestions.value[0], (firstSuggestion) => {
  if (firstSuggestion) {
    emit("select", firstSuggestion.name, true)
  }
}, { immediate: true })

// Calculate how many loading slots to show
const pendingSlots = computed(() => {
  if (!isLoading.value) {
    return 0
  }

  // Show loading slots for suggestions that haven't arrived yet
  // Assume we're expecting 2 suggestions total
  const expectedTotal = 2
  const currentCount = suggestions.value.length
  return Math.max(0, expectedTotal - currentCount)
})
</script>