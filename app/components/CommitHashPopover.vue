<template>
  <UPopover mode="hover">
    <span class="font-mono">
      {{ shortHash }}
    </span>
    <template #content>
      <div class="p-3 space-y-2 min-w-[300px]">
        <p class="text-sm text-toned">{{ message || "No message available" }}</p>
        <CommitInfo :hash="hash" :author="author" :timestamp="timestamp" />
      </div>
    </template>
  </UPopover>
</template>

<script lang="ts" setup>
import { computed } from 'vue'

const props = defineProps<{
  hash: string
  message?: string
  author?: string
  timestamp?: number
}>()

const shortHash = computed(() => {
  return props.hash.substring(0, 8)
})
</script>