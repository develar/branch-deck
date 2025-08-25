<template>
  <div class="flex items-center gap-2 text-xs text-muted">
    <span class="font-mono">{{ formatShortHash(hash) }}</span>
    <span v-if="author">by {{ author }}</span>
    <TimeWithPopover v-if="authorTime" :author-time="authorTime" :committer-time="committerTime" />
    <span v-if="fileCount !== undefined" class="flex items-center gap-1">
      <span>•</span>
      <UIcon name="i-lucide-file-code" class="size-3" />
      <span>{{ fileCount }} {{ fileCount === 1 ? 'file' : 'files' }}</span>
    </span>
  </div>
</template>

<script lang="ts" setup>
defineProps<{
  hash: string
  author?: string
  authorTime?: number
  committerTime?: number
  fileCount?: number
}>()
</script>
