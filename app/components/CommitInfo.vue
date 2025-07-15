<template>
  <div class="flex items-center gap-2 text-xs text-muted">
    <span class="font-mono">{{ formatShortHash(hash) }}</span>
    <span v-if="author">by {{ author }}</span>
    <TimestampWithPopover v-if="timestamp" :author-time="timestamp" :committer-time="committerTimestamp" />
    <span v-if="fileCount !== undefined" class="flex items-center gap-1">
      <span>•</span>
      <UIcon name="i-lucide-file-code" class="w-3 h-3"/>
      <span>{{ fileCount }} {{ fileCount === 1 ? 'file' : 'files' }}</span>
    </span>
  </div>
</template>

<script lang="ts" setup>
import { formatShortHash } from "~/utils/hash"

defineProps<{
  hash: string
  author?: string
  timestamp?: number
  committerTimestamp?: number
  fileCount?: number
}>()
</script>