<template>
  <UPopover mode="hover">
    <span class="font-mono">
      {{ formatShortHash(hash) }}
    </span>
    <template #content>
      <div class="p-3 space-y-2 min-w-[300px]">
        <p class="text-sm font-medium text-highlighted">{{ subject || message || "No message available" }}</p>
        <div v-if="hasFullMessage" class="text-xs text-toned whitespace-pre-wrap mt-2">{{ body }}</div>
        <CommitInfo :hash="hash" :author="author" :author-time="authorTime" />
      </div>
    </template>
  </UPopover>
</template>

<script lang="ts" setup>
const props = defineProps<{
  hash: string
  subject?: string
  message?: string
  author?: string
  authorTime?: number
}>()

// Check if the message has more than just the subject line
const hasFullMessage = computed(() => {
  return props.message && props.subject && props.message !== props.subject && props.message.includes("\n")
})

// Extract the body (everything after the first line)
const body = computed(() => {
  if (!hasFullMessage.value || !props.message) return ""
  const lines = props.message.split("\n")
  // Skip the subject line and any empty lines immediately after
  let bodyStartIndex = 1
  while (bodyStartIndex < lines.length && lines[bodyStartIndex]?.trim() === "") {
    bodyStartIndex++
  }
  return lines.slice(bodyStartIndex).join("\n").trim() || ""
})
</script>
