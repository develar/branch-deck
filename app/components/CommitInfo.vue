<template>
  <div class="flex items-center gap-2 text-xs text-muted">
    <span class="font-mono">{{ shortHash }}</span>
    <span v-if="author">by {{ author }}</span>
    <span v-if="timestamp">{{ formatTimestamp(timestamp) }}</span>
    <span v-if="fileCount !== undefined" class="flex items-center gap-1">
      <span>•</span>
      <UIcon name="i-lucide-file-code" class="w-3 h-3"/>
      <span>{{ fileCount }} {{ fileCount === 1 ? 'file' : 'files' }}</span>
    </span>
  </div>
</template>

<script lang="ts" setup>
import { computed } from 'vue'
import { formatTimestamp } from '~/utils/time'

const props = defineProps<{
  hash: string
  author?: string
  timestamp?: number
  fileCount?: number
}>()

const shortHash = computed(() => {
  return props.hash.substring(0, 8)
})
</script>