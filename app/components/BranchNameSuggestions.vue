<template>
  <div class="space-y-2">
    <!-- Show suggestions (with progressive loading support) -->
    <div v-if="suggestions.length > 0 || isLoading">
      <div v-if="suggestions.length > 0" class="flex items-center justify-between">
        <p class="text-xs text-muted flex items-center gap-1">
          <UIcon name="i-lucide-sparkles" class="size-3" />
          Suggestions:
        </p>
        <!-- Progress indicator when loading -->
        <div v-if="isLoading && (loadingProgress ?? 0) > 0" class="text-xs text-muted">
          {{ Math.round(loadingProgress ?? 0) }}%
        </div>
      </div>

      <div class="flex flex-wrap gap-2">
        <!-- Existing suggestions -->
        <UButton
          v-for="(suggestion, idx) in suggestions"
          :key="suggestion.name"
          size="xs"
          variant="soft"
          :color="idx === 0 ? 'primary' : 'neutral'"
          @click="$emit('select', suggestion.name)"
        >
          <span class="flex items-center gap-1">
            <span>{{ suggestion.name }}</span>
            <span v-if="suggestion.confidence > 0.8" class="text-[10px] opacity-60">
              {{ Math.round(suggestion.confidence * 100) }}%
            </span>
          </span>
        </UButton>

        <!-- Loading placeholders for pending suggestions -->
        <div
          v-for="n in pendingSlots"
          :key="`loading-${n}`"
          class="inline-flex items-center justify-center px-2 py-1 text-xs font-medium rounded-md bg-default border border-default animate-pulse"
        >
          <UIcon name="i-lucide-sparkles" class="size-3 text-muted animate-spin" />
          <span class="ml-1 text-muted">Generating...</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script lang="ts" setup>
import type { BranchSuggestion } from "~/utils/bindings"

const props = defineProps<{
  suggestions: BranchSuggestion[]
  isLoading?: boolean
  loadingProgress?: number
}>()

defineEmits<{
  select: [name: string]
}>()

// Calculate how many loading slots to show
const pendingSlots = computed(() => {
  if (!props.isLoading) return 0

  // Show loading slots for suggestions that haven't arrived yet
  // Assume we're expecting 2 suggestions total
  const expectedTotal = 2
  const currentCount = props.suggestions.length
  return Math.max(0, expectedTotal - currentCount)
})
</script>