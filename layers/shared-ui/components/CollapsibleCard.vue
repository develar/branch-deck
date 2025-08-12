<script setup lang="ts">
import { ref, computed } from "vue"
import { CollapsibleRoot, CollapsibleTrigger, CollapsibleContent } from "reka-ui"

defineSlots<{
  header: () => unknown
  default: () => unknown
}>()

const isOpen = ref(false)

// Card styling classes
const rootClasses = computed(() => [
  "rounded-lg overflow-hidden",
  // outline variant
  "bg-default ring ring-default",
  /// ... with dynamic border class
  isOpen.value ? "divide-y divide-default" : "",
])
</script>

<template>
  <CollapsibleRoot v-model:open="isOpen">
    <div :class="rootClasses">
      <!-- Header with CollapsibleTrigger -->
      <div class="p-4 sm:px-6">
        <CollapsibleTrigger class="w-full group cursor-pointer">
          <slot name="header" />
        </CollapsibleTrigger>
      </div>

      <!-- Body only renders when expanded -->
      <CollapsibleContent v-if="isOpen" class="overflow-x-auto">
        <slot />
      </CollapsibleContent>
    </div>
  </CollapsibleRoot>
</template>