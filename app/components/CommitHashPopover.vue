<template>
  <UPopover mode="hover">
    <span class="font-mono">
      {{ formatShortHash(hash) }}
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
import { formatShortHash } from "~/utils/hash"

defineProps<{
  hash: string
  message?: string
  author?: string
  timestamp?: number
}>()
</script>